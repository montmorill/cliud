use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use cliud::http::{Request, Response};
use cliud::middleware::{Middleware, Next};
use cliud::server::Server;
use cliud::service::{ConnectionFlag, Service};
use cliud::websocket::{Result, WebSocket, WebSocketExt, WebSocketState};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let host = "127.0.0.1:4221";
    let listener = TcpListener::bind(host).await?;
    println!("Listening on {host}...");

    let server = Server::<Box<dyn std::error::Error + Send + Sync>, _>::default()
        .middleware(cliud::websocket::WebSocketHandshakeMiddleware)
        .middleware(cliud::compress::CompressMiddleware { min_size: 1024 })
        .service(EchoWebSocketService)
        .middleware(Router)
        .leak();

    loop {
        let (stream, address) = listener.accept().await?;
        tokio::spawn(server.handle_connection(stream, address));
    }
}

struct Router;

#[async_trait]
impl<E: From<std::io::Error>> Middleware<E> for Router {
    async fn call(&self, request: & Request, next: &dyn Next<E>) -> Result<Response, E> {
        Ok(if request.target == "/" {
            Response::ok().plain(b"Hello, world!")
        }
        // /chat
        else if request.target == "/chat" {
            Response::ok().html(fs::read("./chat.html").await?)
        }
        // /echo/{content}
        else if let Some(content) = request.target.strip_prefix("/echo/") {
            Response::ok().plain(content)
        }
        // /cat/{status_code}/{description}/{body}
        else if let Some(path) = request.target.strip_prefix("/cat/") {
            let mut splited = path.split('/');
            let (status_code, description, body) = (|| {
                let status_code = splited.next()?;
                let description = splited.next()?;
                let body: String = splited.collect::<Vec<_>>().join("/");
                Some((status_code, description, body))
            })()
            .unwrap_or_else(|| ("400", "Bad Request", String::new()));
            Response::new(status_code, description).with_body(body.as_bytes())
        }
        // /user-agent
        else if request.target == "/user-agent" {
            match request.headers.get("User-Agent") {
                Some(user_agent) => Response::ok().plain(user_agent.as_bytes()),
                None => Response::ok().plain(b"User-Agent not found!"),
            }
        }
        // /files/{filepath}
        else if let Some(filepath) = request.target.strip_prefix("/files/") {
            let path = PathBuf::from_iter(["./", filepath]);
            match request.method.as_str() {
                "GET" => {
                    if path.is_file() {
                        Response::ok().plain(fs::read(path).await?)
                    } else if path.is_dir()
                        && let Ok(mut entries) = fs::read_dir(&path).await
                    {
                        let mut resp = Response::ok().html(b"<ul>");
                        resp.body.extend(
                            format!(
                                "<li><a href=\"/files/{}\">./</a></li>\n",
                                path.to_str().unwrap()
                            )
                            .as_bytes(),
                        );
                        if let Some(parent) = path.parent() {
                            resp.body.extend(
                                format!(
                                    "<li><a href=\"/files/{}\">../</a></li>\n",
                                    parent.to_str().unwrap()
                                )
                                .as_bytes(),
                            );
                        }
                        while let Some(entry) = entries.next_entry().await? {
                            resp.body.extend(
                                format!(
                                    "<li><a href=\"/files/{}\">{}{}</a></li>\n",
                                    {
                                        let path = path
                                            .join(entry.file_name())
                                            .into_os_string()
                                            .into_string()
                                            .unwrap();
                                        match path.strip_prefix("./") {
                                            Some(path) => path.to_owned(),
                                            None => path,
                                        }
                                    },
                                    entry.file_name().into_string().unwrap(),
                                    if entry.file_type().await?.is_dir() {
                                        "/"
                                    } else {
                                        ""
                                    },
                                )
                                .as_bytes(),
                            );
                        }
                        resp.body.extend(b"</ul>");
                        resp
                    } else {
                        Response::new(404, "Not Found")
                    }
                }
                "POST" => {
                    fs::write(path, &request.body).await?;
                    Response::new(201, "Created")
                }
                _ => Response::new(501, "Not Implemented"),
            }
        } else {
            next.call(request).await?
        })
    }
}

struct EchoWebSocket<'a, S> {
    stream: Mutex<&'a mut S>,
    address: &'a SocketAddr,
    state: RwLock<WebSocketState>,
}

impl<'a, S> WebSocket for EchoWebSocket<'a, S>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin + Send,
{
    type Stream = &'a mut S;

    async fn stream_mut(&self) -> impl DerefMut<Target = Self::Stream> {
        self.stream.lock().await
    }

    async fn on_message(&mut self, message: Vec<u8>) -> Result<()> {
        eprintln!("receive message from {}: {message:?}", self.address);
        self.send_binary(message).await?;
        Ok(())
    }

    async fn on_close(&mut self, reason: Vec<u8>) -> Result<()> {
        eprintln!("disconnected with {}: {reason:?}", self.address);
        Ok(())
    }

    async fn on_pong(&mut self, delay: Duration) -> Result<()> {
        eprintln!("receive pong from {} in {delay:?}", self.address);
        Ok(())
    }

    async fn state(&self) -> impl Deref<Target = WebSocketState> {
        self.state.read().await
    }

    async fn state_mut(&self) -> impl DerefMut<Target = WebSocketState> {
        self.state.write().await
    }
}

struct EchoWebSocketService;

#[async_trait]
impl<E, S> Service<E, S> for EchoWebSocketService
where
    E: From<std::io::Error> + From<cliud::websocket::Error>,
    S: AsyncReadExt + AsyncWriteExt + Unpin + Send,
{
    async fn call(
        &self,
        _request: &Request,
        response: &Response,
        address: &SocketAddr,
        stream: &mut S,
    ) -> Result<ConnectionFlag, E> {
        if let Some(upgarde) = response.headers.get("Upgrade") {
            if upgarde == "websocket" {
                EchoWebSocket {
                    stream: Mutex::new(stream),
                    state: RwLock::new(WebSocketState::default()),
                    address,
                }
                .run()
                .await?;
            }
            Ok(ConnectionFlag::Close)
        } else {
            Ok(ConnectionFlag::Continue)
        }
    }
}

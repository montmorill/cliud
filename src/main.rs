use cliud::compress::try_compress;
use cliud::http::{Request, Response};
use cliud::websocket::{Result, WebSocket, WebSocketExt, WebSocketState};
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let host = "127.0.0.1:4221";
    let listener = TcpListener::bind(host).await?;
    println!("Listening on {host}...");

    loop {
        let (stream, address) = listener.accept().await?;
        tokio::spawn(async move { handle_connection(stream, address).await.unwrap() });
    }
}

async fn handle_connection(mut stream: TcpStream, address: SocketAddr) -> Result::<(), Box<dyn std::error::Error>> {
    let mut result = Ok(());

    let request = Request::from_buf_async(BufReader::new(&mut stream)).await?;

    let mut response = if let Some(upgrade) = request.headers.get("Upgrade")
        && upgrade == "websocket"
    {
        use base64::prelude::*;
        use sha1_smol::Sha1;

        if let Some(key) = request.headers.get("Sec-WebSocket-Key") {
            let concated = [key, "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"].concat();
            let hashed = Sha1::from(concated).digest().bytes();
            let encoded = BASE64_STANDARD.encode(hashed);

            Response::new(101, "Switching Protocols")
                .header("Upgrade", "websocket")
                .header("Connection", "Upgrade")
                .header("Sec-Websocket-Accept", encoded)
                .header("Sec-Websocket-Version", "13")
        } else {
            Response::new(400, "Bad Request")
        }
    } else {
        match handle_request(&request).await {
            Ok(resp) => resp,
            Err(err) => {
                let body = format!("{err}");
                result = Err(err);
                let mut resp = Response::plain(&body);
                resp.status_code = "500".to_string();
                resp.description = "Internal Server Error".into();
                resp
            }
        }
    };

    // compress
    if !response.body.is_empty() {
        if let Some(encodings) = request.headers.get("Accept-Encoding") {
            for encoding in encodings.split(",").map(str::trim) {
                if let Some(compressed) = try_compress(encoding, &response.body)? {
                    response = response.header("Content-Encoding", encoding);
                    response.body = compressed;
                    break;
                }
            }
        }
    }

    let length = response.body.len();
    if length != 0 {
        response = response.header("Content-Length", length);
    }

    stream.write_all(&response.to_bytes()).await?;
    stream.flush().await?;

    {
        use colored::*;
        eprintln!(
            r#"{} - "{}" - {}"#,
            address,
            request.request_line().bright_cyan(),
            response.response_line()[9..].color(
                match response.status_code.to_string().chars().next() {
                    Some('1') => "cyan",
                    Some('2') => "green",
                    Some('3') => "yellow",
                    Some('4') => "red",
                    Some('5') => "purple",
                    _ => "normal",
                }
            ),
        );
    }

    if let Some(upgarde) = response.headers.get("Upgrade") {
        if upgarde == "websocket" {
            EchoWebSocket {
                stream: Mutex::new(stream),
                state: RwLock::new(WebSocketState::default().timeout(Duration::from_secs(1))),
                address,
            }
            .run()
            .await?;
        }
    }

    result.map_err(|err| err.into())
}

pub async fn handle_request(request: &Request) -> Result<Response> {
    // (index)
    let response = if request.target == "/" {
        Response::plain(b"Hello, world!")
    }
    // /chat
    else if request.target == "/chat" {
        Response::html(&fs::read("./chat.html").await?)
    }
    // /echo/{content}
    else if let Some(content) = request.target.strip_prefix("/echo/") {
        Response::plain(&content)
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
        Response::new(status_code, description).body(&body)
    }
    // /user-agent
    else if request.target == "/user-agent" {
        match request.headers.get("User-Agent") {
            Some(user_agent) => Response::plain(user_agent),
            None => Response::plain(b"User-Agent not found!"),
        }
    }
    // /files/{filepath}
    else if let Some(filepath) = request.target.strip_prefix("/files/") {
        let path = PathBuf::from_iter(["./", filepath]);
        match request.method.as_str() {
            "GET" => {
                if path.is_file() {
                    Response::plain(&fs::read(path).await?)
                } else if path.is_dir()
                    && let Ok(mut entries) = fs::read_dir(&path).await
                {
                    let mut resp = Response::html(b"<ul>");
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
        Response::new(404, "Not Found")
    };

    Ok(response)
}

struct EchoWebSocket {
    stream: Mutex<TcpStream>,
    address: SocketAddr,
    state: RwLock<WebSocketState>,
}

impl WebSocket for EchoWebSocket {
    type Stream = TcpStream;

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

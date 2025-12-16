use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};

use async_trait::async_trait;
use cliud::BoxError;
use cliud::http::{Request, Response};
use cliud::middleware::{Middleware, Next};
use cliud::server::Server;
use cliud::service::{ConnectionFlag, Service};
use cliud::websocket::{Result, WebSocket, WebSocketExt as _, WebSocketState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let address = SocketAddr::new("127.0.0.1".parse().unwrap(), 4223);
    let listener = TcpListener::bind(&address).await?;
    println!("Listening on {address}");

    let server = Server::<BoxError, _>::default()
        .with_middleware(RouterMiddleware)
        .with_middleware(cliud::websocket::WebSocketHandshakeMiddleware)
        .with_service(EchoWebSocketService)
        .leak();

    loop {
        let (stream, addr) = listener.accept().await?;
        tokio::spawn(server.handle_connection(stream, addr));
    }
}

struct RouterMiddleware;

#[async_trait]
impl<E: From<std::io::Error>> Middleware<E> for RouterMiddleware {
    async fn call(&self, request: &Request, next: &dyn Next<E>) -> Result<Response, E> {
        Ok(if request.target == "/" {
            Response::ok().html(include_bytes!("./index.html"))
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

    async fn state(&self) -> impl Deref<Target = WebSocketState> {
        self.state.read().await
    }

    async fn state_mut(&self) -> impl DerefMut<Target = WebSocketState> {
        self.state.write().await
    }

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
        if let Some(upgarde) = response.headers.get("Upgrade")
            && upgarde == "websocket"
        {
            EchoWebSocket {
                stream: Mutex::new(stream),
                state: RwLock::new(WebSocketState::default()),
                address,
            }
            .run()
            .await?;
            Ok(ConnectionFlag::Close)
        } else {
            Ok(ConnectionFlag::Continue)
        }
    }
}

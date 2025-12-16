#![feature(str_split_remainder)]

use std::env::var;
use std::net::SocketAddr;

use async_trait::async_trait;
use cliud::BoxError;
use cliud::http::{Request, Response};
use cliud::middleware::{Middleware, Next};
use cliud::server::Server;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let host = var("CLIUD_HOST")
        .unwrap_or_else(|_| "127.0.0.1".to_owned())
        .parse()
        .expect("CLIUD_HOST must be a valid IP address");
    let port = var("CLIUD_PORT")
        .map(|s| s.parse::<u16>().expect("CLIUD_PORT must be a valid port number"))
        .unwrap_or(4221);
    let address = SocketAddr::new(host, port);
    let listener = TcpListener::bind(&address).await?;
    println!("Listening on {address}");

    let server = Server::<BoxError, _>::default()
        .with_middleware(RouterMiddleware)
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
            Response::ok().plain(b"Hello, world!")
        }
        // /echo/{content}
        else if let Some(content) = request.target.strip_prefix("/echo/") {
            Response::ok().plain(content)
        }
        // /cat/{status_code}/{description}/{body}
        else if let Some(path) = request.target.strip_prefix("/cat/") {
            let mut splited = path.split('/');
            (|| {
                let status_code = splited.next()?;
                let description = splited.next()?;
                let body = splited.remainder().unwrap_or_default();
                Some(Response::new(status_code, description).with_body(body))
            })()
            .unwrap_or_else(|| Response::new(400, "Bad Request"))
        }
        // /user-agent
        else if request.target == "/user-agent" {
            match request.headers.get("User-Agent") {
                Some(user_agent) => Response::ok().plain(user_agent.as_bytes()),
                None => Response::ok().plain(b"User-Agent not found!"),
            }
        } else {
            next.call(request).await?
        })
    }
}

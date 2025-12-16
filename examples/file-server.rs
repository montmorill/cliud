use std::net::SocketAddr;
use std::path::PathBuf;

use async_trait::async_trait;
use cliud::http::{Request, Response};
use cliud::middleware::{Middleware, Next};
use cliud::server::Server;
use cliud::websocket::Result;
use tokio::fs::{read, read_dir, write};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let address = SocketAddr::new("127.0.0.1".parse().unwrap(), 4223);
    let listener = TcpListener::bind(&address).await?;
    println!("Listening on {address}");

    let server = Server::<std::io::Error, _>::default()
        .with_middleware(FileServerMiddleware {
            root: "./",
            endpoint: "/file/",
        })
        .leak();

    loop {
        let (stream, addr) = listener.accept().await?;
        tokio::spawn(server.handle_connection(stream, addr));
    }
}

struct FileServerMiddleware {
    root: &'static str,
    endpoint: &'static str,
}

#[async_trait]
impl<E: From<std::io::Error>> Middleware<E> for FileServerMiddleware {
    async fn call(&self, request: &Request, next: &dyn Next<E>) -> Result<Response, E> {
        if let Some(path) = request.target.strip_prefix(self.endpoint) {
            let path = PathBuf::from_iter([self.root, path]);
            Ok(match request.method.as_str() {
                "GET" => read_filepath(&path, self.endpoint).await?,
                "POST" => {
                    write(path, &request.body).await?;
                    Response::new(201, "Created")
                }
                _ => Response::new(405, "Method Not Allowed"),
            })
        } else {
            println!("{:?}", request);
            next.call(request).await
        }
    }
}

async fn read_filepath(path: &PathBuf, endpoint: &str) -> std::io::Result<Response> {
    if !path.exists() {
        Ok(Response::new(404, "Not Found"))
    } else if path.is_file() {
        Ok(Response::ok().plain(read(path).await?))
    } else {
        let mut entries = read_dir(&path).await?;

        let mut response = Response::ok().html(format!("<p>{}:</p><ul>", path.canonicalize()?.to_string_lossy()));

        let current_dir = format!("<li><a href=\"{endpoint}{}\">./</a></li>", path.to_string_lossy());
        response.body.extend(current_dir.as_bytes());

        if let Some(parent) = path.parent() {
            let parent_dir = format!("<li><a href=\"{endpoint}{}\">../</a></li>", parent.to_string_lossy());
            response.body.extend(parent_dir.as_bytes());
        }

        while let Some(entry) = entries.next_entry().await? {
            let path = path
                .join(entry.file_name())
                .into_os_string()
                .to_string_lossy()
                .to_string();
            response.body.extend(
                format!(
                    "<li><a href=\"{endpoint}{}\">{}{}</a></li>",
                    path.strip_prefix("./").unwrap_or(&path),
                    entry.file_name().to_string_lossy(),
                    if entry.file_type().await?.is_dir() { "/" } else { "" },
                )
                .as_bytes(),
            );
        }

        response.body.extend(b"</ul>");
        Ok(response)
    }
}

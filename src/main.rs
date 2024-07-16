use flate2::write::GzEncoder;
use flate2::Compression;
use http_server_starter_rust::request::{Request, Response};
use itertools::Itertools;
use std::io::Write;
use std::mem;
use std::net::SocketAddr;
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let host = "127.0.0.1:4221";
    let listener = TcpListener::bind(host).await?;
    println!("listening on {host}...");

    loop {
        let (stream, address) = listener.accept().await?;
        tokio::spawn(handle_connection(stream, address));
    }
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Formatting error: {0}")]
    Fmt(#[from] std::fmt::Error),
}

async fn handle_connection(mut stream: TcpStream, address: SocketAddr) -> Result<(), Error> {
    println!("connected with {address}!");

    let (mut reader, mut writer) = stream.split();
    let mut reader = BufReader::new(&mut reader);

    let request = Request::from_async_buf(&mut reader).await?;
    let mut response = handle_request(request.clone()).await?;

    if let Some(encodings) = request.headers.get("Accept-Encoding") {
        let mut encodings = encodings.split(",").map(str::trim);
        if encodings.contains(&"gzip") {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(mem::take(&mut response.body.as_slice()))?;
            let body = encoder.finish()?;
            response = response
                .header("Content-Encoding", "gzip")
                .header("Content-Length", body.len().to_string());
            response.body = body;
        }
    }


    writer.write_all(&response.to_bytes()?).await?;
    writer.flush().await?;

    dbg!(request, response);
    println!("disconnected with {address}!");

    Ok(())
}

async fn handle_request(request: Request) -> std::io::Result<Response> {
    let directory = std::env::args().nth(2).unwrap_or("./".to_string());

    match request.method.as_str() {
        "GET" => {
            if request.target == "/" {
                return Ok(Response::new(200, "OK"));
            } else if let Some(content) = request.target.strip_prefix("/echo/") {
                return Ok(Response::new(200, "OK")
                    .header("Content-Type", "text/plain")
                    .header("Content-Length", content.as_bytes().len().to_string())
                    .body(content.as_bytes()));
            } else if request.target == "/user-agent" {
                if let Some(user_agent) = request.headers.get("User-Agent") {
                    return Ok(Response::new(200, "OK")
                        .header("Content-Type", "text/plain")
                        .header("Content-Length", user_agent.as_bytes().len().to_string())
                        .body(user_agent.as_bytes()));
                }
            } else if let Some(filename) = request.target.strip_prefix("/files/") {
                if let Ok(content) = fs::read(format!("{}/{}", directory, filename)).await {
                    return Ok(Response::new(200, "OK")
                        .header("Content-Type", "application/octet-stream")
                        .header("Content-Length", content.len().to_string())
                        .body(&content));
                }
            }
        }
        "POST" => {
            if let Some(filename) = request.target.strip_prefix("/files/") {
                fs::write(format!("{}/{}", directory, filename), request.body).await?;
                return Ok(Response::new(201, "Created"));
            }
        }
        _ => return Ok(Response::new(501, "Not Implemented")),
    }

    Ok(Response::new(404, "Not Found"))
}

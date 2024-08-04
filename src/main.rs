use cliud::compress::try_compress;
use cliud::http::{escape, Request, Response};
use cliud::websocket::{handle_websocket, Result};
use colored::*;
use std::net::SocketAddr;
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let host = "127.0.0.1:4221";
    let listener = TcpListener::bind(host).await?;
    println!("Listening on {host}...");

    loop {
        let (stream, address) = listener.accept().await?;
        tokio::spawn(handle_connection(stream, address));
    }
}

async fn handle_connection(mut stream: TcpStream, address: SocketAddr) -> Result<()> {
    let request = Request::from_buf_async(BufReader::new(&mut stream)).await?;
    let mut response = handle_request(&request).await;

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

    if let Some(upgarde) = response.headers.get("Upgrade") {
        if upgarde == "websocket" {
            handle_websocket(stream, address).await?;
        }
    }

    Ok(())
}

pub async fn handle_request(request: &Request) -> Response {
    if let Some(upgrade) = request.headers.get("Upgrade") {
        if upgrade == "websocket" {
            use base64::prelude::*;
            use sha1_smol::Sha1;

            if let Some(key) = request.headers.get("Sec-WebSocket-Key") {
                let concated = [key, "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"].concat();
                let hashed = Sha1::from(concated).digest().bytes();
                let encoded = BASE64_STANDARD.encode(hashed);

                return Response::new(101, "Switching Protocols")
                    .header("Upgrade", "websocket")
                    .header("Connection", "Upgrade")
                    .header("Sec-Websocket-Accept", encoded)
                    .header("Sec-Websocket-Version", "13");
            } else {
                return Response::new(400, "Bad Request");
            };
        }
    }

    // (index)
    if request.target == "/" {
        return Response::plain(200, "OK").body(b"Hello, world!");
    }
    // /chat
    else if request.target == "/chat" {
        return Response::html(200, "OK").body(&include_bytes!("../chat.html"));
    }
    // /echo/{content}
    else if let Some(content) = request.target.strip_prefix("/echo/") {
        return Response::plain(200, "OK").body(&content);
    }
    // /cat/{status_code}/{description}/{body}
    else if let Some(path) = request.target.strip_prefix("/cat/") {
        let path = escape(path.to_owned());
        let mut splited = path.split('/');
        let status_code = splited.next().unwrap_or("404");
        let description = splited.next().unwrap_or("Not Found");
        let body = splited.collect::<Vec<_>>().join("/");
        return Response::new(status_code, description).body(&body);
    }
    // /user-agent
    else if request.target == "/user-agent" {
        if let Some(user_agent) = request.headers.get("User-Agent") {
            return Response::plain(200, "OK").body(user_agent);
        }
    }
    // /files/filename
    else if let Some(filename) = request.target.strip_prefix("/files/") {
        let directory = std::env::args().nth(2).unwrap_or("./".to_string());
        let path = format!("{directory}/{filename}");
        return match request.method.as_str() {
            "GET" => match fs::read(path).await {
                Ok(content) => Response::plain(200, "OK").body(&content),
                Err(_) => Response::new(404, "Not Found"),
            },
            "POST" => match fs::write(path, &request.body).await {
                Ok(_) => Response::new(201, "Created"),
                Err(_) => Response::new(500, "Internal Server Error"),
            },
            _ => Response::new(501, "Not Implemented"),
        };
    }

    Response::new(404, "Not Found")
}

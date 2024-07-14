use http_server_starter_rust::request::{Request, Response, Result};
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::{env, fs};

const PROTOCOL: &str = "HTTP/1.1";

fn handle_connection(mut stream: TcpStream) -> Result<()> {
    let request = Request::try_from_stream(&stream)?;
    assert_eq!(request.protocol, PROTOCOL);
    match request.method.as_str() {
        "GET" => {
            let target = request.target.as_str();
            let response = if target == "/" {
                Response::new(PROTOCOL, 200.to_string(), "OK")
            } else if target == "/user-agent" {
                let user_agent = &request.headers["User-Agent"];
                Response::new(PROTOCOL, 200.to_string(), "OK")
                    .header("Content-Type", "text/plain")
                    .header("Content-Length", user_agent.len().to_string())
                    .body(user_agent)
            } else if let Some(content) = target.strip_prefix("/echo/") {
                Response::new(PROTOCOL, 200.to_string(), "OK")
                    .header("Content-Type", "text/plain")
                    .header("Content-Length", content.len().to_string())
                    .body(content)
            } else if let Some(filename) = target.strip_prefix("/files/") {
                let path = format!("{}/{}", env::args().nth(2).unwrap(), filename);
                if let Ok(content) = fs::read(path) {
                    Response::new(PROTOCOL, 200.to_string(), "OK")
                        .header("Content-Type", "application/octet-stream")
                        .header("Content-Length", content.len().to_string())
                        .body(dbg!(String::from_utf8(content).unwrap()))
                } else {
                    Response::new(PROTOCOL, 404.to_string(), "Not Found")
                }
            } else {
                Response::new(PROTOCOL, 404.to_string(), "Not Found")
            };
            stream.write(response.to_string().as_bytes())?;
            stream.flush()?;
        }
        _ => unimplemented!(),
    }

    Ok(())
}

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4221")?;

    for stream in listener.incoming() {
        println!("accepted new connection");
        std::thread::spawn(|| handle_connection(stream?));
    }

    Ok(())
}

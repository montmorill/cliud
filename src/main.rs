use http_server_starter_rust::request::{Request, Response, Result};
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};

const PROTOCOL: &str = "HTTP/1.1";

fn handle_connection(mut stream: TcpStream) -> Result<()> {
    let request = Request::try_from_stream(&stream)?;
    assert_eq!(request.protocol, PROTOCOL);
    match request.method.as_str() {
        "GET" => {
            let target = request.target.as_str();
            let response = if target == "/" {
                Response::new(PROTOCOL, 200.to_string(), "OK")
            } else if let Some(str) = target.strip_prefix("/echo/") {
                Response::new(PROTOCOL, 200.to_string(), "OK")
                    .header("Content-Type", "text/plain")
                    .header("Content-Length", str.len().to_string())
                    .body(str)
            } else {
                Response::new(PROTOCOL, 404.to_string(), "Not Found")
            };
            stream.write(response.to_string().as_bytes())?;
            stream.flush()?;
            dbg!(request, response);
        }
        _ => unimplemented!(),
    }

    Ok(())
}

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4221")?;

    for stream in listener.incoming() {
        println!("accepted new connection");
        handle_connection(stream?)?;
    }

    Ok(())
}

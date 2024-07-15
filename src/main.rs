use http_server_starter_rust::request::{Request, Response};
use lazy_static::lazy_static;
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

lazy_static! {
    static ref BASE_DIR: String = std::env::args().nth(2).unwrap_or("./".to_string());
}

#[tokio::main]
async fn main() {
    let host = "127.0.0.1:4221";
    let listener = TcpListener::bind(host).await.unwrap();
    println!("listening on {host}");

    loop {
        let (stream, address) = listener.accept().await.unwrap();
        tokio::spawn(handle_connection(stream));
        println!("connected with {address}");
    }
}

async fn handle_connection(mut stream: TcpStream) {
    let (mut reader, mut writer) = stream.split();
    let mut reader = BufReader::new(&mut reader);

    let request = Request::from_async_buf(&mut reader).await;

    let response = handle_request(request.clone()).await;

    writer.write(response.to_string().as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    dbg!(request, response);

    // stream.read_to_string(&mut buf).await.unwrap();
    // println!("{}", buf);
}

async fn handle_request(request: Request) -> Response {
    match request.method.as_str() {
        "GET" => {
            if request.target == "/" {
                return Response::new(200, "OK");
            } else if let Some(content) = request.target.strip_prefix("/echo/") {
                return Response::new(200, "OK")
                    .header("Content-Type", "text/plain")
                    .header("Content-Length", content.as_bytes().len().to_string())
                    .body(content);
            } else if request.target == "/user-agent" {
                if let Some(user_agent) = request.headers.get("User-Agent") {
                    return Response::new(200, "OK")
                        .header("Content-Type", "text/plain")
                        .header("Content-Length", user_agent.as_bytes().len().to_string())
                        .body(user_agent);
                }
            } else if let Some(filename) = request.target.strip_prefix("/files/") {
                if let Ok(content) = fs::read(format!("{}/{}", *BASE_DIR, filename)).await {
                    return Response::new(200, "OK")
                        .header("Content-Type", "application/octet-stream")
                        .header("Content-Length", content.len().to_string())
                        .body(&String::from_utf8_lossy(&content));
                }
            }
        }

        "POST" => {
            if let Some(filename) = request.target.strip_prefix("/files/") {
                fs::write(format!("{}/{}", *BASE_DIR, filename), request.body)
                    .await
                    .unwrap();
                return Response::new(201, "Created");
            }
        }

        _ => return Response::new(501, "Not Implemented"),
    }

    Response::new(404, "Not Found")
}

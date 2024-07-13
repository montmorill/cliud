use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 512];
    let _ = stream.read(&mut buffer).unwrap();
    let request = String::from_utf8_lossy(&buffer);
    let mut splited = request.split("\r\n").into_iter();
    let mut line = splited.next().unwrap().split(" ").into_iter();
    let method = line.next().unwrap();
    let target = line.next().unwrap();
    let version = line.next().unwrap();

    assert_eq!(version, "HTTP/1.1");
    match method {
        "GET" => {
            let responce = match target {
                "/" => "HTTP/1.1 200 OK\r\n\r\n",
                _ => "HTTP/1.1 404 Not Found\r\n\r\n",
            };
            println!("{responce}");
            stream.write(responce.as_bytes()).unwrap();
            stream.flush().unwrap();
        }
        _ => unimplemented!(),
    }
}

fn main() {
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming().map(Result::unwrap) {
        println!("accepted new connection");
        handle_connection(stream);
    }
}

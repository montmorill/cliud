use std::io::{prelude::*, BufReader};
use std::net::{TcpListener, TcpStream};

fn handle_connection(mut stream: TcpStream) {
    let reader = BufReader::new(&stream);
    let mut lines = reader.lines().map(Result::unwrap).into_iter();
    let line = lines.next().unwrap();
    let mut splited = line.split(" ").into_iter();
    let method = splited.next().unwrap();
    let target = splited.next().unwrap();
    let version = splited.next().unwrap();

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

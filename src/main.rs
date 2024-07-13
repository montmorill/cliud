use std::io::{Read, Write};
use std::net::TcpListener;

fn main() {
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for mut stream in listener.incoming().map(Result::unwrap) {
        println!("accepted new connection");
        let mut buf = String::new();
        let _ = stream.read_to_string(&mut buf).unwrap();
        println!("{buf}");
        let _ = stream.write(b"HTTP/1.1 200 OK\r\n\r\n").unwrap();
    }
}

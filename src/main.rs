use std::io::prelude::*;
use std::net::TcpListener;

fn main() {
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for mut stream in listener.incoming().map(Result::unwrap) {
        println!("accepted new connection");

        let mut request = String::new();
        stream.read_to_string(&mut request).unwrap();
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
                stream.write(responce.as_bytes()).unwrap();
            }
            _ => unimplemented!(),
        }
    }
}

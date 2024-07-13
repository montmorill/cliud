use std::collections::HashMap;
use std::fmt::Display;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub protocol: String,
    pub headers: HashMap<String, String>,
    // pub body: Vec<u8>,
}

impl Request {
    pub fn try_from_stream(stream: &TcpStream) -> Result<Self> {
        let mut reader = BufReader::new(stream);

        let request_line = read_line(&mut reader)?;
        let mut splited = request_line.split(" ").into_iter();
        let method = splited.next().ok_or(Error::Missing("method"))?.into();
        let target = splited.next().ok_or(Error::Missing("target"))?.into();
        let protocol = splited.next().ok_or(Error::Missing("protocol"))?.into();

        let mut headers = HashMap::new();

        while let Some((key, value)) = read_line(&mut reader)?.split_once(":") {
            headers.insert(key.trim().into(), value.trim().into());
        }

        // let mut body = Vec::new();
        // reader.read_to_end(&mut body)?;

        Ok(Self {
            method,
            target,
            protocol,
            headers,
            // body,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub protocol: String,
    pub code: String,
    pub status: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} {}\r\n", self.protocol, self.code, self.status)?;
        for (key, value) in &self.headers {
            write!(f, "{key}: {value}\r\n")?;
        }
        write!(f, "\r\n{}", self.body)?;
        Ok(())
    }
}

impl Response {
    pub fn new(
        protocol: impl Into<String>,
        code: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            protocol: protocol.into(),
            code: code.into(),
            status: status.into(),
            headers: HashMap::new(),
            body: String::new(),
        }
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn build(self) -> Response {
        Response {
            protocol: self.protocol,
            code: self.code,
            status: self.status,
            headers: self.headers,
            body: self.body,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid data to decode")]
    InvalidData(#[from] std::io::Error),
    #[error("missing {0}")]
    Missing(&'static str),
}
pub type Result<T> = std::result::Result<T, Error>;

fn read_line(reader: &mut impl BufRead) -> std::io::Result<String> {
    let mut buf = String::new();
    loop {
        (*reader).read_line(&mut buf)?;
        if buf.ends_with("\r\n") {
            buf.pop(); // pop the `\n`
            buf.pop(); // and the `\r`
            break Ok(buf);
        }
    }
}

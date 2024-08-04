use std::collections::HashMap;
use std::fmt::Write;
use tokio::io::{AsyncBufReadExt, AsyncReadExt};

pub fn escape(raw: String) -> String {
    raw.replace("%20", " ")
}

async fn read_line(stream: &mut (impl AsyncBufReadExt + Unpin)) -> std::io::Result<String> {
    let mut buf = String::new();
    loop {
        stream.read_line(&mut buf).await?;

        if buf.ends_with("\r\n") {
            buf.pop(); // pop the `\n`
            buf.pop(); // and the `\r`
            break Ok(buf);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    pub async fn from_buf_async(
        mut stream: impl AsyncReadExt + AsyncBufReadExt + Unpin,
    ) -> std::io::Result<Self> {
        let request_line = read_line(&mut stream).await?;
        let mut splited = request_line.split(" ").into_iter();
        let method = splited.next().unwrap().into();
        let target = splited.next().unwrap().into();
        let version = splited.next().unwrap().into();

        let mut headers: HashMap<String, String> = HashMap::new();

        while let Some((key, value)) = read_line(&mut stream).await?.split_once(":") {
            headers.insert(key.trim().into(), value.trim().into());
        }

        let length = headers
            .get("Content-Length")
            .map_or(0, |length| length.as_str().parse::<usize>().unwrap());
        let mut body = Vec::with_capacity(length);
        if length != 0 {
            stream.read_exact(&mut body).await?;
        }

        Ok(Self {
            method,
            target,
            version,
            headers,
            body,
        })
    }

    pub fn request_line(&self) -> String {
        format!("{} {} {}", self.method, self.target, self.version)
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub version: String,
    pub status_code: String,
    pub description: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status_code: impl ToString, description: impl ToString) -> Self {
        Self {
            version: "HTTP/1.1".into(),
            status_code: status_code.to_string(),
            description: description.to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    pub fn plain(status_code: impl ToString, description: impl ToString) -> Self {
        Self::new(status_code, description).header("Content-Type", "text/plain")
    }

    pub fn file(status_code: impl ToString, description: impl ToString) -> Self {
        Self::new(status_code, description).header("Content-Type", "application/octet-stream")
    }

    pub fn html(status_code: impl ToString, description: impl ToString) -> Self {
        Self::new(status_code, description).header("Content-Type", "text/html")
    }

    pub fn header(mut self, key: impl ToString, value: impl ToString) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn body(mut self, body: &impl AsRef<[u8]>) -> Self {
        self.body.extend(body.as_ref());
        self
    }

    pub fn response_line(&self) -> String {
        format!("{} {} {}", self.version, self.status_code, self.description)
    }

    pub fn response_headers(&self) -> String {
        let mut buf = self.response_line() + "\r\n";
        for (key, value) in &self.headers {
            write!(buf, "{key}: {value}\r\n").unwrap();
        }
        buf
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.response_headers().as_bytes().to_vec();
        bytes.extend(b"\r\n");
        bytes.extend(&self.body);
        bytes
    }
}

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.response_headers().fmt(f)?;
        write!(f, "\r\n")?;
        String::from_utf8_lossy(&self.body).fmt(f)
    }
}

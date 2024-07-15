use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncReadExt};

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

async fn read_line(reader: &mut (impl AsyncBufReadExt + Unpin)) -> std::io::Result<String> {
    let mut buf = String::new();
    loop {
        reader.read_line(&mut buf).await?;

        if buf.ends_with("\r\n") {
            buf.pop(); // pop the `\n`
            buf.pop(); // and the `\r`
            break Ok(buf);
        }
    }
}

impl Request {
    pub async fn from_async_buf(
        reader: &mut (impl AsyncReadExt + AsyncBufReadExt + Unpin),
    ) -> Self {
        let request_line = read_line(reader).await.unwrap();
        let mut splited = request_line.split(" ").into_iter();
        let method = splited.next().unwrap().into();
        let target = splited.next().unwrap().into();
        let version = splited.next().unwrap().into();

        let mut headers: HashMap<String, String> = HashMap::new();

        while let Some((key, value)) = read_line(reader).await.unwrap().split_once(":") {
            headers.insert(key.trim().into(), value.trim().into());
        }

        let length = headers
            .get("Content-Length")
            .map_or(0, |length| length.as_str().parse::<usize>().unwrap());
        let mut body = Vec::with_capacity(length);
        if length != 0 {
            reader.read_buf(&mut body).await.unwrap();
        }
        let body = String::from_utf8_lossy(&body).to_string();

        Self {
            method,
            target,
            version,
            headers,
            body,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub version: String,
    pub status_code: usize,
    pub description: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl Response {
    pub fn new(
        // version: impl Into<String>,
        status_code: usize,
        description: impl Into<String>,
    ) -> Self {
        Self {
            // version: version.into(),
            version: "HTTP/1.1".into(),
            status_code: status_code.into(),
            description: description.into(),
            headers: HashMap::new(),
            body: String::new(),
        }
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn body(mut self, body: &str) -> Self {
        self.body += &body;
        self
    }
}

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {}\r\n",
            self.version, self.status_code, self.description
        )?;
        for (key, value) in &self.headers {
            write!(f, "{key}: {value}\r\n")?;
        }
        write!(f, "\r\n{}", self.body)?;
        Ok(())
    }
}

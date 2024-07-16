use nom::HexDisplay;
use std::collections::HashMap;
use std::fmt::Write;
use tokio::io::{AsyncBufReadExt, AsyncReadExt};

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
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
    ) -> std::io::Result<Self> {
        let request_line = read_line(reader).await?;
        let mut splited = request_line.split(" ").into_iter();
        let method = splited.next().unwrap().into();
        let target = splited.next().unwrap().into();
        let version = splited.next().unwrap().into();

        let mut headers: HashMap<String, String> = HashMap::new();

        while let Some((key, value)) = read_line(reader).await?.split_once(":") {
            headers.insert(key.trim().into(), value.trim().into());
        }

        let length = headers
            .get("Content-Length")
            .map_or(0, |length| length.as_str().parse::<usize>().unwrap());
        let mut body = Vec::with_capacity(length);
        if length != 0 {
            reader.read_buf(&mut body).await?;
        }

        Ok(Self {
            method,
            target,
            version,
            headers,
            body,
        })
    }
}

#[derive(Clone)]
pub struct Response {
    pub version: String,
    pub status_code: usize,
    pub description: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
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
            body: Vec::new(),
        }
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn body(mut self, body: &[u8]) -> Self {
        self.body.extend(body);
        self
    }

    pub fn response_headers(&self) -> Result<String, std::fmt::Error> {
        let mut buf = String::new();
        write!(
            buf,
            "{} {} {}\r\n",
            self.version, self.status_code, self.description
        )?;
        for (key, value) in &self.headers {
            write!(buf, "{key}: {value}\r\n")?;
        }
        Ok(buf)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, std::fmt::Error> {
        let mut bytes = self.response_headers()?.as_bytes().to_vec();
        bytes.extend(b"\r\n");
        bytes.extend(&self.body);
        Ok(bytes)
    }
}

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\r\n{}",
            self.response_headers()?,
            String::from_utf8_lossy(&self.body)
        )
    }
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\r\n{}", self.response_headers()?, self.body.to_hex(8))
    }
}

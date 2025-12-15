use std::collections::HashMap;
use std::fmt::Write as _;
use std::num::ParseIntError;

use tokio::io::{AsyncBufRead, AsyncBufReadExt as _, AsyncReadExt as _};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Bad Request Line: {0}")]
    BadRequestLine(String),
    #[error("Bad Content-Length: {0}")]
    BadContentLength(ParseIntError),
    #[error("Content-Length is required")]
    ContentLengthRequired,
}

pub type Result<T> = std::result::Result<T, Error>;

async fn read_line(reader: &mut (impl AsyncBufRead + Unpin)) -> Result<String> {
    let mut buf = String::new();
    reader.read_line(&mut buf).await?;
    buf.pop(); // pop the `\n`
    if buf.ends_with("\r") {
        buf.pop(); // pop the `\r`
    }
    Ok(buf)
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub target: String, // TODO: struct URI
    pub version: String,
    pub headers: HashMap<String, String>, // TODO: struct Header
    pub body: Vec<u8>,
}

impl Request {
    #[inline]
    pub async fn try_from_buf_async(mut reader: impl AsyncBufRead + Unpin) -> Result<Self> {
        let request_line = read_line(&mut reader).await?;
        let [method, target, version] = request_line.split(" ").collect::<Vec<_>>()[..] else {
            return Err(Error::BadRequestLine(request_line));
        };

        let method = method.into();
        let target = percent_encoding::percent_decode_str(target)
            .decode_utf8_lossy()
            .to_string();
        let version = version.into();

        let mut headers: HashMap<String, String> = HashMap::new();

        while let Some((key, value)) = read_line(&mut reader).await?.split_once(":") {
            headers.insert(key.trim().into(), value.trim().into());
        }

        let length = match headers.get("Content-Length") {
            Some(length) => length.parse::<usize>().map_err(Error::BadContentLength)?,
            None => 0,
        };
        let mut body = Vec::with_capacity(length);
        if length != 0 {
            reader.read_exact(&mut body).await?;
        }

        Ok(Self {
            method,
            target,
            version,
            headers,
            body,
        })
    }

    #[inline]
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
    #[inline]
    pub fn new(status_code: impl ToString, description: impl ToString) -> Self {
        Self {
            version: "HTTP/1.1".into(),
            status_code: status_code.to_string(),
            description: description.to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    #[inline]
    pub fn with_header(mut self, key: impl ToString, value: impl ToString) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    #[inline]
    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    #[inline]
    pub fn response_line(&self) -> String {
        format!("{} {} {}", self.version, self.status_code, self.description)
    }

    #[inline]
    pub fn response_headers(&self) -> String {
        let mut buf = self.response_line();
        buf.push_str("\r\n");
        #[expect(clippy::iter_over_hash_type, reason = "headers order is not important")]
        #[expect(clippy::unwrap_used, reason = "format should never fail")]
        for (key, value) in &self.headers {
            write!(buf, "{key}: {value}\r\n").unwrap();
        }
        buf
    }

    #[inline]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.response_headers().as_bytes().to_vec();
        bytes.extend(b"\r\n");
        bytes.extend(&self.body);
        bytes
    }

    #[inline]
    pub fn ok() -> Self {
        Self::new(200, "OK")
    }

    #[inline]
    pub fn not_found(error: impl ToString) -> Self {
        Self::new(404, "Not Found").with_body(error.to_string().as_bytes())
    }

    #[inline]
    pub fn err(error: impl ToString) -> Self {
        Self::new(500, "Internal Server Error").with_body(error.to_string().as_bytes())
    }

    #[inline]
    pub fn plain(self, body: impl Into<Vec<u8>>) -> Self {
        self.with_header("Content-Type", "text/plain; charset=utf-8")
            .with_body(body)
    }

    #[inline]
    pub fn file(self, body: impl Into<Vec<u8>>) -> Self {
        self.with_header("Content-Type", "application/octet-reader")
            .with_body(body)
    }

    #[inline]
    pub fn html(self, body: impl Into<Vec<u8>>) -> Self {
        self.with_header("Content-Type", "text/html; charset=utf-8")
            .with_body(body)
    }
}

impl std::fmt::Display for Response {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\r\n{}",
            self.response_headers(),
            String::from_utf8_lossy(&self.body)
        )
    }
}

impl<E: std::error::Error> From<E> for Response {
    #[inline]
    fn from(error: E) -> Self {
        Self::err(error)
    }
}

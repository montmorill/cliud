use std::error::Error;
use std::fmt::{Display, Formatter, Result};

use smol_str::{SmolStr, ToSmolStr};

use crate::http::HeaderMap;

#[derive(Debug, Clone)]
pub struct Response {
    pub version: SmolStr,
    pub status_code: SmolStr,
    pub description: SmolStr,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Response {
    #[inline]
    pub fn new(status_code: impl ToSmolStr, description: impl ToSmolStr) -> Self {
        Self {
            version: "HTTP/1.1".into(),
            status_code: status_code.to_smolstr(),
            description: description.to_smolstr(),
            headers: HeaderMap::new(),
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
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = format!("{}\r\n{}\r\n", self.response_line(), self.headers).into_bytes();
        buf.extend_from_slice(&self.body);
        buf
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

impl Display for Response {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}\r\n{}\r\n{}",
            self.response_line(),
            self.headers,
            String::from_utf8_lossy(&self.body)
        )
    }
}

impl<E: Error> From<E> for Response {
    #[inline]
    fn from(error: E) -> Self {
        Self::err(error)
    }
}

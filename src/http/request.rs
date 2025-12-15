use std::fmt::{Display, Formatter};

use smol_str::SmolStr;
use tokio::io::{AsyncBufRead, AsyncReadExt as _};

use super::{Error, HeaderMap, Result, read_line};
use crate::http::Target;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: SmolStr,
    pub target: Target,
    pub version: SmolStr,
    pub headers: HeaderMap,
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
        let target = percent_encoding::percent_decode_str(target).decode_utf8_lossy().into();
        let version = version.into();

        let mut headers = HeaderMap::new();

        while let Some((key, value)) = read_line(&mut reader).await?.split_once(":") {
            headers.insert(key.trim(), value.trim_start());
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

    #[inline]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = format!("{}\r\n{}\r\n", self.request_line(), self.headers).into_bytes();
        buf.extend_from_slice(&self.body);
        buf
    }
}

impl Display for Request {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\r\n{}\r\n{}",
            self.request_line(),
            self.headers,
            String::from_utf8_lossy(&self.body)
        )
    }
}

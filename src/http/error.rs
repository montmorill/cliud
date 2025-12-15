use std::num::ParseIntError;

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

mod error;
mod header;
mod request;
mod response;
mod target;

pub use error::{Error, Result};
pub use header::HeaderMap;
pub use request::Request;
pub use response::Response;
pub use target::Target;
use tokio::io::{AsyncBufRead, AsyncBufReadExt as _};

async fn read_line(reader: &mut (impl AsyncBufRead + Unpin)) -> Result<String> {
    let mut buf = String::new();
    reader.read_line(&mut buf).await?;
    buf.pop(); // pop the `\n`
    if buf.ends_with("\r") {
        buf.pop(); // pop the `\r`
    }
    Ok(buf)
}

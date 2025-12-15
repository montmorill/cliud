use std::borrow::Cow;
use std::io::{Error, Write as _};

use async_trait::async_trait;
use flate2::Compression;
use flate2::write::{DeflateDecoder, DeflateEncoder, GzDecoder, GzEncoder, ZlibDecoder, ZlibEncoder};

use crate::http::{Request, Response};
use crate::middleware::{Middleware, Next};

type Result<T, E = Error> = std::result::Result<T, E>;

fn try_compress(encoding: &str, data: &[u8]) -> Result<Option<Vec<u8>>> {
    Ok(match encoding {
        "gzip" => {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data)?;
            Some(encoder.finish()?)
        }
        "deflate" => {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data)?;
            Some(encoder.finish()?)
        }
        "zlib" => {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data)?;
            Some(encoder.finish()?)
        }
        _ => None,
    })
}

fn try_decompress(encoding: &str, data: &[u8]) -> Result<Option<Vec<u8>>> {
    Ok(match encoding {
        "gzip" => {
            let mut decoder = GzDecoder::new(Vec::new());
            decoder.write_all(data)?;
            Some(decoder.finish()?)
        }
        "deflate" => {
            let mut decoder = DeflateDecoder::new(Vec::new());
            decoder.write_all(data)?;
            Some(decoder.finish()?)
        }
        "zlib" => {
            let mut decoder = ZlibDecoder::new(Vec::new());
            decoder.write_all(data)?;
            Some(decoder.finish()?)
        }
        _ => None,
    })
}

pub struct CompressMiddleware {
    pub min_size: usize,
}

#[async_trait]
impl<E: From<Error>> Middleware<E> for CompressMiddleware {
    #[inline]
    async fn call(&self, request: &Request, next: &dyn Next<E>) -> Result<Response, E> {
        let mut request = Cow::Borrowed(request);

        if let Some(encoding) = request.headers.get("Content-Encoding")
            && let Some(decompressed) = try_decompress(encoding, &request.body)?
        {
            let length = decompressed.len().to_string();
            let mut owned = request.into_owned();
            owned.body = decompressed;
            owned.headers.remove("Content-Encoding");
            owned.headers.insert("Content-Length", length);
            request = Cow::Owned(owned);
        }

        let mut response = next.call(&request).await?;

        if response.body.len() >= self.min_size
            && let Some(encodings) = request.headers.get("Accept-Encoding")
        {
            for encoding in encodings.split(",").map(str::trim) {
                if let Some(compressed) = try_compress(encoding, &response.body)? {
                    response.body = compressed;
                    return Ok(response.with_header("Content-Encoding", encoding));
                }
            }
        }
        Ok(response)
    }
}

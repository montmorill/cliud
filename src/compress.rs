use async_trait::async_trait;
use flate2::Compression;
use flate2::write::{DeflateDecoder, GzDecoder, ZlibDecoder};
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use std::io::Write;

use crate::http::{Request, Response};
use crate::middleware::{Middleware, Next};

fn try_compress(encoding: &str, data: &[u8]) -> std::io::Result<Option<Vec<u8>>> {
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

fn try_decompress(encoding: &str, data: &[u8]) -> std::io::Result<Option<Vec<u8>>> {
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

pub struct CompressMiddleware;

#[async_trait]
impl<E: From<std::io::Error>> Middleware<E> for CompressMiddleware {
    async fn call(&self, request: &mut Request, next: &dyn Next<E>) -> Result<Response, E> {
        if let Some(encoding) = request.headers.get("Content-Encoding") {
            if let Some(decompressed) = try_decompress(encoding, &request.body)? {
                let length = decompressed.len().to_string();
                request.body = decompressed;
                request.headers.remove("Content-Encoding");
                request.headers.insert("Content-Length".into(), length);
            }
        }

        let response = next.call(request).await?;

        if !response.body.is_empty()
            && let Some(encodings) = request.headers.get("Accept-Encoding")
        {
            for encoding in encodings.split(",").map(str::trim) {
                if let Some(compressed) = try_compress(encoding, &response.body)? {
                    return Ok(response
                        .header("Content-Encoding", encoding)
                        .body(&compressed));
                }
            }
        }
        Ok(response)
    }
}

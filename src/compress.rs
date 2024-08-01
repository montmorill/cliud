use flate2::write::{DeflateDecoder, GzDecoder, ZlibDecoder};
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::Compression;
use std::io::{Result, Write};

pub fn escape(raw: String) -> String {
    raw.replace("%20", " ")
}

pub fn try_compress(encoding: &str, data: &[u8]) -> Result<Option<Vec<u8>>> {
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

pub fn try_decompress(encoding: &str, data: &[u8]) -> Result<Option<Vec<u8>>> {
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

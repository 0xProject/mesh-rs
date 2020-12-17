use crate::prelude::*;
use std::io::{Error, ErrorKind, Result};

/// Read a Serde Serialize from an futures::io::AsyncRead.
///
/// This is difficult because there is no framing other than JSON succeeding to
/// parse. All we can do, it seems, is to repeatedly try parsing and wait for
/// more content to arrive if it fails.
///
/// TODO: Maximum size
///
/// TODO: Remove once Serde gains async support.
/// See <https://github.com/serde-rs/json/issues/316>
pub async fn read_json<R, T>(io: &mut R) -> Result<T>
where
    R: AsyncRead + Unpin + Send,
    T: for<'a> Deserialize<'a>,
{
    trace!("Attempting to read JSON from socket");
    let mut buffer = Vec::new();
    loop {
        // Read another (partial) block
        let mut block = [0_u8; 1024];
        let n = match io.read(&mut block).await {
            Ok(0) => {
                Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "Unexpected EOF while reading JSON.",
                ))
            }
            r => r,
        }?;
        buffer.extend(&block[..n]);
        trace!("Read {} more bytes, total {} in buffer", n, buffer.len());

        // Try to parse
        let result = serde_json::de::from_slice::<T>(&buffer);
        match result {
            Err(e) if e.is_eof() => {
                // Read some more
                continue;
            }
            _ => {}
        }

        if result.is_err() {
            error!("Could not parse: {}", String::from_utf8_lossy(&buffer));
        }
        return Ok(result?);
    }
}

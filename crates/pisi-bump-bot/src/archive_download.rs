use std::io::Read;

use sha1::{Digest, Sha1};
use ureq::Agent;

use crate::error::ArchiveDownloadError;
use crate::http_client::{SHARED_AGENT, USER_AGENT};

pub const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const CHUNK_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadResult {
    pub url: String,
    pub sha1: String,
    pub size_bytes: u64,
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn reject_oversize(size: u64, max_bytes: u64) -> Result<(), ArchiveDownloadError> {
    if size > max_bytes {
        return Err(ArchiveDownloadError::permanent(format!(
            "arşiv çok büyük ({size} bayt, sınır {max_bytes} bayt)"
        )));
    }
    Ok(())
}

fn is_rate_limit_forbidden(status: u16, header: impl Fn(&str) -> Option<String>) -> bool {
    status == 403
        && (header("x-ratelimit-remaining").as_deref() == Some("0")
            || header("retry-after").is_some())
}

fn status_failure(status: u16, header: impl Fn(&str) -> Option<String>) -> ArchiveDownloadError {
    let transient = status >= 500 || status == 429 || is_rate_limit_forbidden(status, header);
    let message = format!("indirme başarısız (HTTP {status})");
    if transient {
        ArchiveDownloadError::transient(message)
    } else {
        ArchiveDownloadError::permanent(message)
    }
}

fn transport_failure(url: &str, error: &ureq::Error) -> ArchiveDownloadError {
    match error {
        ureq::Error::Timeout(_) => ArchiveDownloadError::transient("indirme zaman aşımına uğradı"),
        other => ArchiveDownloadError::transient(format!("indirme başarısız ({url}: {other})")),
    }
}

fn read_failure(source: std::io::Error) -> ArchiveDownloadError {
    if source.kind() == std::io::ErrorKind::TimedOut {
        return ArchiveDownloadError::transient("indirme zaman aşımına uğradı");
    }
    ArchiveDownloadError::transient(format!("indirme başarısız ({source})"))
}

fn hash_stream(
    mut reader: impl Read,
    max_bytes: u64,
) -> Result<(String, u64), ArchiveDownloadError> {
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; CHUNK_BYTES];
    let mut total: u64 = 0;
    loop {
        let read = reader.read(&mut buffer).map_err(read_failure)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        reject_oversize(total, max_bytes)?;
        hasher.update(&buffer[..read]);
    }
    Ok((hex_encode(&hasher.finalize()), total))
}

pub fn download_sha1(url: &str) -> Result<DownloadResult, ArchiveDownloadError> {
    download_sha1_with_agent(&SHARED_AGENT, url, DEFAULT_MAX_BYTES)
}

pub fn download_sha1_with_agent(
    agent: &Agent,
    url: &str,
    max_bytes: u64,
) -> Result<DownloadResult, ArchiveDownloadError> {
    let mut response = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| transport_failure(url, &error))?;

    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let header = move |name: &str| {
        headers
            .iter()
            .find(|(key, _)| key.as_str().eq_ignore_ascii_case(name))
            .and_then(|(_, value)| value.to_str().ok())
            .map(str::to_string)
    };
    if status != 200 {
        return Err(status_failure(status, header));
    }

    if let Some(length) = response.body().content_length() {
        reject_oversize(length, max_bytes)?;
    }

    let (sha1, size) = hash_stream(response.body_mut().as_reader(), max_bytes)?;
    Ok(DownloadResult {
        url: url.to_string(),
        sha1,
        size_bytes: size,
    })
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_MAX_BYTES, download_sha1_with_agent, hash_stream};
    use crate::http_client::test_agent;
    use sha1::{Digest, Sha1};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn hex(bytes: &[u8]) -> String {
        let mut hasher = Sha1::new();
        hasher.update(bytes);
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn spawn_server(response: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(response);
            }
        });
        format!("http://{address}")
    }

    #[test]
    fn should_hash_archive_when_using_real_ureq_agent() {
        let payload = b"pisi-bump-bot loopback payload\n".repeat(1000);
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            payload.len()
        );
        let response: Vec<u8> = [header.into_bytes(), payload.clone()].concat();
        let base = spawn_server(Box::leak(response.into_boxed_slice()));
        let agent = test_agent();
        let result =
            download_sha1_with_agent(&agent, &format!("{base}/archive.tar.gz"), DEFAULT_MAX_BYTES)
                .unwrap();
        assert_eq!(
            (result.sha1, result.size_bytes),
            (hex(&payload), payload.len() as u64)
        );
    }

    #[test]
    fn should_raise_download_error_when_real_server_returns_404() {
        let base = spawn_server(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        let agent = test_agent();
        let error =
            download_sha1_with_agent(&agent, &format!("{base}/missing.tar.gz"), DEFAULT_MAX_BYTES)
                .unwrap_err();
        assert!(error.message.contains("HTTP 404"), "{}", error.message);
        assert!(!error.transient);
    }

    #[test]
    fn should_raise_download_error_when_port_is_closed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let agent = test_agent();
        let error = download_sha1_with_agent(
            &agent,
            &format!("http://{address}/archive.tar.gz"),
            DEFAULT_MAX_BYTES,
        )
        .unwrap_err();
        assert!(
            error.message.contains("indirme başarısız"),
            "{}",
            error.message
        );
        assert!(error.transient);
    }

    #[test]
    fn should_accept_payload_equal_to_limit() {
        let data = b"q".repeat(10);
        let (_, size) = hash_stream(std::io::Cursor::new(data), 10).unwrap();
        assert_eq!(size, 10);
    }

    #[test]
    fn should_reject_stream_exceeding_limit() {
        let data = b"z".repeat(50);
        let result = hash_stream(std::io::Cursor::new(data), 10);
        assert!(result.is_err());
        assert!(!result.unwrap_err().transient);
    }
}

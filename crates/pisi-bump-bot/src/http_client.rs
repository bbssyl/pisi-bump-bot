use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use ureq::Agent;
use ureq::config::RedirectAuthHeaders;
use ureq::http::HeaderMap;

use crate::error::FetchError;

pub const USER_AGENT: &str = "pisi-bump-bot";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const HASH_CHUNK_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(String::as_str)
    }
}

fn build_agent() -> Agent {
    let config = Agent::config_builder()
        .http_status_as_error(false)
        .redirect_auth_headers(RedirectAuthHeaders::SameHost)
        .timeout_connect(Some(REQUEST_TIMEOUT))
        .timeout_recv_response(Some(REQUEST_TIMEOUT))
        .build();
    config.into()
}

pub static SHARED_AGENT: LazyLock<Agent> = LazyLock::new(build_agent);

#[cfg(test)]
pub(crate) fn test_agent() -> Agent {
    build_agent()
}

fn lowercase_headers(headers: &HeaderMap) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (name, value) in headers.iter() {
        if let Ok(text) = value.to_str() {
            result.insert(name.as_str().to_lowercase(), text.to_string());
        }
    }
    result
}

pub fn fetch(url: &str, headers: &[(String, String)]) -> Result<HttpResponse, FetchError> {
    fetch_with_agent(&SHARED_AGENT, url, headers)
}

pub fn fetch_with_agent(
    agent: &Agent,
    url: &str,
    headers: &[(String, String)],
) -> Result<HttpResponse, FetchError> {
    let mut request = agent.get(url);
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let mut response = request
        .call()
        .map_err(|error| FetchError::from_transport(url, &error))?;
    let status = response.status().as_u16();
    let response_headers = lowercase_headers(response.headers());
    let body = response
        .body_mut()
        .read_to_vec()
        .map_err(|error| FetchError::from_transport(url, &error))?;
    Ok(HttpResponse {
        status,
        headers: response_headers,
        body,
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn stream_sha1(url: &str) -> Result<String, FetchError> {
    stream_sha1_with_agent(&SHARED_AGENT, url)
}

pub fn stream_sha1_with_agent(agent: &Agent, url: &str) -> Result<String, FetchError> {
    use sha1::{Digest, Sha1};
    use std::io::Read;

    let mut response = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| FetchError::from_transport(url, &error))?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; HASH_CHUNK_BYTES];
    let mut reader = response.body_mut().as_reader();
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| FetchError::from_io(url, &error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_encode(&hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::{build_agent, fetch_with_agent, stream_sha1_with_agent};

    fn spawn_server(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{address}")
    }

    #[test]
    fn should_return_status_and_lowercase_headers_when_server_responds_with_error_status() {
        let base = spawn_server(
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 2\r\nX-RateLimit-Remaining: 0\r\n\r\n{}",
        );
        let agent = build_agent();
        let response = fetch_with_agent(&agent, &format!("{base}/"), &[]).unwrap();
        assert_eq!(response.status, 403);
        assert_eq!(response.header("x-ratelimit-remaining"), Some("0"));
    }

    fn spawn_redirecting_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            for request_index in 0..2 {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]).to_lowercase();
                let response = if request_index == 0 {
                    "HTTP/1.1 301 Moved Permanently\r\nLocation: /moved\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
                } else {
                    let marker = if request.contains("authorization: bearer secret") {
                        "auth"
                    } else {
                        "none"
                    };
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n{marker}"
                    )
                };
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{address}")
    }

    #[test]
    fn should_keep_authorization_when_redirected_to_same_host() {
        let base = spawn_redirecting_server();
        let agent = build_agent();
        let headers = [("Authorization".to_string(), "Bearer secret".to_string())];

        let response = fetch_with_agent(&agent, &format!("{base}/renamed"), &headers).unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"auth");
    }

    #[test]
    fn should_fail_when_connection_is_refused() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let agent = build_agent();
        let result = fetch_with_agent(&agent, &format!("http://{address}/"), &[]);
        assert!(result.is_err());
    }

    #[test]
    fn should_hash_body_when_server_responds_with_payload() {
        let base = spawn_server("HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");
        let agent = build_agent();
        let digest = stream_sha1_with_agent(&agent, &format!("{base}/")).unwrap();
        assert_eq!(digest, sha1_hex(b"hello"));
    }

    fn sha1_hex(data: &[u8]) -> String {
        use sha1::{Digest, Sha1};
        let mut hasher = Sha1::new();
        hasher.update(data);
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

use crate::error::CommandError;
use serde_json::Value;
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    str::FromStr,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tokio_tungstenite::tungstenite::http::Uri;

const MAX_DISCOVERY_RESPONSE_BYTES: usize = 1024 * 1024;

pub fn validate_loopback_ws_url(raw: &str, port: u16) -> Result<String, CommandError> {
    let uri = Uri::from_str(raw).map_err(|error| {
        CommandError::new(
            "invalid_cdp_websocket_url",
            format!("无效的 CDP WebSocket 地址：{error}"),
        )
    })?;

    if uri.scheme_str() != Some("ws")
        || uri.host() != Some("127.0.0.1")
        || uri.port_u16() != Some(port)
    {
        return Err(CommandError::new(
            "non_loopback_cdp_endpoint",
            "拒绝非 127.0.0.1 或端口不匹配的 CDP WebSocket 地址。",
        ));
    }

    Ok(uri.to_string())
}

pub async fn get_local_json(port: u16, resource: &str) -> Result<Value, CommandError> {
    if port == 0 || !resource.starts_with('/') || resource.contains('\r') || resource.contains('\n')
    {
        return Err(CommandError::new(
            "invalid_cdp_request",
            "无效的本地 CDP discovery 请求。",
        ));
    }

    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    let mut stream = timeout(Duration::from_secs(2), TcpStream::connect(address))
        .await
        .map_err(|_| CommandError::new("cdp_connect_timeout", "连接本地 CDP 端口超时。"))?
        .map_err(|error| CommandError::new("cdp_connect_failed", error.to_string()))?;

    let request = format!(
        "GET {resource} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nAccept: application/json\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|error| CommandError::new("cdp_discovery_write_failed", error.to_string()))?;

    let response = timeout(Duration::from_secs(2), read_http_response(&mut stream))
        .await
        .map_err(|_| {
            CommandError::new("cdp_discovery_timeout", "读取本地 CDP discovery 响应超时。")
        })??;

    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| {
            CommandError::new("invalid_cdp_response", "本地 CDP 返回了无效 HTTP 响应。")
        })?;
    let header = std::str::from_utf8(&response[..header_end])
        .map_err(|_| CommandError::new("invalid_cdp_response", "本地 CDP 响应头不是 UTF-8。"))?;
    if !header.starts_with("HTTP/1.1 200 ") && !header.starts_with("HTTP/1.0 200 ") {
        return Err(CommandError::new(
            "cdp_discovery_status",
            format!(
                "本地 CDP discovery 返回：{}",
                header.lines().next().unwrap_or("未知状态")
            ),
        ));
    }

    serde_json::from_slice(&response[header_end + 4..])
        .map_err(|error| CommandError::new("invalid_cdp_json", error.to_string()))
}

async fn read_http_response(stream: &mut TcpStream) -> Result<Vec<u8>, CommandError> {
    let mut response = Vec::new();
    let mut expected_response_bytes = None;
    let mut buffer = [0_u8; 8192];

    loop {
        if let Some(expected) = expected_response_bytes {
            if response.len() >= expected {
                response.truncate(expected);
                return Ok(response);
            }
        }

        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|error| CommandError::new("cdp_discovery_read_failed", error.to_string()))?;
        if read == 0 {
            return Err(CommandError::new(
                "invalid_cdp_response",
                "本地 CDP 在返回完整 HTTP 响应前关闭了连接。",
            ));
        }
        response.extend_from_slice(&buffer[..read]);
        if response.len() > MAX_DISCOVERY_RESPONSE_BYTES {
            return Err(CommandError::new(
                "cdp_discovery_too_large",
                "本地 CDP discovery 响应过大。",
            ));
        }

        if expected_response_bytes.is_none() {
            let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let header = std::str::from_utf8(&response[..header_end]).map_err(|_| {
                CommandError::new("invalid_cdp_response", "本地 CDP 响应头不是 UTF-8。")
            })?;
            let content_length = header
                .lines()
                .skip(1)
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then_some(value.trim())
                })
                .ok_or_else(|| {
                    CommandError::new(
                        "invalid_cdp_response",
                        "本地 CDP discovery 响应缺少 Content-Length。",
                    )
                })?
                .parse::<usize>()
                .map_err(|_| {
                    CommandError::new(
                        "invalid_cdp_response",
                        "本地 CDP discovery 响应的 Content-Length 无效。",
                    )
                })?;
            let total = header_end
                .checked_add(4)
                .and_then(|body_start| body_start.checked_add(content_length))
                .filter(|total| *total <= MAX_DISCOVERY_RESPONSE_BYTES)
                .ok_or_else(|| {
                    CommandError::new("cdp_discovery_too_large", "本地 CDP discovery 响应过大。")
                })?;
            expected_response_bytes = Some(total);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_loopback_ws_url;

    #[test]
    fn accepts_matching_loopback_websocket() {
        assert!(validate_loopback_ws_url("ws://127.0.0.1:43123/devtools/page/1", 43123).is_ok());
    }

    #[test]
    fn rejects_non_loopback_websocket() {
        assert!(
            validate_loopback_ws_url("ws://192.168.1.10:43123/devtools/page/1", 43123).is_err()
        );
    }

    #[test]
    fn rejects_wrong_port() {
        assert!(validate_loopback_ws_url("ws://127.0.0.1:43124/devtools/page/1", 43123).is_err());
    }
}

#[tokio::test]
async fn reads_a_content_length_response_without_waiting_for_connection_close() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::{sleep, timeout},
    };

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind local test listener");
    let port = listener.local_addr().expect("local address").port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).await.expect("read request");
        let body = br#"[{"id":"page-1"}]"#;
        let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
                body.len(),
                std::str::from_utf8(body).expect("UTF-8 test body"),
            );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("write response");
        sleep(Duration::from_secs(3)).await;
    });

    let response = timeout(
        Duration::from_millis(500),
        get_local_json(port, "/json/list"),
    )
    .await;
    let value = response
        .expect("discovery must not wait for the server to close the socket")
        .expect("valid discovery response");
    assert_eq!(value[0]["id"], "page-1");
}

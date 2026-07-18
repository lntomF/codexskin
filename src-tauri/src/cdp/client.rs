use crate::{cdp::local_endpoint::validate_loopback_ws_url, error::CommandError};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::{broadcast, mpsc, oneshot, Mutex},
    time::timeout,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::{header::ORIGIN, HeaderValue, Request},
        Message,
    },
};

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, CommandError>>>>>;

fn local_cdp_handshake_request(
    websocket_url: &str,
    port: u16,
) -> Result<Request<()>, CommandError> {
    let websocket_url = validate_loopback_ws_url(websocket_url, port)?;
    let mut request = websocket_url
        .as_str()
        .into_client_request()
        .map_err(|error| CommandError::new("cdp_websocket_request_failed", error.to_string()))?;
    let origin = HeaderValue::from_str(&format!("http://127.0.0.1:{port}"))
        .map_err(|error| CommandError::new("cdp_websocket_request_failed", error.to_string()))?;
    request.headers_mut().insert(ORIGIN, origin);
    Ok(request)
}

#[derive(Clone, Debug)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
}

impl CdpEvent {
    fn from_wire(value: &Value) -> Option<Self> {
        if value.get("id").is_some() {
            return None;
        }
        Some(Self {
            method: value.get("method")?.as_str()?.to_string(),
            params: value.get("params").cloned().unwrap_or(Value::Null),
        })
    }
}

pub struct CdpClient {
    outbound: mpsc::Sender<Message>,
    pending: Pending,
    events: broadcast::Sender<CdpEvent>,
    connected: Arc<AtomicBool>,
    next_id: AtomicU64,
}

impl CdpClient {
    pub async fn connect(websocket_url: &str, port: u16) -> Result<Self, CommandError> {
        let request = local_cdp_handshake_request(websocket_url, port)?;
        let (stream, _) = connect_async(request).await.map_err(|error| {
            CommandError::new("cdp_websocket_connect_failed", error.to_string())
        })?;
        let (mut writer, mut reader) = stream.split();
        let (outbound, mut outbound_receiver) = mpsc::channel::<Message>(32);
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (events, _) = broadcast::channel::<CdpEvent>(64);
        let connected = Arc::new(AtomicBool::new(true));

        let write_pending = Arc::clone(&pending);
        let write_connected = Arc::clone(&connected);
        tokio::spawn(async move {
            while let Some(message) = outbound_receiver.recv().await {
                if writer.send(message).await.is_err() {
                    write_connected.store(false, Ordering::Release);
                    fail_pending(&write_pending, "CDP WebSocket 写入连接已断开。").await;
                    break;
                }
            }
        });

        let read_pending = Arc::clone(&pending);
        let read_events = events.clone();
        let read_connected = Arc::clone(&connected);
        tokio::spawn(async move {
            while let Some(result) = reader.next().await {
                let Ok(message) = result else { break };
                let Message::Text(text) = message else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if let Some(id) = value.get("id").and_then(Value::as_u64) {
                    if let Some(sender) = read_pending.lock().await.remove(&id) {
                        let _ = sender.send(Ok(value));
                    }
                    continue;
                }
                if let Some(event) = CdpEvent::from_wire(&value) {
                    let _ = read_events.send(event);
                }
            }
            read_connected.store(false, Ordering::Release);
            fail_pending(&read_pending, "CDP WebSocket 连接已断开。").await;
        });

        Ok(Self {
            outbound,
            pending,
            events,
            connected,
            next_id: AtomicU64::new(1),
        })
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, CommandError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let payload =
            serde_json::to_string(&json!({ "id": id, "method": method, "params": params }))
                .map_err(|error| {
                    CommandError::new("cdp_request_serialize_failed", error.to_string())
                })?;
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(id, sender);

        if self.outbound.send(Message::Text(payload)).await.is_err() {
            self.pending.lock().await.remove(&id);
            return Err(CommandError::new(
                "cdp_disconnected",
                "CDP WebSocket 已断开。",
            ));
        }

        match timeout(Duration::from_secs(5), receiver).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => {
                self.pending.lock().await.remove(&id);
                Err(CommandError::new(
                    "cdp_disconnected",
                    "CDP WebSocket 已断开。",
                ))
            }
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(CommandError::new(
                    "cdp_request_timeout",
                    format!("CDP 请求超时：{method}"),
                ))
            }
        }
    }
}

async fn fail_pending(pending: &Pending, detail: &str) {
    let pending_requests = std::mem::take(&mut *pending.lock().await);
    for (_, sender) in pending_requests {
        let _ = sender.send(Err(CommandError::new("cdp_disconnected", detail)));
    }
}

#[cfg(test)]
mod tests {
    use super::{local_cdp_handshake_request, CdpEvent};
    use serde_json::json;

    #[test]
    fn parses_page_load_event_without_an_id() {
        let event = CdpEvent::from_wire(&json!({
            "method": "Page.loadEventFired",
            "params": { "timestamp": 123.0 }
        }))
        .expect("CDP notification should be parsed");

        assert_eq!(event.method, "Page.loadEventFired");
        assert_eq!(event.params["timestamp"], 123.0);
    }

    #[test]
    fn ignores_response_messages_when_parsing_events() {
        assert!(CdpEvent::from_wire(&json!({ "id": 7, "result": {} })).is_none());
    }

    #[test]
    fn websocket_handshake_uses_the_exact_local_loopback_origin() {
        let request = local_cdp_handshake_request("ws://127.0.0.1:43123/devtools/page/1", 43123)
            .expect("valid local CDP handshake request");

        assert_eq!(
            request
                .headers()
                .get("origin")
                .and_then(|value| value.to_str().ok()),
            Some("http://127.0.0.1:43123")
        );
    }
}

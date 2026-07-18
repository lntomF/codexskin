use crate::{
    cdp::local_endpoint::{get_local_json, validate_loopback_ws_url},
    error::CommandError,
};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct PageTarget {
    pub id: String,
    pub url: String,
    pub websocket_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTarget {
    id: String,
    #[serde(rename = "type")]
    target_type: String,
    url: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    websocket_debugger_url: Option<String>,
}

pub async fn discover_page_targets(port: u16) -> Result<Vec<PageTarget>, CommandError> {
    let raw_targets: Vec<RawTarget> =
        serde_json::from_value(get_local_json(port, "/json/list").await?)
            .map_err(|error| CommandError::new("invalid_cdp_target_list", error.to_string()))?;

    raw_targets
        .into_iter()
        .filter(|target| target.target_type == "page")
        .filter_map(|target| {
            let websocket_url = target.websocket_debugger_url.clone()?;
            Some((target, websocket_url))
        })
        .map(|(target, websocket_url)| {
            Ok(PageTarget {
                id: target.id,
                url: target.url,
                websocket_url: validate_loopback_ws_url(&websocket_url, port)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod live_tests {
    use super::{discover_page_targets, RawTarget};
    use crate::{cdp::local_endpoint::get_local_json, models::CodexConnectionState, process};

    #[tokio::test]
    #[ignore = "requires running Codex Desktop with local CDP"]
    async fn prints_live_discovery_payload() {
        let status = process::inspect_running_codex();
        assert_eq!(status.state, CodexConnectionState::DebugPortDetected);
        let port = status.port.expect("debug port");
        let payload = get_local_json(port, "/json/list")
            .await
            .expect("local discovery payload");
        eprintln!("live payload: {payload:#}");
        let raw: Vec<RawTarget> = serde_json::from_value(payload.clone()).expect("raw targets");
        eprintln!("raw target count: {}", raw.len());
        let targets = discover_page_targets(port).await.expect("page discovery");
        eprintln!("page target count: {}", targets.len());
        assert!(!targets.is_empty(), "payload was: {payload:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::RawTarget;

    #[test]
    fn parses_chromium_web_socket_debugger_url_spelling() {
        let target: RawTarget = serde_json::from_str(
            r#"{"id":"page-1","type":"page","url":"app://-/index.html","webSocketDebuggerUrl":"ws://127.0.0.1:9222/devtools/page/page-1"}"#,
        )
        .expect("target JSON should deserialize");

        assert_eq!(
            target.websocket_debugger_url.as_deref(),
            Some("ws://127.0.0.1:9222/devtools/page/page-1")
        );
    }
}

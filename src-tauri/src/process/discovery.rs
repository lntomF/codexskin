use crate::models::{CodexConnectionState, CodexStatus};
use sysinfo::System;

pub fn inspect_running_codex() -> CodexStatus {
    let mut system = System::new_all();
    system.refresh_all();

    let candidates = system
        .processes()
        .values()
        .filter_map(|process| {
            let executable_path = process.exe().map(|path| path.display().to_string());
            let command_line = process
                .cmd()
                .iter()
                .map(|argument| argument.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let process_name = process.name().to_string();

            is_codex_desktop_process(
                &process_name,
                executable_path.as_deref().unwrap_or_default(),
                &command_line,
            )
            .then_some((executable_path, command_line))
        })
        .collect::<Vec<_>>();

    let Some((executable_path, command_line)) = candidates
        .iter()
        .find(|(_, command_line)| parse_debug_port(command_line).is_some())
        .or_else(|| candidates.first())
    else {
        return CodexStatus::not_running();
    };

    match parse_debug_port(command_line) {
        Some(port) => CodexStatus {
            state: CodexConnectionState::DebugPortDetected,
            port: Some(port),
            executable_path: executable_path.clone(),
            detail: format!("检测到 Codex Desktop，CDP 调试端口为 {port}。"),
        },
        None => CodexStatus {
            state: CodexConnectionState::RunningWithoutDebugPort,
            port: None,
            executable_path: executable_path.clone(),
            detail: "Codex Desktop 正在运行，但未以远程调试端口启动。请通过 Codex 正常退出后，再由 CodeSkin 启动。".into(),
        },
    }
}

pub(crate) fn is_codex_desktop_process(
    process_name: &str,
    executable_path: &str,
    command_line: &str,
) -> bool {
    let command_line = command_line.to_ascii_lowercase();
    if command_line.contains("--type=") || command_line.contains(" app-server") {
        return false;
    }

    if process_name.eq_ignore_ascii_case("Codex.exe") {
        return true;
    }

    process_name.eq_ignore_ascii_case("ChatGPT.exe")
        && executable_path
            .to_ascii_lowercase()
            .contains("\\windowsapps\\openai.codex_")
}

pub(crate) fn parse_debug_port(command_line: &str) -> Option<u16> {
    let marker = "--remote-debugging-port";
    let marker_position = command_line.find(marker)?;
    let after_marker = &command_line[marker_position + marker.len()..];
    let value = after_marker
        .strip_prefix('=')
        .or_else(|| after_marker.strip_prefix(' '))?
        .trim_start_matches(|character: char| character == '"' || character.is_whitespace())
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();

    let port = value.parse::<u16>().ok()?;
    (port != 0).then_some(port)
}

#[cfg(test)]
mod tests {
    use super::{is_codex_desktop_process, parse_debug_port};

    #[test]
    fn parses_equals_style_debug_port() {
        assert_eq!(
            parse_debug_port("Codex.exe --remote-debugging-port=43123"),
            Some(43123)
        );
    }

    #[test]
    fn parses_spaced_style_debug_port() {
        assert_eq!(
            parse_debug_port("Codex.exe --remote-debugging-port 43124"),
            Some(43124)
        );
    }

    #[test]
    fn rejects_invalid_port() {
        assert_eq!(
            parse_debug_port("Codex.exe --remote-debugging-port=0"),
            None
        );
    }

    #[test]
    fn recognizes_store_chatgpt_root_process() {
        assert!(is_codex_desktop_process(
            "ChatGPT.exe",
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.715.2305.0_x64__2p2nqsd0c76g0\app\ChatGPT.exe",
            r#""C:\Program Files\WindowsApps\OpenAI.Codex_26.715.2305.0_x64__2p2nqsd0c76g0\app\ChatGPT.exe""#,
        ));
    }

    #[test]
    fn rejects_store_chatgpt_renderer_process() {
        assert!(!is_codex_desktop_process(
            "ChatGPT.exe",
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.715.2305.0_x64__2p2nqsd0c76g0\app\ChatGPT.exe",
            "ChatGPT.exe --type=renderer --user-data-dir=C:\\Users\\DUIE\\AppData\\Roaming\\Codex",
        ));
    }

    #[test]
    fn rejects_codex_app_server_sidecar() {
        assert!(!is_codex_desktop_process(
            "codex.exe",
            r"C:\Users\DUIE\.vscode\extensions\openai.chatgpt\bin\codex.exe",
            "codex.exe -c features.code_mode_host=true app-server --analytics-default-enabled",
        ));
    }
}

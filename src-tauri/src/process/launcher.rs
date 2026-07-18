use crate::error::CommandError;
use std::{
    net::{Ipv4Addr, TcpListener},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn find_available_loopback_port() -> Result<u16, CommandError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| CommandError::new("port_allocation_failed", error.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|error| CommandError::new("port_allocation_failed", error.to_string()))?
        .port();
    drop(listener);
    Ok(port)
}

pub fn find_installed_codex() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let root = PathBuf::from(local_app_data);
        candidates.push(root.join("Programs").join("Codex").join("Codex.exe"));
        candidates.push(root.join("Programs").join("Codex").join("ChatGPT.exe"));
        candidates.push(root.join("Codex").join("Codex.exe"));
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        let root = PathBuf::from(program_files);
        candidates.push(root.join("Codex").join("Codex.exe"));
        candidates.extend(find_windows_store_codex(&root.join("WindowsApps")));
    }

    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn find_windows_store_codex(windows_apps: &Path) -> Vec<PathBuf> {
    // WindowsApps is normally ACL-protected, so direct enumeration is only a
    // best effort. Query the registered package afterwards without relying on
    // Node.js or any network service.
    let mut candidates = std::fs::read_dir(windows_apps)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("OpenAI.Codex_"))
        })
        .flat_map(store_executable_candidates)
        .filter(|candidate| candidate.is_file())
        .collect::<Vec<_>>();

    candidates.extend(find_windows_store_codex_via_package_manager());
    candidates.sort();
    candidates.dedup();
    candidates.reverse();
    candidates
}

fn find_windows_store_codex_via_package_manager() -> Vec<PathBuf> {
    const SCRIPT: &str =
        "Get-AppxPackage -Name 'OpenAI.Codex' | ForEach-Object { $_.InstallLocation }";
    let Ok(output) = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            SCRIPT,
        ])
        .stdin(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .flat_map(store_executable_candidates)
        .filter(|candidate| candidate.is_file())
        .collect()
}

fn store_executable_candidates(package_root: PathBuf) -> [PathBuf; 2] {
    [
        package_root.join("app").join("ChatGPT.exe"),
        package_root.join("app").join("Codex.exe"),
    ]
}

fn launch_arguments(port: u16) -> [String; 3] {
    [
        format!("--remote-debugging-port={port}"),
        "--remote-debugging-address=127.0.0.1".to_string(),
        format!("--remote-allow-origins=http://127.0.0.1:{port}"),
    ]
}

pub fn launch_codex(path: &Path, port: u16) -> Result<(), CommandError> {
    if port == 0 {
        return Err(CommandError::new(
            "invalid_port",
            "CDP port must be non-zero.",
        ));
    }
    if !path.is_file() {
        return Err(CommandError::new(
            "codex_executable_not_found",
            "指定的 Codex Desktop 可执行文件不存在。",
        ));
    }

    Command::new(path)
        .args(launch_arguments(port))
        .spawn()
        .map_err(|error| CommandError::new("codex_launch_failed", error.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::launch_arguments;

    #[test]
    fn permits_only_the_exact_loopback_origin_for_the_selected_debug_port() {
        let arguments = launch_arguments(54_742);

        assert_eq!(arguments[0], "--remote-debugging-port=54742");
        assert_eq!(arguments[1], "--remote-debugging-address=127.0.0.1");
        assert_eq!(
            arguments[2],
            "--remote-allow-origins=http://127.0.0.1:54742"
        );
        assert!(!arguments.iter().any(|argument| argument.contains('*')));
    }
}

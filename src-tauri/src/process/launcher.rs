use crate::error::CommandError;
use std::{
    net::{Ipv4Addr, TcpListener},
    path::{Path, PathBuf},
    process::Command,
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
    let Ok(entries) = std::fs::read_dir(windows_apps) else {
        return Vec::new();
    };

    let mut candidates = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("OpenAI.Codex_"))
        })
        .map(|package| package.join("app").join("ChatGPT.exe"))
        .filter(|candidate| candidate.is_file())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.reverse();
    candidates
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
        .arg(format!("--remote-debugging-port={port}"))
        .spawn()
        .map_err(|error| CommandError::new("codex_launch_failed", error.to_string()))?;

    Ok(())
}

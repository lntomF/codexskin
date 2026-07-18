use crate::error::CommandError;
use std::{
    ffi::c_void,
    net::{Ipv4Addr, TcpListener},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexLaunchTarget {
    DesktopExecutable {
        executable: PathBuf,
    },
    StorePackage {
        executable: PathBuf,
        app_user_model_id: String,
    },
}

impl CodexLaunchTarget {
    pub fn executable_path(&self) -> &Path {
        match self {
            Self::DesktopExecutable { executable } | Self::StorePackage { executable, .. } => {
                executable
            }
        }
    }
}

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

pub fn find_installed_codex_launch_target() -> Result<Option<CodexLaunchTarget>, CommandError> {
    find_installed_codex()
        .map(classify_codex_launch_target)
        .transpose()
}

pub fn classify_codex_launch_target(path: PathBuf) -> Result<CodexLaunchTarget, CommandError> {
    if is_windows_store_path(&path) {
        let app_user_model_id = store_app_user_model_id(&path).ok_or_else(|| {
            CommandError::new(
                "codex_store_metadata_unavailable",
                "检测到 Microsoft Store 版 Codex，但无法读取其应用激活标识。请从 Microsoft Store 更新或重新安装 Codex 后重试。",
            )
        })?;
        return Ok(CodexLaunchTarget::StorePackage {
            executable: path,
            app_user_model_id,
        });
    }

    Ok(CodexLaunchTarget::DesktopExecutable { executable: path })
}

fn is_windows_store_path(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("WindowsApps")
    })
}

fn store_app_user_model_id(executable: &Path) -> Option<String> {
    let package_root = executable.ancestors().find(|candidate| {
        candidate
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("OpenAI.Codex_"))
    })?;
    let package_folder = package_root.file_name()?.to_str()?;
    let publisher_id = package_folder.rsplit('_').next()?;
    let manifest = std::fs::read_to_string(package_root.join("AppxManifest.xml")).ok()?;
    let package_name = manifest_attribute(&manifest, "Identity", "Name")?;
    let application_id = manifest_attribute(&manifest, "Application", "Id")?;
    Some(format!("{package_name}_{publisher_id}!{application_id}"))
}

fn manifest_attribute(manifest: &str, element: &str, attribute: &str) -> Option<String> {
    let tag = manifest
        .split('<')
        .find(|fragment| {
            fragment.strip_prefix(element).is_some_and(|suffix| {
                suffix.chars().next().is_some_and(|character| {
                    character.is_ascii_whitespace() || character == '>' || character == '/'
                })
            })
        })?
        .split_once('>')?
        .0;
    let attribute_marker = format!(r#"{attribute}=""#);
    let value = tag.split_once(&attribute_marker)?.1;
    Some(value.split_once('\"')?.0.to_string())
}

fn find_windows_store_codex(windows_apps: &Path) -> Vec<PathBuf> {
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

pub fn launch_codex(target: &CodexLaunchTarget, port: u16) -> Result<(), CommandError> {
    if port == 0 {
        return Err(CommandError::new(
            "invalid_port",
            "CDP port must be non-zero.",
        ));
    }
    if !target.executable_path().is_file() {
        return Err(CommandError::new(
            "codex_executable_not_found",
            "指定的 Codex Desktop 可执行文件不存在。",
        ));
    }

    let arguments = launch_arguments(port);
    match target {
        CodexLaunchTarget::DesktopExecutable { executable } => Command::new(executable)
            .args(arguments)
            .spawn()
            .map_err(|error| CommandError::new("codex_launch_failed", error.to_string()))
            .map(|_| ()),
        CodexLaunchTarget::StorePackage {
            app_user_model_id, ..
        } => activate_store_package(app_user_model_id, &arguments.join(" ")),
    }
}

#[cfg(windows)]
fn activate_store_package(app_user_model_id: &str, arguments: &str) -> Result<(), CommandError> {
    const COINIT_MULTITHREADED: u32 = 0;
    const CLSCTX_INPROC_SERVER: u32 = 1;
    const CLSID_APPLICATION_ACTIVATION_MANAGER: Guid = Guid {
        data1: 0x45BA127D,
        data2: 0x10A8,
        data3: 0x46EA,
        data4: [0x8A, 0xB7, 0x56, 0xEA, 0x90, 0x78, 0x94, 0x3C],
    };
    const IID_APPLICATION_ACTIVATION_MANAGER: Guid = Guid {
        data1: 0x2E941141,
        data2: 0x7F97,
        data3: 0x4756,
        data4: [0xBA, 0x1D, 0x9D, 0xEC, 0xDE, 0x89, 0x4A, 0x3D],
    };

    let app_id = wide_null(app_user_model_id);
    let activation_arguments = wide_null(arguments);
    unsafe {
        let initialized = co_initialize_ex(std::ptr::null_mut(), COINIT_MULTITHREADED);
        if initialized < 0 {
            return Err(CommandError::new(
                "codex_store_activation_failed",
                format!(
                    "初始化 Windows Store 应用激活组件失败（HRESULT 0x{:08X}）。",
                    initialized as u32
                ),
            ));
        }

        let result = (|| {
            let mut manager: *mut ApplicationActivationManager = std::ptr::null_mut();
            let create_result = co_create_instance(
                &CLSID_APPLICATION_ACTIVATION_MANAGER,
                std::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_APPLICATION_ACTIVATION_MANAGER,
                (&mut manager as *mut *mut ApplicationActivationManager).cast::<*mut c_void>(),
            );
            if create_result < 0 || manager.is_null() {
                return Err(CommandError::new(
                    "codex_store_activation_failed",
                    format!(
                        "创建 Windows Store 应用激活器失败（HRESULT 0x{:08X}）。",
                        create_result as u32
                    ),
                ));
            }

            let mut process_id = 0_u32;
            let activation_result = ((*(*manager).vtable).activate_application)(
                manager,
                app_id.as_ptr(),
                activation_arguments.as_ptr(),
                0,
                &mut process_id,
            );
            ((*(*manager).vtable).release)(manager);
            if activation_result < 0 {
                return Err(CommandError::new(
                    "codex_store_activation_failed",
                    format!(
                        "无法激活 Microsoft Store 版 Codex（HRESULT 0x{:08X}）。",
                        activation_result as u32
                    ),
                ));
            }
            if process_id == 0 {
                return Err(CommandError::new(
                    "codex_store_activation_failed",
                    "Microsoft Store 已接受启动请求，但未返回 Codex 进程标识。",
                ));
            }
            Ok(())
        })();
        co_uninitialize();
        result
    }
}

#[cfg(not(windows))]
fn activate_store_package(_: &str, _: &str) -> Result<(), CommandError> {
    Err(CommandError::new(
        "codex_store_activation_unsupported",
        "Microsoft Store 版 Codex 只能在 Windows 上启动。",
    ))
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[cfg(windows)]
#[repr(C)]
struct ApplicationActivationManager {
    vtable: *const ApplicationActivationManagerVtable,
}

#[cfg(windows)]
#[repr(C)]
struct ApplicationActivationManagerVtable {
    query_interface: unsafe extern "system" fn(
        *mut ApplicationActivationManager,
        *const Guid,
        *mut *mut c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut ApplicationActivationManager) -> u32,
    release: unsafe extern "system" fn(*mut ApplicationActivationManager) -> u32,
    activate_application: unsafe extern "system" fn(
        *mut ApplicationActivationManager,
        *const u16,
        *const u16,
        u32,
        *mut u32,
    ) -> i32,
}

#[cfg(windows)]
#[link(name = "ole32")]
extern "system" {
    #[link_name = "CoInitializeEx"]
    fn co_initialize_ex(reserved: *mut c_void, co_init: u32) -> i32;
    #[link_name = "CoUninitialize"]
    fn co_uninitialize();
    #[link_name = "CoCreateInstance"]
    fn co_create_instance(
        clsid: *const Guid,
        outer: *mut c_void,
        context: u32,
        iid: *const Guid,
        object: *mut *mut c_void,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::{classify_codex_launch_target, launch_arguments, CodexLaunchTarget};
    use std::{fs, path::PathBuf};

    #[test]
    fn classifies_a_regular_installation_as_a_direct_executable_launch() {
        let executable = PathBuf::from(r"C:\Users\Alice\AppData\Local\Programs\Codex\Codex.exe");
        let target = classify_codex_launch_target(executable.clone())
            .expect("regular executable should be launchable");
        assert_eq!(target, CodexLaunchTarget::DesktopExecutable { executable });
    }

    #[test]
    fn classifies_windows_store_codex_as_package_activation_not_direct_execution() {
        let root = std::env::temp_dir().join(format!(
            "codeskin-launcher-store-test-{}",
            std::process::id()
        ));
        let package = root
            .join("WindowsApps")
            .join("OpenAI.Codex_26.715.4045.0_x64__2p2nqsd0c76g0");
        let executable = package.join("app").join("ChatGPT.exe");
        fs::create_dir_all(executable.parent().expect("app directory")).unwrap();
        fs::write(&executable, b"placeholder").unwrap();
        fs::write(
            package.join("AppxManifest.xml"),
            r#"<Package><Identity Name="OpenAI.Codex"/><Applications><Application Id="App"/></Applications></Package>"#,
        )
        .unwrap();

        let target = classify_codex_launch_target(executable.clone())
            .expect("Store package metadata should produce an activation target");
        assert_eq!(
            target,
            CodexLaunchTarget::StorePackage {
                executable,
                app_user_model_id: "OpenAI.Codex_2p2nqsd0c76g0!App".into(),
            }
        );
        fs::remove_dir_all(root).unwrap();
    }

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

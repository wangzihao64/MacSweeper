use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedItem {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallReport {
    pub removed: Vec<String>,
    pub failed: Vec<FailedItem>,
    pub quit_attempted: bool,
}

#[tauri::command]
pub async fn uninstall(
    app_path: String,
    bundle_id: String,
    extra_paths: Vec<String>,
    mode: String,
) -> Result<UninstallReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        uninstall_impl(app_path, bundle_id, extra_paths, mode)
    })
    .await
    .map_err(|e| format!("卸载任务异常：{e}"))?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTarget {
    pub app_path: String,
    pub bundle_id: String,
    #[serde(default)]
    pub extra_paths: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub app_path: String,
    pub app_name: String,
    pub report: Option<UninstallReport>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn uninstall_many(
    targets: Vec<BatchTarget>,
    mode: String,
) -> Result<Vec<BatchResult>, String> {
    tauri::async_runtime::spawn_blocking(move || batch_impl(targets, mode))
        .await
        .map_err(|e| format!("批量卸载任务异常：{e}"))
}

fn batch_impl(targets: Vec<BatchTarget>, mode: String) -> Vec<BatchResult> {
    targets
        .into_iter()
        .map(|target| {
            let app_name = Path::new(&target.app_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&target.app_path)
                .to_string();
            match uninstall_impl(
                target.app_path.clone(),
                target.bundle_id,
                target.extra_paths,
                mode.clone(),
            ) {
                Ok(report) => BatchResult {
                    app_path: target.app_path,
                    app_name,
                    report: Some(report),
                    error: None,
                },
                Err(error) => BatchResult {
                    app_path: target.app_path,
                    app_name,
                    report: None,
                    error: Some(error),
                },
            }
        })
        .collect()
}

fn uninstall_impl(
    app_path: String,
    bundle_id: String,
    extra_paths: Vec<String>,
    mode: String,
) -> Result<UninstallReport, String> {
    let app = PathBuf::from(&app_path);
    if !app.is_absolute() || app.extension().and_then(|e| e.to_str()) != Some("app") {
        return Err("无效的应用路径".to_string());
    }
    if !app.exists() {
        return Err("应用不存在，可能已被删除".to_string());
    }
    if app_path.starts_with("/System/") {
        return Err("系统 App 受系统完整性保护，无法卸载".to_string());
    }

    let app_canon = std::fs::canonicalize(&app).unwrap_or_else(|_| app.clone());
    if let Ok(current) = std::env::current_exe() {
        let current = std::fs::canonicalize(&current).unwrap_or(current);
        if current.starts_with(&app_canon) {
            return Err("不能卸载正在运行的 MacSweeper 自身".to_string());
        }
    }

    let permanent = match mode.as_str() {
        "trash" => false,
        "delete" => true,
        _ => return Err("未知的删除模式".to_string()),
    };

    let quit_attempted = quit_app(&app, &bundle_id);

    let home = crate::util::home_dir().ok_or("无法定位用户目录")?;
    let library = std::fs::canonicalize(home.join("Library"))
        .unwrap_or_else(|_| home.join("Library"));

    let mut removed: Vec<String> = Vec::new();
    let mut failed: Vec<FailedItem> = Vec::new();

    let mut targets: Vec<PathBuf> = vec![app.clone()];
    for raw in extra_paths {
        match resolve_under_library(&raw, &library) {
            Ok(resolved) => {
                if resolved == app_canon {
                    continue;
                }
                if targets
                    .iter()
                    .any(|t| resolved.starts_with(t) && resolved != *t)
                {
                    continue;
                }
                if targets.contains(&resolved) {
                    continue;
                }
                targets.push(resolved);
            }
            Err(err) => failed.push(FailedItem {
                path: raw,
                error: err,
            }),
        }
    }

    for target in &targets {
        match delete_path(target, permanent) {
            Ok(()) => removed.push(target.to_string_lossy().to_string()),
            Err(err) => failed.push(FailedItem {
                path: target.to_string_lossy().to_string(),
                error: friendly_error(&err),
            }),
        }
    }

    Ok(UninstallReport {
        removed,
        failed,
        quit_attempted,
    })
}

fn resolve_under_library(raw: &str, library_canon: &Path) -> Result<PathBuf, String> {
    let pb = PathBuf::from(raw);
    if !pb.is_absolute() {
        return Err("路径无效".to_string());
    }
    let parent = pb
        .parent()
        .ok_or_else(|| "路径无效".to_string())?;
    let Some(name) = pb.file_name() else {
        return Err("路径无效".to_string());
    };
    let parent_canon =
        std::fs::canonicalize(parent).map_err(|_| "文件不存在".to_string())?;
    let resolved = parent_canon.join(name);
    if !resolved.starts_with(library_canon) {
        return Err("出于安全限制，仅允许删除用户资源库中的文件".to_string());
    }
    if std::fs::symlink_metadata(&resolved).is_err() {
        return Err("文件不存在".to_string());
    }
    Ok(resolved)
}

fn delete_path(path: &Path, permanent: bool) -> std::io::Result<()> {
    if permanent {
        let meta = std::fs::symlink_metadata(path)?;
        if meta.is_dir() && !meta.file_type().is_symlink() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        }
    } else {
        trash_path(path).map_err(|e| std::io::Error::other(e.to_string()))
    }
}

fn trash_path(path: &Path) -> Result<(), trash::Error> {
    // The Finder backend drives osascript, which triggers an Automation
    // permission prompt and blocks; NSFileManager needs no extra consent.
    use trash::macos::{DeleteMethod, TrashContextExtMacos};

    let mut ctx = trash::TrashContext::default();
    ctx.set_delete_method(DeleteMethod::NsFileManager);
    ctx.delete_all([path])
}

fn friendly_error(err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::PermissionDenied => {
            "没有权限删除（该 App 可能由管理员安装，或正被系统保护）".to_string()
        }
        std::io::ErrorKind::NotFound => "文件不存在".to_string(),
        _ => err.to_string(),
    }
}

fn is_valid_bundle_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

fn running_pids(app: &Path) -> Vec<u32> {
    let prefix = format!("{}/Contents/", app.to_string_lossy());
    let Ok(output) = Command::new("/bin/ps")
        .args(["-axo", "pid=,command="])
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (pid, cmd) = line.split_once(char::is_whitespace)?;
            if cmd.contains(&prefix) {
                pid.parse::<u32>().ok()
            } else {
                None
            }
        })
        .collect()
}

fn quit_app(app: &Path, bundle_id: &str) -> bool {
    if running_pids(app).is_empty() {
        return false;
    }

    if is_valid_bundle_id(bundle_id) {
        request_quit(bundle_id);
        if wait_until_quit(app, Duration::from_secs(3)) {
            return true;
        }
    }

    for pid in running_pids(app) {
        let _ = Command::new("/bin/kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    if wait_until_quit(app, Duration::from_secs(2)) {
        return true;
    }

    for pid in running_pids(app) {
        let _ = Command::new("/bin/kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
    std::thread::sleep(Duration::from_millis(300));
    true
}

fn request_quit(bundle_id: &str) {
    // osascript blocks indefinitely if macOS shows the Automation consent
    // prompt, so cap the wait and fall through to signals.
    let script = format!(
        "with timeout of 2 seconds\n tell application id \"{}\" to quit\nend timeout",
        bundle_id
    );
    let Ok(mut child) = Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
}

fn wait_until_quit(app: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if running_pids(app).is_empty() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    running_pids(app).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_app_path() {
        let err = uninstall_impl(
            "/tmp/not-an-app.txt".to_string(),
            String::new(),
            vec![],
            "delete".to_string(),
        );
        assert!(err.is_err());
    }

    #[test]
    fn rejects_system_app() {
        let err = uninstall_impl(
            "/System/Applications/Calculator.app".to_string(),
            "com.apple.calculator".to_string(),
            vec![],
            "trash".to_string(),
        );
        assert!(err.is_err());
    }

    #[test]
    fn batch_reports_error_per_app() {
        let results = batch_impl(
            vec![BatchTarget {
                app_path: "/tmp/macsweeper-missing.app".to_string(),
                bundle_id: String::new(),
                extra_paths: Vec::new(),
            }],
            "delete".to_string(),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_name, "macsweeper-missing");
        assert!(results[0].report.is_none());
        assert!(results[0].error.is_some());
    }

    #[test]
    fn rejects_leftovers_outside_library() {
        let home = crate::util::home_dir().unwrap();
        let lib = home.join("Library");
        let err = resolve_under_library("/Users/Shared/evil", &lib);
        assert!(err.is_err());
        let err2 = resolve_under_library(&home.join("Documents").to_string_lossy(), &lib);
        assert!(err2.is_err());
    }

    #[test]
    fn validates_bundle_id() {
        assert!(is_valid_bundle_id("com.example.app"));
        assert!(!is_valid_bundle_id("com.example.\"app\""));
        assert!(!is_valid_bundle_id(""));
    }

    #[test]
    fn permanent_delete_removes_file_and_dir() {
        let base = std::env::temp_dir().join(format!(
            "macsweeper-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(base.join("sub")).unwrap();
        std::fs::write(base.join("a.txt"), b"x").unwrap();
        delete_path(&base, true).unwrap();
        assert!(!base.exists());
    }
}

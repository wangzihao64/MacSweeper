use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use icns::IconFamily;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

static CACHE: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[tauri::command]
pub async fn app_icon(app_path: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || icon_impl(&app_path))
        .await
        .ok()
        .flatten()
}

fn icon_impl(app_path: &str) -> Option<String> {
    if let Ok(cache) = CACHE.lock() {
        if let Some(hit) = cache.get(app_path) {
            return hit.clone();
        }
    }
    let result = extract_icon(app_path);
    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(app_path.to_string(), result.clone());
    }
    result
}

fn extract_icon(app_path: &str) -> Option<String> {
    let bundle = Path::new(app_path);
    let resources = bundle.join("Contents").join("Resources");

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(dict) = plist::Value::from_file(bundle.join("Contents").join("Info.plist"))
        .ok()
        .and_then(|v| v.into_dictionary())
    {
        for key in ["CFBundleIconFile", "CFBundleIconName"] {
            if let Some(raw) = dict.get(key).and_then(|v| v.as_string()) {
                let mut icon_path = resources.join(raw);
                if icon_path.extension().is_none() {
                    icon_path.set_extension("icns");
                }
                if icon_path.is_file() {
                    candidates.push(icon_path);
                }
            }
        }
    }

    if let Ok(entries) = std::fs::read_dir(&resources) {
        let mut icns_files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .map(|e| e.eq_ignore_ascii_case("icns"))
                    .unwrap_or(false)
            })
            .collect();
        icns_files.sort_by_key(|p| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if name.contains("appicon") {
                0
            } else {
                1
            }
        });
        candidates.extend(icns_files);
    }

    let mut seen = std::collections::HashSet::new();
    for icns_path in candidates {
        if !seen.insert(icns_path.clone()) {
            continue;
        }
        if let Some(url) = icns_to_data_url(&icns_path) {
            return Some(url);
        }
    }

    // 部分 App（尤其是系统 App）的图标只存在 Assets.car 中，
    // 用 Quick Look 缩略图作为兜底。
    qlmanage_icon(bundle)
}

fn icns_to_data_url(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let family = IconFamily::read(BufReader::new(file)).ok()?;

    let mut best: Option<(u32, Vec<u8>)> = None;
    for icon_type in family.available_icons() {
        let Ok(image) = family.get_icon_with_type(icon_type) else {
            continue;
        };
        let width = image.width();
        let height = image.height();
        if width == 0 || height == 0 {
            continue;
        }
        let mut png_data = Vec::new();
        if image.write_png(&mut png_data).is_err() {
            continue;
        }

        let dim = width.max(height);
        let better = match &best {
            None => true,
            Some((best_dim, _)) => {
                (dim <= 512 && (*best_dim > 512 || dim > *best_dim))
                    || (*best_dim > 512 && dim < *best_dim)
            }
        };
        if better {
            best = Some((dim, png_data));
        }
    }

    let (_, bytes) = best?;
    Some(format!("data:image/png;base64,{}", B64.encode(bytes)))
}

static QL_COUNTER: AtomicU64 = AtomicU64::new(0);

fn qlmanage_icon(bundle: &Path) -> Option<String> {
    let dir = std::env::temp_dir().join(format!(
        "macsweeper-thumb-{}-{}",
        std::process::id(),
        QL_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).ok()?;

    // qlmanage occasionally never exits (e.g. for script-only bundles),
    // so cap the wait instead of blocking an icon worker forever.
    let mut child = std::process::Command::new("/usr/bin/qlmanage")
        .arg("-t")
        .arg("-s")
        .arg("256")
        .arg("-o")
        .arg(&dir)
        .arg(bundle)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    let ok = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(100))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };

    let mut result = None;
    if ok {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path
                    .extension()
                    .map(|e| e.eq_ignore_ascii_case("png"))
                    .unwrap_or(false)
                {
                    if let Ok(bytes) = std::fs::read(&path) {
                        result = Some(format!("data:image/png;base64,{}", B64.encode(bytes)));
                        break;
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_icon_for_some_app() {
        let candidates = [
            "/System/Applications/Calculator.app",
            "/System/Applications/TextEdit.app",
            "/Applications/Safari.app",
            "/Applications/Google Chrome.app",
        ];
        for path in candidates {
            if !Path::new(path).exists() {
                continue;
            }
            if let Some(url) = extract_icon(path) {
                assert!(url.starts_with("data:image/png;base64,"));
                return;
            }
        }
    }
}

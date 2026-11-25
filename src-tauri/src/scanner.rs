use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub bundle_id: String,
    pub version: String,
    pub path: String,
    pub executable: String,
    pub is_system: bool,
}

#[tauri::command]
pub async fn scan_apps() -> Vec<AppInfo> {
    tauri::async_runtime::spawn_blocking(scan_apps_impl)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn app_sizes(paths: Vec<String>) -> Vec<(String, u64)> {
    tauri::async_runtime::spawn_blocking(move || app_sizes_impl(paths))
        .await
        .unwrap_or_default()
}

fn scan_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let user_apps = PathBuf::from(home).join("Applications");
        if user_apps.is_dir() {
            roots.push(user_apps);
        }
    }
    roots
}

fn scan_apps_impl() -> Vec<AppInfo> {
    let mut bundles: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for root in scan_roots() {
        if !root.is_dir() {
            continue;
        }
        let mut it = WalkDir::new(&root)
            .max_depth(4)
            .follow_links(false)
            .into_iter();
        while let Some(entry) = it.next() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if entry.depth() == 0 {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !entry.file_type().is_dir() {
                continue;
            }
            if name.starts_with('.') {
                it.skip_current_dir();
                continue;
            }
            if name.ends_with(".app") {
                if seen.insert(entry.path().to_path_buf()) {
                    bundles.push(entry.path().to_path_buf());
                }
                it.skip_current_dir();
            }
        }
    }

    let mut apps: Vec<AppInfo> = bundles.par_iter().map(|p| read_app(p)).collect();
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.path.cmp(&b.path))
    });
    apps
}

fn read_app(path: &Path) -> AppInfo {
    let info_path = path.join("Contents").join("Info.plist");
    let dict = plist::Value::from_file(&info_path)
        .ok()
        .and_then(|v| v.into_dictionary());

    let get = |key: &str| {
        dict.as_ref()
            .and_then(|d| d.get(key))
            .and_then(|v| v.as_string())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let bundle_id = get("CFBundleIdentifier").unwrap_or_else(|| file_stem.clone());
    let name = get("CFBundleDisplayName")
        .or_else(|| get("CFBundleName"))
        .unwrap_or_else(|| file_stem.clone());
    let version = get("CFBundleShortVersionString")
        .or_else(|| get("CFBundleVersion"))
        .unwrap_or_default();
    let executable = get("CFBundleExecutable").unwrap_or_default();

    let path_str = path.to_string_lossy().to_string();
    let is_system = path_str.starts_with("/System/")
        || bundle_id.to_lowercase().starts_with("com.apple.");

    AppInfo {
        name,
        bundle_id,
        version,
        path: path_str,
        executable,
        is_system,
    }
}

fn app_sizes_impl(paths: Vec<String>) -> Vec<(String, u64)> {
    paths
        .par_iter()
        .map(|p| {
            let size = dir_size(Path::new(p));
            (p.clone(), size)
        })
        .collect()
}

pub fn dir_size(path: &Path) -> u64 {
    let mut total: u64 = 0;
    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_system_apps() {
        let apps = scan_apps_impl();
        assert!(!apps.is_empty(), "should find at least one app bundle");
        for app in &apps {
            assert!(app.path.ends_with(".app"));
            assert!(!app.name.is_empty());
        }
    }

    #[test]
    fn dir_size_handles_missing_path() {
        assert_eq!(dir_size(Path::new("/nonexistent/path/xyz")), 0);
    }
}

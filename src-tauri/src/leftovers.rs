use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::scanner;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverItem {
    pub path: String,
    pub kind: String,
    pub size: u64,
}

#[tauri::command]
pub async fn find_leftovers(
    app_path: String,
    bundle_id: String,
    names: Vec<String>,
) -> Vec<LeftoverItem> {
    tauri::async_runtime::spawn_blocking(move || {
        find_leftovers_impl(&app_path, &bundle_id, &names)
    })
    .await
    .unwrap_or_default()
}

const KIND_ORDER: &[&str] = &[
    "应用支持",
    "偏好设置",
    "缓存",
    "应用容器",
    "共享容器",
    "网络存储",
    "WebKit 缓存",
    "日志",
    "窗口状态",
    "启动项",
    "应用脚本",
];

fn kind_rank(kind: &str) -> usize {
    KIND_ORDER
        .iter()
        .position(|k| *k == kind)
        .unwrap_or(KIND_ORDER.len())
}

fn find_leftovers_impl(app_path: &str, bundle_id: &str, names: &[String]) -> Vec<LeftoverItem> {
    let Some(home) = crate::util::home_dir() else {
        return Vec::new();
    };
    let library = home.join("Library");

    let file_stem = Path::new(app_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut all_names: Vec<String> = names.to_vec();
    all_names.push(file_stem);

    let matcher = Matcher::new(bundle_id, &all_names, app_path);
    let mut items: Vec<LeftoverItem> = Vec::new();

    scan_dir(
        &library.join("Application Support"),
        "应用支持",
        &matcher,
        true,
        false,
        &mut items,
    );
    scan_deep(
        &library.join("Application Support"),
        "应用支持",
        &matcher,
        &mut items,
    );
    scan_dir(
        &library.join("Caches"),
        "缓存",
        &matcher,
        true,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("Preferences"),
        "偏好设置",
        &matcher,
        true,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("Preferences").join("ByHost"),
        "偏好设置",
        &matcher,
        true,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("Logs"),
        "日志",
        &matcher,
        true,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("Saved Application State"),
        "窗口状态",
        &matcher,
        true,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("Containers"),
        "应用容器",
        &matcher,
        false,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("Group Containers"),
        "共享容器",
        &matcher,
        false,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("HTTPStorages"),
        "网络存储",
        &matcher,
        false,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("WebKit"),
        "WebKit 缓存",
        &matcher,
        false,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("Cookies"),
        "网络存储",
        &matcher,
        false,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("Application Scripts"),
        "应用脚本",
        &matcher,
        false,
        false,
        &mut items,
    );
    scan_dir(
        &library.join("LaunchAgents"),
        "启动项",
        &matcher,
        true,
        true,
        &mut items,
    );

    dedupe(&mut items);
    items.par_iter_mut().for_each(|item| {
        item.size = scanner::dir_size(Path::new(&item.path));
    });
    items.sort_by(|a, b| {
        kind_rank(&a.kind)
            .cmp(&kind_rank(&b.kind))
            .then_with(|| a.path.cmp(&b.path))
    });
    items
}

fn make_item(path: PathBuf, kind: &str) -> LeftoverItem {
    LeftoverItem {
        path: path.to_string_lossy().to_string(),
        kind: kind.to_string(),
        size: 0,
    }
}

fn scan_dir(
    dir: &Path,
    kind: &str,
    matcher: &Matcher,
    allow_name_match: bool,
    content_scan: bool,
    out: &mut Vec<LeftoverItem>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".DS_Store" {
            continue;
        }
        match matcher.matches(&name) {
            Some(true) => out.push(make_item(path, kind)),
            Some(false) if allow_name_match => out.push(make_item(path, kind)),
            _ => {
                if content_scan && matcher.file_content_matches(&path) {
                    out.push(make_item(path, kind));
                }
            }
        }
    }
}

/// Application Support 常见的 `Vendor/AppName` 布局：
/// 若子目录本身没被直接匹配，再尝试用「父目录名 + 子目录名」组合匹配 App 名称。
fn scan_deep(dir: &Path, kind: &str, matcher: &Matcher, out: &mut Vec<LeftoverItem>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let already: HashSet<String> = out.iter().map(|i| i.path.clone()).collect();
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if already.contains(&path.to_string_lossy().to_string()) {
            continue;
        }
        let parent_name = entry.file_name().to_string_lossy().to_string();
        if parent_name.starts_with('.') {
            continue;
        }
        scan_dir(&path, kind, matcher, true, false, out);
        let Ok(children) = std::fs::read_dir(&path) else {
            continue;
        };
        for child in children.filter_map(|c| c.ok()) {
            let child_path = child.path();
            let child_name = child.file_name().to_string_lossy().to_string();
            if matcher.matches(&child_name).is_some() {
                continue;
            }
            if matcher.matches_name_parts(&[&parent_name, &child_name]) {
                out.push(make_item(child_path, kind));
            }
        }
    }
}

fn dedupe(items: &mut Vec<LeftoverItem>) {
    items.sort_by(|a, b| a.path.cmp(&b.path));
    items.dedup_by(|a, b| a.path == b.path);
    items.sort_by_key(|i| i.path.len());

    let mut kept: Vec<LeftoverItem> = Vec::with_capacity(items.len());
    'outer: for item in items.drain(..) {
        for existing in &kept {
            if item.path.len() > existing.path.len()
                && item.path.starts_with(&existing.path)
                && item.path.as_bytes()[existing.path.len()] == b'/'
            {
                continue 'outer;
            }
        }
        kept.push(item);
    }
    *items = kept;
}

const KNOWN_SUFFIXES: &[&str] = &[
    ".plist.lockfile",
    ".binarycookies",
    ".savedstate",
    ".lockfile",
    ".plist",
    ".log",
    ".crash",
];

fn strip_known_suffixes(s: &str) -> String {
    let mut out = s.to_string();
    loop {
        let mut changed = false;
        for suffix in KNOWN_SUFFIXES {
            if out.len() > suffix.len() && out.ends_with(suffix) {
                out.truncate(out.len() - suffix.len());
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }
    out
}

fn normalize_name(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_alphanumeric())
        .collect()
}

struct Matcher {
    bundle_id: String,
    names: Vec<String>,
    app_path_lower: String,
}

impl Matcher {
    fn new(bundle_id: &str, names: &[String], app_path: &str) -> Self {
        let mut names: Vec<String> = names
            .iter()
            .map(|n| normalize_name(n))
            .filter(|n| !n.is_empty())
            .collect();
        names.sort();
        names.dedup();
        Matcher {
            bundle_id: bundle_id.trim().to_lowercase(),
            names,
            app_path_lower: app_path.to_lowercase(),
        }
    }

    /// Some(true) = 按 Bundle ID 匹配，Some(false) = 按 App 名称匹配。
    fn matches(&self, entry_name: &str) -> Option<bool> {
        let raw = entry_name.to_lowercase();
        let base = strip_known_suffixes(&raw);

        if !self.bundle_id.is_empty() {
            let bid = &self.bundle_id;
            if base == *bid
                || base.starts_with(&format!("{bid}."))
                || base.ends_with(&format!(".{bid}"))
                || base.contains(&format!(".{bid}."))
            {
                return Some(true);
            }
        }

        let norm = normalize_name(&base);
        if !norm.is_empty() && self.names.iter().any(|n| *n == norm) {
            return Some(false);
        }
        None
    }

    fn matches_name_parts(&self, parts: &[&str]) -> bool {
        let combined: String = parts.iter().map(|p| normalize_name(p)).collect();
        !combined.is_empty() && self.names.iter().any(|n| *n == combined)
    }

    fn file_content_matches(&self, path: &Path) -> bool {
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        if !meta.is_file() || meta.len() > 512 * 1024 {
            return false;
        }
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };
        let text = String::from_utf8_lossy(&bytes).to_lowercase();
        (!self.bundle_id.is_empty() && text.contains(&self.bundle_id))
            || text.contains(&self.app_path_lower)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher() -> Matcher {
        Matcher::new(
            "com.example.Demo",
            &["Demo App".to_string(), "Demo".to_string()],
            "/Applications/Demo.app",
        )
    }

    #[test]
    fn matches_bundle_id_variants() {
        let m = matcher();
        assert_eq!(m.matches("com.example.Demo"), Some(true));
        assert_eq!(m.matches("com.example.Demo.plist"), Some(true));
        assert_eq!(m.matches("com.example.Demo.savedState"), Some(true));
        assert_eq!(m.matches("com.example.Demo.binarycookies"), Some(true));
        assert_eq!(m.matches("TEAMID1234.com.example.Demo"), Some(true));
        assert_eq!(m.matches("com.example.Demo.helper"), Some(true));
    }

    #[test]
    fn matches_app_name() {
        let m = matcher();
        assert_eq!(m.matches("Demo App"), Some(false));
        assert_eq!(m.matches("demo-app"), Some(false));
        assert_eq!(m.matches("Demo"), Some(false));
        assert_eq!(m.matches("Google"), None);
        assert_eq!(m.matches("Unrelated Thing"), None);
    }

    #[test]
    fn vendor_app_name_parts() {
        let m = matcher();
        assert!(m.matches_name_parts(&["Demo", "App"]));
        assert!(!m.matches_name_parts(&["Other", "Thing"]));
    }

    #[test]
    fn strips_known_suffixes() {
        assert_eq!(strip_known_suffixes("com.foo.bar.plist"), "com.foo.bar");
        assert_eq!(
            strip_known_suffixes("com.foo.bar.plist.lockfile"),
            "com.foo.bar"
        );
        assert_eq!(
            strip_known_suffixes("com.foo.bar.savedState".to_lowercase().as_str()),
            "com.foo.bar"
        );
        assert_eq!(strip_known_suffixes("com.foo.bar"), "com.foo.bar");
    }

    #[test]
    fn dedupes_nested_paths() {
        let mut items = vec![
            make_item(PathBuf::from("/tmp/a/b"), "缓存"),
            make_item(PathBuf::from("/tmp/a"), "缓存"),
            make_item(PathBuf::from("/tmp/a/b"), "缓存"),
            make_item(PathBuf::from("/tmp/ab"), "缓存"),
        ];
        dedupe(&mut items);
        let paths: Vec<&str> = items.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, vec!["/tmp/a", "/tmp/ab"]);
    }
}

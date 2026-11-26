use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

const MAX_ENTRIES: usize = 200;
static FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub name: String,
    pub path: String,
    pub ok: bool,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp: i64,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub freed_bytes: u64,
    #[serde(default)]
    pub removed: usize,
    #[serde(default)]
    pub failed: usize,
    #[serde(default)]
    pub items: Vec<HistoryItem>,
}

impl HistoryEntry {
    pub fn new(kind: &str, title: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            id: now.as_nanos().to_string(),
            timestamp: now.as_millis() as i64,
            kind: kind.to_string(),
            title: title.into(),
            mode: None,
            freed_bytes: 0,
            removed: 0,
            failed: 0,
            items: Vec::new(),
        }
    }

    pub fn item(name: &str, path: &str, ok: bool, message: Option<String>) -> HistoryItem {
        HistoryItem {
            name: name.to_string(),
            path: path.to_string(),
            ok,
            message,
        }
    }
}

fn history_file(app: &tauri::AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("history.json"))
}

fn read_entries(path: &Path) -> Vec<HistoryEntry> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn record(app: &tauri::AppHandle, entry: HistoryEntry) {
    let Some(path) = history_file(app) else {
        return;
    };
    let Ok(_guard) = FILE_LOCK.lock() else {
        return;
    };
    let mut entries = read_entries(&path);
    entries.insert(0, entry);
    entries.truncate(MAX_ENTRIES);
    if let Ok(json) = serde_json::to_vec_pretty(&entries) {
        let _ = std::fs::write(&path, json);
    }
}

#[tauri::command]
pub fn list_history(app: tauri::AppHandle) -> Vec<HistoryEntry> {
    let Some(path) = history_file(&app) else {
        return Vec::new();
    };
    let _guard = FILE_LOCK.lock();
    read_entries(&path)
}

#[tauri::command]
pub fn clear_history(app: tauri::AppHandle) -> Result<(), String> {
    let Some(path) = history_file(&app) else {
        return Err("无法定位应用数据目录".to_string());
    };
    let _guard = FILE_LOCK.lock().map_err(|e| e.to_string())?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("清空历史失败：{e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_camel_case_and_roundtrips() {
        let mut entry = HistoryEntry::new("uninstall", "Demo");
        entry.mode = Some("trash".to_string());
        entry.freed_bytes = 2048;
        entry.removed = 2;
        entry.items.push(HistoryEntry::item("Demo", "/tmp/Demo.app", true, None));

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"freedBytes\":2048"));
        assert!(json.contains("\"items\":["));

        let back: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.title, "Demo");
        assert_eq!(back.freed_bytes, 2048);
        assert_eq!(back.items.len(), 1);
    }

    #[test]
    fn tolerates_legacy_entries_without_optional_fields() {
        let json = r#"[{"id":"1","timestamp":1,"kind":"uninstall","title":"Old"}]"#;
        let entries: Vec<HistoryEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].freed_bytes, 0);
        assert!(entries[0].items.is_empty());
    }
}

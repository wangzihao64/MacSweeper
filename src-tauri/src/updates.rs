use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

use rayon::prelude::*;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCandidate {
    pub path: String,
    pub bundle_id: String,
    pub version: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub path: String,
    /// appstore | homebrew | sparkle | self
    pub channel: String,
    pub latest_version: Option<String>,
    pub installed_version: Option<String>,
    pub update_available: bool,
    /// trackId / cask token / app path, depending on channel
    pub target: String,
    pub note: Option<String>,
}

#[tauri::command]
pub async fn check_updates(apps: Vec<UpdateCandidate>) -> Vec<UpdateInfo> {
    tauri::async_runtime::spawn_blocking(move || check_impl(apps))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn perform_update(
    app_path: String,
    channel: String,
    target: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || perform_impl(&app_path, &channel, &target))
        .await
        .map_err(|e| format!("更新任务异常：{e}"))?
}

#[derive(Debug, Clone)]
struct BrewCask {
    token: String,
    installed: String,
    latest: String,
    auto_updates: bool,
}

fn check_impl(apps: Vec<UpdateCandidate>) -> Vec<UpdateInfo> {
    let casks = brew_casks();
    apps.into_par_iter()
        .filter_map(|app| check_one(app, &casks))
        .collect()
}

fn check_one(app: UpdateCandidate, casks: &HashMap<String, BrewCask>) -> Option<UpdateInfo> {
    let dir = Path::new(&app.path);
    if !dir.is_dir() {
        return None;
    }

    if let Some(cask) = casks.get(&normalize(&app.path)) {
        let available = is_newer(&cask.latest, &cask.installed);
        return Some(UpdateInfo {
            path: app.path.clone(),
            channel: if cask.auto_updates { "self" } else { "homebrew" }.to_string(),
            latest_version: Some(cask.latest.clone()),
            installed_version: Some(cask.installed.clone()),
            update_available: available,
            target: if cask.auto_updates {
                app.path.clone()
            } else {
                cask.token.clone()
            },
            note: if cask.auto_updates {
                Some(format!(
                    "Homebrew 托管且启用自动更新，请在应用内升级，或执行 brew upgrade --cask {}",
                    cask.token
                ))
            } else {
                None
            },
        });
    }

    if dir.join("Contents/_MASReceipt/receipt").exists() {
        let (latest, track_id) = appstore_latest(&app.bundle_id)?;
        return Some(UpdateInfo {
            path: app.path.clone(),
            channel: "appstore".to_string(),
            update_available: is_newer(&latest, &app.version),
            latest_version: Some(latest),
            installed_version: Some(app.version.clone()),
            target: track_id,
            note: None,
        });
    }

    sparkle_check(&app)
}

fn sparkle_check(app: &UpdateCandidate) -> Option<UpdateInfo> {
    let dir = Path::new(&app.path);
    let dict = plist::Value::from_file(dir.join("Contents/Info.plist"))
        .ok()?
        .into_dictionary()?;
    let feed = dict.get("SUFeedURL")?.as_string()?.trim().to_string();
    if !(feed.starts_with("https://") || feed.starts_with("http://")) {
        return None;
    }

    let body = curl_text(&feed, 8)?;
    let (short, build) = appcast_latest(&body)?;

    let (latest, installed, available) = match (short, build) {
        (Some(short), _) => {
            let available = is_newer(&short, &app.version);
            (short, app.version.clone(), available)
        }
        (None, Some(build)) => {
            let installed_build = dict
                .get("CFBundleVersion")
                .and_then(|v| v.as_string())
                .unwrap_or_default()
                .to_string();
            let available = is_newer(&build, &installed_build);
            (build, installed_build, available)
        }
        (None, None) => return None,
    };

    Some(UpdateInfo {
        path: app.path.clone(),
        channel: "sparkle".to_string(),
        latest_version: Some(latest),
        installed_version: Some(installed),
        update_available: available,
        target: app.path.clone(),
        note: None,
    })
}

fn brew_path() -> Option<&'static str> {
    ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
        .into_iter()
        .find(|p| Path::new(p).is_file())
}

fn brew_casks() -> HashMap<String, BrewCask> {
    let mut map = HashMap::new();
    let Some(brew) = brew_path() else {
        return map;
    };
    // brew list --cask --json was removed in Homebrew 6, info --installed
    // carries install/latest versions and app artifact targets in one call.
    let Ok(output) = Command::new(brew)
        .args(["info", "--cask", "--json=v2", "--installed"])
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .stdin(Stdio::null())
        .output()
    else {
        return map;
    };
    if !output.status.success() {
        return map;
    }
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return map;
    };

    for cask in json["casks"].as_array().into_iter().flatten() {
        let token = cask["token"].as_str().unwrap_or_default().to_string();
        if token.is_empty() {
            continue;
        }
        let entry = BrewCask {
            token,
            installed: json_version(&cask["installed"]),
            latest: json_version(&cask["version"]),
            auto_updates: cask["auto_updates"].as_bool().unwrap_or(false),
        };
        for artifact in cask["artifacts"].as_array().into_iter().flatten() {
            if let Some(target) = artifact["target"].as_str() {
                if target.ends_with(".app") {
                    map.entry(normalize(target)).or_insert_with(|| entry.clone());
                }
            }
        }
    }
    map
}

fn json_version(value: &serde_json::Value) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else if let Some(list) = value.as_array() {
        list.iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(",")
    } else {
        String::new()
    }
}

fn appstore_latest(bundle_id: &str) -> Option<(String, String)> {
    if bundle_id.is_empty()
        || !bundle_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return None;
    }
    for country in ["cn", "us"] {
        let url = format!(
            "https://itunes.apple.com/lookup?bundleId={bundle_id}&country={country}&entity=macSoftware&limit=1"
        );
        let Some(body) = curl_text(&url, 8) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) else {
            continue;
        };
        let Some(result) = json.get("results").and_then(|r| r.get(0)) else {
            continue;
        };
        let (Some(version), Some(track_id)) = (
            result.get("version").and_then(|v| v.as_str()),
            result.get("trackId").and_then(|v| v.as_i64()),
        ) else {
            continue;
        };
        return Some((version.to_string(), track_id.to_string()));
    }
    None
}

fn curl_text(url: &str, timeout_secs: u64) -> Option<String> {
    let output = Command::new("/usr/bin/curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "6",
            "--max-time",
            &timeout_secs.to_string(),
            "-A",
            "MacSweeper/1.0",
        ])
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Returns (short version, build version) of the newest appcast item.
fn appcast_latest(xml: &str) -> Option<(Option<String>, Option<String>)> {
    let mut best: Option<(String, Option<String>, Option<String>)> = None;
    for chunk in xml.split("<item").skip(1) {
        let block = chunk.split("</item>").next().unwrap_or(chunk);
        let short = attr(block, "shortVersionString");
        let build = attr(block, "version");
        let Some(key) = short.clone().or_else(|| build.clone()) else {
            continue;
        };
        let better = match &best {
            None => true,
            Some((current, _, _)) => is_newer(&key, current),
        };
        if better {
            best = Some((key, short, build));
        }
    }
    let (_, short, build) = best?;
    if short.is_none() && build.is_none() {
        return None;
    }
    Some((short, build))
}

fn attr(block: &str, key: &str) -> Option<String> {
    let pattern = format!("{key}=\"");
    let start = block.find(&pattern)? + pattern.len();
    let rest = &block[start..];
    let end = rest.find('"')?;
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Numeric-aware dotted comparison: 1.10 > 1.9, 4.4.0 > 4.3.3.
fn is_newer(latest: &str, current: &str) -> bool {
    let a = version_parts(latest);
    let b = version_parts(current);
    if a.is_empty() || b.is_empty() {
        return false;
    }
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

fn version_parts(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u64>().unwrap_or(0))
        .collect()
}

fn normalize(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}

fn perform_impl(app_path: &str, channel: &str, target: &str) -> Result<String, String> {
    match channel {
        "homebrew" => run_brew_upgrade(target),
        "appstore" => {
            if target.is_empty() || !target.chars().all(|c| c.is_ascii_digit()) {
                return Err("无效的 App Store 应用编号".to_string());
            }
            let url = format!("macappstore://apps.apple.com/app/id{target}");
            let status = Command::new("/usr/bin/open")
                .arg(&url)
                .status()
                .map_err(|e| format!("无法打开 App Store：{e}"))?;
            if !status.success() {
                return Err("无法打开 App Store".to_string());
            }
            Ok("已打开 App Store，点击「更新」即可完成升级。".to_string())
        }
        "sparkle" | "self" => {
            let path = Path::new(app_path);
            if !path.is_absolute() || !path.is_dir() || path.extension().and_then(|e| e.to_str()) != Some("app") {
                return Err("无效的应用路径".to_string());
            }
            let status = Command::new("/usr/bin/open")
                .arg("-a")
                .arg(path)
                .status()
                .map_err(|e| format!("无法启动应用：{e}"))?;
            if !status.success() {
                return Err("无法启动应用".to_string());
            }
            Ok("已启动应用，请在应用内使用「检查更新」完成升级。".to_string())
        }
        _ => Err("未知的更新渠道".to_string()),
    }
}

fn run_brew_upgrade(token: &str) -> Result<String, String> {
    if token.is_empty()
        || !token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+' | '@'))
    {
        return Err("无效的 Cask 名称".to_string());
    }
    let brew = brew_path().ok_or("未找到 Homebrew")?;
    let output = Command::new(brew)
        .args(["upgrade", "--cask", token])
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("无法启动 Homebrew：{e}"))?;

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let summary = summarize(&text);
    if output.status.success() {
        Ok(summary)
    } else if summary.is_empty() {
        Err("Homebrew 升级失败".to_string())
    } else {
        Err(summary)
    }
}

fn summarize(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    let start = lines.len().saturating_sub(8);
    let mut out = lines[start..].join("\n");
    if out.chars().count() > 800 {
        out = out.chars().skip(out.chars().count() - 800).collect();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numeric_versions() {
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("4.4.0", "4.3.3"));
        assert!(is_newer("5.1.0,609", "4.2.1,487"));
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(!is_newer("1.2.3", "1.2.4"));
        assert!(!is_newer("1.0", ""));
    }

    #[test]
    fn parses_appcast_newest_item() {
        let xml = r#"<?xml version="1.0"?><rss><channel>
<item><title>1.2.0</title><enclosure url="https://example.com/a.zip" sparkle:version="120" sparkle:shortVersionString="1.2.0"/></item>
<item><title>1.10.0</title><enclosure url="https://example.com/b.zip" sparkle:version="1100" sparkle:shortVersionString="1.10.0"/></item>
</channel></rss>"#;
        let (short, build) = appcast_latest(xml).unwrap();
        assert_eq!(short.as_deref(), Some("1.10.0"));
        assert_eq!(build.as_deref(), Some("1100"));
    }

    #[test]
    fn rejects_bad_brew_token() {
        assert!(run_brew_upgrade("a;rm -rf /").is_err());
        assert!(run_brew_upgrade("").is_err());
    }
}

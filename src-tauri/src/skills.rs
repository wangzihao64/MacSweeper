use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::util;

const ROOTS: &[(&str, &str)] = &[
    ("agents", ".agents/skills"),
    ("claude", ".claude/skills"),
    ("codex", ".codex/skills"),
    ("codex-system", ".codex/skills/.system"),
    ("qoder", ".qoder/skills"),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLocation {
    pub root: String,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub path: String,
    pub real_path: String,
    pub locations: Vec<SkillLocation>,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub installed_at: Option<String>,
    pub updated_at: Option<String>,
    pub managed: bool,
}

#[derive(Default)]
struct FrontMatter {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
}

struct Found {
    root: &'static str,
    path: PathBuf,
    name: String,
    description: String,
    version: Option<String>,
}

#[derive(Deserialize)]
struct LockFile {
    #[serde(default)]
    skills: BTreeMap<String, LockSkill>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LockSkill {
    source: Option<String>,
    source_url: Option<String>,
    installed_at: Option<String>,
    updated_at: Option<String>,
}

#[tauri::command]
pub async fn scan_skills() -> Result<Vec<SkillInfo>, String> {
    tauri::async_runtime::spawn_blocking(scan_impl)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_skill(name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || update_impl(&name))
        .await
        .map_err(|e| e.to_string())?
}

fn scan_impl() -> Result<Vec<SkillInfo>, String> {
    let home = util::home_dir().ok_or("无法定位用户目录")?;
    let lock = read_lockfile(&home);

    let mut found: Vec<Found> = Vec::new();
    for (root_id, rel) in ROOTS {
        let dir = home.join(rel);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let link_name = entry.file_name().to_string_lossy().to_string();
            if link_name.starts_with('.') {
                continue;
            }
            if !is_dir(&path) {
                continue;
            }
            let skill_md = path.join("SKILL.md");
            if !skill_md.is_file() {
                continue;
            }
            let content = fs::read_to_string(&skill_md).unwrap_or_default();
            let fm = parse_frontmatter(&content);
            let name = fm
                .name
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| link_name.clone());
            let version = fm
                .version
                .filter(|s| !s.is_empty())
                .or_else(|| metadata_json_version(&path));
            found.push(Found {
                root: root_id,
                path,
                name,
                description: fm.description.unwrap_or_default(),
                version,
            });
        }
    }

    // 同一个 skill 常通过软链接出现在多个工具目录里，按名称合并
    let mut merged: BTreeMap<String, SkillInfo> = BTreeMap::new();
    for item in found {
        let real_path = item
            .path
            .canonicalize()
            .unwrap_or_else(|_| item.path.clone());
        let entry = merged.entry(item.name.clone()).or_insert_with(|| SkillInfo {
            name: item.name.clone(),
            description: item.description.clone(),
            version: item.version.clone(),
            path: item.path.to_string_lossy().to_string(),
            real_path: real_path.to_string_lossy().to_string(),
            locations: Vec::new(),
            source: None,
            source_url: None,
            installed_at: None,
            updated_at: None,
            managed: false,
        });
        if entry.description.is_empty() && !item.description.is_empty() {
            entry.description = item.description.clone();
        }
        if entry.version.is_none() {
            entry.version = item.version.clone();
        }
        if item.root == "agents" {
            entry.path = item.path.to_string_lossy().to_string();
            entry.real_path = real_path.to_string_lossy().to_string();
        }
        entry.locations.push(SkillLocation {
            root: item.root.to_string(),
            path: item.path.to_string_lossy().to_string(),
        });
    }

    let mut result: Vec<SkillInfo> = merged.into_values().collect();
    for skill in &mut result {
        let has_agents = skill.locations.iter().any(|l| l.root == "agents");
        // lockfile 以安装目录名登记，frontmatter 的 name 可能不同
        let lock_key = if has_agents {
            Path::new(&skill.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&skill.name)
                .to_string()
        } else {
            skill.name.clone()
        };
        if let Some(lock_skill) = lock.get(&lock_key) {
            skill.managed = has_agents;
            if has_agents {
                skill.source = lock_skill.source.clone();
                skill.source_url = lock_skill.source_url.clone();
                skill.installed_at = lock_skill.installed_at.clone();
                skill.updated_at = lock_skill.updated_at.clone();
            }
        }
        skill.locations.sort_by_key(|l| root_order(&l.root));
    }
    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(result)
}

fn root_order(root: &str) -> usize {
    ROOTS.iter().position(|(id, _)| *id == root).unwrap_or(usize::MAX)
}

fn is_dir(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

fn read_lockfile(home: &Path) -> BTreeMap<String, LockSkill> {
    let path = home.join(".agents/.skill-lock.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<LockFile>(&text).ok())
        .map(|f| f.skills)
        .unwrap_or_default()
}

fn metadata_json_version(dir: &Path) -> Option<String> {
    let text = fs::read_to_string(dir.join("metadata.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get("version")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let (first, last) = (bytes[0], bytes[bytes.len() - 1]);
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn parse_frontmatter(content: &str) -> FrontMatter {
    let mut fm = FrontMatter::default();
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return fm;
    }
    let mut block: Vec<&str> = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        block.push(line);
    }

    let mut in_metadata = false;
    let mut i = 0;
    while i < block.len() {
        let line = block[i];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let Some((key, rest)) = trimmed.split_once(':') else {
            i += 1;
            continue;
        };
        let key = key.trim();
        let value = rest.trim();

        if indent > 0 {
            if in_metadata && key == "version" && fm.version.is_none() {
                fm.version = Some(unquote(value));
            }
            i += 1;
            continue;
        }

        if key == "metadata" {
            in_metadata = true;
            i += 1;
            continue;
        }
        in_metadata = false;

        let mut collected = value.to_string();
        let mut next = i + 1;
        if value.is_empty() || value == ">" || value == "|" || value == ">-" || value == "|-" {
            collected.clear();
            while next < block.len() {
                let cont = block[next];
                let cont_trimmed = cont.trim();
                let cont_indent = cont.len() - cont.trim_start().len();
                if cont_trimmed.is_empty() || cont_indent == 0 {
                    break;
                }
                if !collected.is_empty() {
                    collected.push(' ');
                }
                collected.push_str(cont_trimmed);
                next += 1;
            }
        }
        let cleaned = unquote(&collected);
        match key {
            "name" if fm.name.is_none() => fm.name = Some(cleaned),
            "description" if fm.description.is_none() => fm.description = Some(cleaned),
            "version" if fm.version.is_none() => fm.version = Some(cleaned),
            _ => {}
        }
        i = next;
    }
    fm
}

fn find_npx(home: &Path) -> Option<PathBuf> {
    let mut candidates = vec![
        home.join(".local/bin/npx"),
        PathBuf::from("/opt/homebrew/bin/npx"),
        PathBuf::from("/usr/local/bin/npx"),
        PathBuf::from("/usr/bin/npx"),
    ];
    let nvm = home.join(".nvm/versions/node");
    if let Ok(entries) = fs::read_dir(nvm) {
        let mut versions: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        versions.sort();
        for v in versions.into_iter().rev() {
            candidates.push(v.join("bin/npx"));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

fn update_impl(name: &str) -> Result<String, String> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !valid {
        return Err("Skill 名称不合法。".into());
    }

    let home = util::home_dir().ok_or("无法定位用户目录")?;
    let lock = read_lockfile(&home);
    if !lock.contains_key(name) {
        return Err("该 Skill 不在 Skills CLI 的安装记录中，请用安装它的工具更新。".into());
    }

    let npx = find_npx(&home)
        .ok_or("未找到 npx，请先安装 Node.js（https://nodejs.org）。")?;

    let mut cmd = Command::new(npx);
    cmd.args(["-y", "skills", "update", name, "-g", "-y"])
        .current_dir(&home)
        .env(
            "PATH",
            format!(
                "{}:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
                home.join(".local/bin").display()
            ),
        )
        .env("CI", "1")
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let (ok, stdout, stderr) = run_with_timeout(cmd, Duration::from_secs(240))?;
    let raw = if stdout.trim().is_empty() { stderr } else { stdout };
    let text = clean_output(&raw);
    let summary = summarize_output(&text);

    if !ok {
        return Err(if summary.is_empty() {
            format!("更新「{name}」失败，请检查网络后重试。")
        } else {
            format!("更新「{name}」失败：\n{summary}")
        });
    }
    if text.to_lowercase().contains("up to date") {
        Ok(format!("「{name}」已是最新版本。"))
    } else {
        Ok(format!("「{name}」已更新到最新版本。"))
    }
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('[') => {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next == '\x07' {
                        break;
                    }
                    if next == '\x1b' {
                        chars.next();
                        break;
                    }
                }
            }
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

fn clean_output(text: &str) -> String {
    let stripped = strip_ansi(text);
    stripped
        .lines()
        .map(|line| line.rsplit('\r').next().unwrap_or(line).trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_with_timeout(
    mut cmd: Command,
    timeout: Duration,
) -> Result<(bool, String, String), String> {
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法启动 npx：{e}"))?;
    let out = child.stdout.take();
    let err = child.stderr.take();
    let (tx, rx) = mpsc::channel();

    if let Some(out) = out {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(("out".to_string(), read_all(out)));
        });
    }
    if let Some(err) = err {
        let tx2 = tx.clone();
        std::thread::spawn(move || {
            let _ = tx2.send(("err".to_string(), read_all(err)));
        });
    }
    drop(tx);

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(150))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    for (kind, text) in rx.iter() {
        if kind == "out" {
            stdout = text;
        } else {
            stderr = text;
        }
    }

    let Some(status) = status else {
        return Err("更新超时，请检查网络后重试。".into());
    };
    Ok((status.success(), stdout, stderr))
}

fn read_all(mut reader: impl Read) -> String {
    let mut buf = String::new();
    let _ = reader.read_to_string(&mut buf);
    buf
}

fn summarize_output(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with("(node:"))
        .filter(|l| !l.starts_with("(Use `node"))
        .filter(|l| !l.starts_with("Warning:"))
        .collect();
    lines
        .iter()
        .skip(lines.len().saturating_sub(8))
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_fields() {
        let text = "---\nname: deploy-to-vercel\ndescription: Deploy apps.\nmetadata:\n  author: vercel\n  version: \"3.0.0\"\n---\nbody";
        let fm = parse_frontmatter(text);
        assert_eq!(fm.name.as_deref(), Some("deploy-to-vercel"));
        assert_eq!(fm.description.as_deref(), Some("Deploy apps."));
        assert_eq!(fm.version.as_deref(), Some("3.0.0"));
    }

    #[test]
    fn parses_quoted_and_folded_description() {
        let text = "---\nname: \"imagegen\"\ndescription: >\n  First line\n  second line.\nlicense: MIT\n---\n";
        let fm = parse_frontmatter(text);
        assert_eq!(fm.name.as_deref(), Some("imagegen"));
        assert_eq!(fm.description.as_deref(), Some("First line second line."));
        assert_eq!(fm.version, None);
    }

    #[test]
    fn rejects_missing_frontmatter() {
        let fm = parse_frontmatter("# Title\nname: nope\n");
        assert!(fm.name.is_none());
    }

    #[test]
    fn summarizes_cli_output() {
        let text = "(node:1) Warning: x\nUpdating find-skills…\nUpdated find-skills\n";
        assert_eq!(
            summarize_output(text),
            "Updating find-skills…\nUpdated find-skills"
        );
    }

    #[test]
    fn strips_ansi_and_progress_frames() {
        let text = "\x1b[?25l│\r◒  Downloading…\r◇  Downloaded\x1b[?25h\n\x1b[1G✓ Updated\n";
        assert_eq!(clean_output(text), "◇  Downloaded\n✓ Updated");
    }
}

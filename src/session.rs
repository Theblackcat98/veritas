//! Session persistence — JSON transcripts under the XDG data dir (D012).
//!
//! One file per conversation:
//! `<data>/veritas/sessions/sess-<unixsecs>.json`. Time is std-only unix
//! seconds (no chrono); the UI renders relative age (D012).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::app::{ChatMessage, Role};

const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct SessionFile {
    version: u32,
    title: Option<String>,
    created_at: u64,
    updated_at: u64,
    messages: Vec<StoredMessage>,
}

#[derive(Serialize, Deserialize)]
struct StoredMessage {
    role: String,
    content: String,
}

/// XDG data dir + `veritas/sessions` (`$XDG_DATA_HOME`, else `~/.local/share`).
pub fn sessions_dir() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|h| PathBuf::from(h).join(".local/share"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("veritas").join("sessions")
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn new_id() -> String {
    format!("sess-{}", now_unix())
}

fn to_stored(messages: &[ChatMessage]) -> Vec<StoredMessage> {
    messages
        .iter()
        .map(|m| StoredMessage {
            role: match m.role {
                Role::User => "user".to_string(),
                Role::Agent => "agent".to_string(),
            },
            content: m.content.clone(),
        })
        .collect()
}

fn to_chat(stored: &[StoredMessage]) -> Vec<ChatMessage> {
    stored
        .iter()
        .map(|m| ChatMessage {
            role: if m.role == "user" {
                Role::User
            } else {
                Role::Agent
            },
            content: m.content.clone(),
        })
        .collect()
}

/// Atomic write (tmp + rename). `updated_at` is stamped as "now".
pub fn save(
    dir: &Path,
    id: &str,
    title: Option<&str>,
    created_at: u64,
    messages: &[ChatMessage],
) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let file = SessionFile {
        version: FORMAT_VERSION,
        title: title.map(|s| s.to_string()),
        created_at,
        updated_at: now_unix(),
        messages: to_stored(messages),
    };
    let tmp = dir.join(format!("{id}.tmp"));
    fs::write(&tmp, serde_json::to_vec(&file)?)?;
    fs::rename(&tmp, dir.join(format!("{id}.json")))?;
    Ok(())
}

pub struct SessionEntry {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    pub msg_count: usize,
}

/// Scan `dir` for session files, newest first. Corrupt files are skipped,
/// never repaired (D012).
pub fn list(dir: &Path) -> Vec<SessionEntry> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(file) = serde_json::from_slice::<SessionFile>(&bytes) else {
            continue;
        };
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        out.push(SessionEntry {
            id: id.to_string(),
            title: file.title.unwrap_or_else(|| "Untitled".to_string()),
            updated_at: file.updated_at,
            msg_count: file.messages.len(),
        });
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(b.id.cmp(&a.id)));
    out
}

/// Load `(title, created_at, messages)` for `id`.
pub fn load(dir: &Path, id: &str) -> io::Result<(Option<String>, u64, Vec<ChatMessage>)> {
    let bytes = fs::read(dir.join(format!("{id}.json")))?;
    let file: SessionFile = serde_json::from_slice(&bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok((file.title, file.created_at, to_chat(&file.messages)))
}

/// First user message → whitespace-collapsed, ≤40 chars total (ellipsis
/// included when truncated), char-boundary safe.
pub fn auto_title(s: &str) -> String {
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= 40 {
        return collapsed;
    }
    let mut out: String = collapsed.chars().take(39).collect();
    out.push('…');
    out
}

/// Coarse relative age for the picker; no calendar math (D012).
pub fn rel_time(ts: u64, now: u64) -> String {
    let d = now.saturating_sub(ts);
    match d {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{}m ago", d / 60),
        3600..=86_399 => format!("{}h ago", d / 3600),
        _ => format!("{}d ago", d / 86_400),
    }
}

/// Transcript as markdown: H1 title, then `## You` / `## Agent` sections
/// with verbatim content (D014).
pub fn export_markdown(title: Option<&str>, messages: &[ChatMessage]) -> String {
    let mut out = format!("# {}\n\n", title.unwrap_or("Veritas session"));
    for m in messages {
        let role = match m.role {
            Role::User => "You",
            Role::Agent => "Agent",
        };
        out.push_str(&format!("## {role}\n\n{}\n\n", m.content));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("veritas-test-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn msg(role: Role, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: content.to_string(),
        }
    }

    #[test]
    fn roundtrip_save_load_preserves_messages() {
        let dir = tmp_dir("roundtrip");
        let messages = vec![msg(Role::User, "hello there"), msg(Role::Agent, "greetings")];
        save(&dir, "sess-1", Some("hello there"), 42, &messages).expect("save");
        let (title, created, loaded) = load(&dir, "sess-1").expect("load");
        assert_eq!(title.as_deref(), Some("hello there"));
        assert_eq!(created, 42);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].role, Role::User);
        assert_eq!(loaded[1].content, "greetings");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_orders_newest_first_and_skips_corrupt() {
        let dir = tmp_dir("list");
        save(&dir, "sess-1", Some("old"), 1, &[msg(Role::User, "a")]).expect("save 1");
        save(&dir, "sess-2", Some("new"), 2, &[msg(Role::User, "b")]).expect("save 2");
        // Force newer updated_at on sess-2 by rewriting with a later stamp.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        save(&dir, "sess-2", Some("new"), 2, &[msg(Role::User, "b")]).expect("resave 2");
        fs::write(dir.join("junk.json"), b"{not json").expect("junk");
        fs::write(dir.join("sess-1.tmp"), b"partial").expect("tmp");

        let entries = list(&dir);
        assert_eq!(entries.len(), 2, "corrupt + non-json files must be skipped");
        assert_eq!(entries[0].id, "sess-2", "newest first");
        assert_eq!(entries[0].msg_count, 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn auto_title_collapses_and_truncates_on_char_boundary() {
        assert_eq!(auto_title("  a   b\tc  "), "a b c");
        let long = "é".repeat(50);
        let t = auto_title(&long);
        assert_eq!(t.chars().count(), 40, "39 chars + ellipsis");
        assert!(t.ends_with('…'));
    }

    #[test]
    fn rel_time_buckets() {
        assert_eq!(rel_time(100, 130), "just now");
        assert_eq!(rel_time(100, 300), "3m ago");
        assert_eq!(rel_time(100, 7300), "2h ago");
        assert_eq!(rel_time(100, 200_000), "2d ago");
        assert_eq!(rel_time(200_000, 100), "just now", "clock skew is clamped");
    }

    #[test]
    fn export_markdown_sections() {
        let messages = vec![msg(Role::User, "hi"), msg(Role::Agent, "hello")];
        let md = export_markdown(Some("chat"), &messages);
        assert!(md.starts_with("# chat\n\n"));
        assert!(md.contains("## You\n\nhi\n\n"));
        assert!(md.contains("## Agent\n\nhello\n\n"));
        assert!(export_markdown(None, &messages).starts_with("# Veritas session"));
    }
}

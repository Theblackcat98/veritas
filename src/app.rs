use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use ratatui_textarea::{Input, TextArea};
use tokio::runtime::Handle;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::provider::{self, StreamEvent, WireMessage};
use crate::session;
use crate::sysmon::SysStats;

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    User,
    Agent,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Clone)]
pub struct Config {
    pub base_url: String,
    pub api_key: String,
    /// `OPENAI_MODEL` when explicitly set; None = auto-detect (D030).
    pub model: Option<String>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("base_url", &self.base_url)
            .field("api_key", &"***")
            .field("model", &self.model)
            .finish()
    }
}

impl Config {
    pub fn from_env() -> Self {
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        let model = std::env::var("OPENAI_MODEL").ok().filter(|s| !s.is_empty());
        Self {
            base_url,
            api_key,
            model,
        }
    }
}

/// Transcript scroll ceiling. Doubles as the u16 overflow guard for
/// ratatui 0.30's `area.height + scroll` and bounds the follow-tail range
/// (D011).
pub const MAX_SCROLL: u16 = 60_000;

/// Auto-hide the sidebar below this geometry (D032): 32-col sidebar +
/// 40-col minimum main pane; height ≈ the no-GPU widget stack plus slack.
pub const SIDEBAR_MIN_WIDTH: u16 = 72;
pub const SIDEBAR_MIN_HEIGHT: u16 = 20;

/// Phase 5 catalog (D031): signature + one-line description, alphabetical.
/// Single source of truth for the `/` popup; `/help` stays curated.
pub const COMMANDS: &[(&str, &str)] = &[
    ("/clear", "Reset the conversation"),
    ("/copy [n]", "Copy code block to clipboard"),
    ("/export <file>", "Export transcript as markdown"),
    ("/help", "Show this help"),
    ("/model <id>", "Set model — bare re-detects"),
    ("/quit", "Exit Veritas"),
    ("/session", "Open session picker (alias)"),
    ("/sessions", "Open session picker"),
    ("/sidebar", "Toggle sidebar visibility"),
    ("/temp <0.0-2.0>", "Override temperature"),
];

/// Case-insensitive subsequence match of `query` against `target`.
/// Lower score = closer match; `None` = no match. Each skipped character
/// costs +1; a pure prefix therefore scores 0 (D031).
pub fn fuzzy_score(query: &str, target: &str) -> Option<u32> {
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let t: Vec<char> = target.to_lowercase().chars().collect();
    if q.is_empty() {
        return Some(0);
    }
    if q.len() > t.len() {
        return None;
    }
    let mut qi = 0;
    let mut score: u32 = 0;
    for &c in &t {
        if qi >= q.len() {
            break;
        }
        if c == q[qi] {
            qi += 1;
        } else {
            score += 1;
        }
    }
    if qi < q.len() {
        None
    } else {
        Some(score)
    }
}

/// Fuzzy-ranked commands for the `/` popup (D031). `query` excludes the
/// leading `/`. Equal scores keep `COMMANDS` declaration order.
pub fn match_commands(query: &str) -> Vec<(&'static str, &'static str)> {
    let mut scored: Vec<(u32, &'static str, &'static str)> = COMMANDS
        .iter()
        .filter_map(|&(cmd, desc)| {
            let name = cmd.trim_start_matches('/').split(' ').next().unwrap_or("");
            fuzzy_score(query, name).map(|score| (score, cmd, desc))
        })
        .collect();
    // Stable sort: declaration order is the tiebreak for equal scores.
    scored.sort_by_key(|&(score, _, _)| score);
    scored
        .into_iter()
        .map(|(_, cmd, desc)| (cmd, desc))
        .collect()
}

/// Session-picker overlay state (D013). Keyboard-only; owns no I/O —
/// storage goes through `session`.
pub struct Picker {
    pub entries: Vec<session::SessionEntry>,
    pub selected: usize,
}

pub struct App<'a> {
    pub messages: Vec<ChatMessage>,
    pub input: TextArea<'a>,
    pub scroll: u16,
    pub follow: bool,
    pub streaming: bool,
    /// Tick counter for tui-spinner widgets (D029).
    pub spinner_tick: u64,
    /// Whether the model context window is actively being probed from the engine.
    pub probing_ctx: bool,
    /// Active model id; None = engine unknown → sidebar shows `n/a` (D030).
    pub model: Option<String>,
    /// Whether a `GET /models` probe is in flight (D030).
    pub probing_model: bool,
    pub status: String,
    pub config: Config,
    pub stats: SysStats,
    rx: Option<UnboundedReceiver<StreamEvent>>,
    /// Handle of the in-flight stream task — aborted on Esc-cancel so the
    /// HTTP body stops downloading (refines D002's drop-the-receiver).
    stream_task: Option<tokio::task::JoinHandle<()>>,
    pub pending: String,
    /// Runtime `/temp` override; None = engine's own temperature (D009).
    pub temperature_override: Option<f32>,
    /// Context window probed from the engine; None = unknown → sidebar
    /// shows n/a (D009).
    pub ctx_window: Option<u64>,
    /// Active session file stem; None until the first message is saved (D012).
    pub session_id: Option<String>,
    /// Auto-title from the first user message (D012).
    pub session_title: Option<String>,
    /// created_at stamp for the active session; 0 = no session yet.
    pub session_created: u64,
    /// Message count at the last successful save — auto-save dirty check.
    last_saved_len: usize,
    /// Open picker overlay; while `Some`, every key routes to it (D013).
    pub picker: Option<Picker>,
    /// Open help overlay; while `true`, every key routes to it (Esc closes).
    pub help_open: bool,
    /// Manual sidebar override; None = auto by terminal size (D032).
    pub sidebar_override: Option<bool>,
    /// Visibility last drawn — what Ctrl-B flips relative to (D032).
    pub sidebar_effective: bool,
    /// Esc dismissed the `/` popup until the input text changes (D031).
    pub cmd_popup_dismissed: bool,
    /// Selected row in the `/` popup (D033); reset on text change.
    pub cmd_popup_selected: usize,
    /// `/quit` raised this flag; `run` exits after the event (D034).
    pub quit: bool,
    /// Phase (0.0..1.0) of the streaming shimmer sweep (D017).
    pub shimmer_phase: f32,
    meta_tx: UnboundedSender<(String, Option<u64>)>,
    meta_rx: Option<UnboundedReceiver<(String, Option<u64>)>>,
    model_tx: UnboundedSender<Option<String>>,
    model_rx: Option<UnboundedReceiver<Option<String>>>,
}

impl<'a> App<'a> {
    pub fn new(config: Config) -> Self {
        let mut input = TextArea::default();
        input.set_placeholder_text("Type a message — Enter to send, Shift+Enter newline…");
        let (meta_tx, meta_rx) = unbounded_channel::<(String, Option<u64>)>();
        let (model_tx, model_rx) = unbounded_channel::<Option<String>>();
        let model = config.model.clone();
        let mut app = Self {
            // Start empty — a seeded greeting would be sent to the model as
            // if it had said it.
            messages: Vec::new(),
            input,
            scroll: 0,
            follow: true,
            streaming: false,
            spinner_tick: 0,
            probing_ctx: false,
            model,
            probing_model: false,
            status: "Ready — Enter send · PgUp/PgDn scroll · Ctrl-S sessions · Ctrl-C quit"
                .to_string(),
            config,
            stats: SysStats::new(),
            rx: None,
            stream_task: None,
            pending: String::new(),
            temperature_override: None,
            ctx_window: None,
            session_id: None,
            session_title: None,
            session_created: 0,
            last_saved_len: 0,
            picker: None,
            help_open: false,
            sidebar_override: None,
            sidebar_effective: true,
            cmd_popup_dismissed: false,
            cmd_popup_selected: 0,
            quit: false,
            shimmer_phase: 0.0,
            meta_tx,
            meta_rx: Some(meta_rx),
            model_tx,
            model_rx: Some(model_rx),
        };
        if app.model.is_none() {
            // No OPENAI_MODEL: ask the engine what it has (D030).
            app.spawn_model_probe();
        } else {
            app.spawn_ctx_probe();
        }
        app
    }

    /// Ask the inference engine for the model's context window; the answer
    /// arrives on `meta_rx` and is applied in `drain_meta` (D009).
    fn spawn_ctx_probe(&mut self) {
        let Some(model) = self.model.clone() else {
            return;
        };
        self.probing_ctx = true;
        let base = self.config.base_url.clone();
        let tx = self.meta_tx.clone();
        Handle::current().spawn(async move {
            let ctx = provider::fetch_context_length(base, model.clone()).await;
            let _ = tx.send((model, ctx));
        });
    }

    /// Probe `GET {base}/models` and adopt the first id (D030); the answer
    /// arrives on `model_rx` and is applied in `drain_model`.
    fn spawn_model_probe(&mut self) {
        self.probing_model = true;
        let base = self.config.base_url.clone();
        let key = self.config.api_key.clone();
        let tx = self.model_tx.clone();
        Handle::current().spawn(async move {
            let m = provider::fetch_models(base, key).await;
            let _ = tx.send(m);
        });
    }

    fn drain_model(&mut self) {
        let mut answer: Option<Option<String>> = None;
        if let Some(rx) = self.model_rx.as_mut() {
            while let Ok(m) = rx.try_recv() {
                answer = Some(m);
            }
        }
        let Some(detected) = answer else {
            return;
        };
        self.probing_model = false;
        match detected {
            Some(id) => {
                let changed = self.model.as_deref() != Some(id.as_str());
                self.model = Some(id.clone());
                if changed || self.ctx_window.is_none() {
                    self.spawn_ctx_probe();
                }
                if !self.streaming {
                    self.status = format!("Detected model {id}.");
                }
            }
            None => {
                self.model = None;
                if !self.streaming {
                    self.status =
                        "No model — engine unreachable or empty. /model <id> to set manually."
                            .to_string();
                }
            }
        }
    }

    fn drain_meta(&mut self) {
        if let Some(rx) = self.meta_rx.as_mut() {
            while let Ok((model, ctx)) = rx.try_recv() {
                // Ignore stale answers for a model we've already switched away from.
                if Some(model.as_str()) == self.model.as_deref() {
                    self.ctx_window = ctx;
                    self.probing_ctx = false;
                }
            }
        }
    }

    /// First user message content, or "" — auto-title source (D012).
    fn first_user_text(&self) -> String {
        self.messages
            .iter()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.clone())
            .unwrap_or_default()
    }

    /// Persist the transcript (D012): after each finished/cancelled stream
    /// and before opening the picker. Cheap atomic writes; the empty
    /// transcript resets session identity so /clear starts fresh.
    fn auto_save(&mut self) {
        if self.messages.is_empty() {
            self.session_id = None;
            self.session_title = None;
            self.session_created = 0;
            self.last_saved_len = 0;
            return;
        }
        if self.last_saved_len == self.messages.len() {
            return; // nothing appended since the last write
        }
        if self.session_created == 0 {
            self.session_created = session::now_unix();
        }
        let dir = session::sessions_dir();
        if self.session_id.is_none() {
            let base = session::new_id();
            self.session_id = Some(session::unique_id(&dir, &base));
        }
        let title = match self.session_title.clone() {
            Some(t) => t,
            None => {
                let t = session::auto_title(&self.first_user_text());
                self.session_title = Some(t.clone());
                t
            }
        };
        let id = self.session_id.as_deref().expect("just set").to_string();
        let created = self.session_created;
        match session::save(&dir, &id, Some(&title), created, &self.messages) {
            Ok(()) => {
                self.last_saved_len = self.messages.len();
                self.status = format!(
                    "Saved {id} · {} msgs — Ctrl-S sessions",
                    self.messages.len()
                );
            }
            Err(e) => self.status = format!("Session save failed: {e}"),
        }
    }

    /// Open the session picker, saving any unsaved transcript first (D013).
    pub fn open_picker(&mut self) {
        if self.streaming {
            self.status = "Stop the stream first (Esc), then Ctrl-S.".to_string();
            return;
        }
        self.auto_save();
        let entries = session::list(&session::sessions_dir());
        self.picker = Some(Picker {
            entries,
            selected: 0,
        });
    }

    pub fn close_picker(&mut self) {
        self.picker = None;
    }

    /// Open the help overlay.
    pub fn open_help(&mut self) {
        self.help_open = true;
    }

    /// Close the help overlay.
    pub fn close_help(&mut self) {
        self.help_open = false;
    }

    /// Effective sidebar visibility (D032): session override beats auto.
    pub fn sidebar_visible(&self, area: Rect) -> bool {
        self.sidebar_override
            .unwrap_or(area.width >= SIDEBAR_MIN_WIDTH && area.height >= SIDEBAR_MIN_HEIGHT)
    }

    /// Flip sidebar visibility relative to what is currently effective (D032).
    pub fn toggle_sidebar(&mut self) {
        let show = !self.sidebar_effective;
        self.sidebar_override = Some(show);
        self.status = if show {
            "Sidebar shown.".to_string()
        } else {
            "Sidebar hidden.".to_string()
        };
    }

    /// Fuzzy matches for the `/` popup; empty when input isn't a `/` prefix
    /// or the query no longer matches any command (D031).
    pub fn cmd_popup_matches(&self) -> Vec<(&'static str, &'static str)> {
        let Some(first) = self.input.lines().first() else {
            return Vec::new();
        };
        let Some(query) = first.strip_prefix('/') else {
            return Vec::new();
        };
        match_commands(query)
    }

    /// Whether the command popup should be drawn this frame (D031).
    pub fn cmd_popup_visible(&self) -> bool {
        !self.cmd_popup_dismissed && !self.cmd_popup_matches().is_empty()
    }

    /// Move the popup selection with wrap-around (D033).
    pub fn cmd_popup_move(&mut self, delta: i32) {
        let n = self.cmd_popup_matches().len();
        if n == 0 {
            return;
        }
        self.cmd_popup_selected =
            (self.cmd_popup_selected as i32 + delta).rem_euclid(n as i32) as usize;
    }

    /// Tab: replace the input with the selected row's full signature so
    /// placeholder args can be edited (D033).
    pub fn cmd_popup_complete(&mut self) {
        let Some((cmd, _)) = self
            .cmd_popup_matches()
            .get(self.cmd_popup_selected)
            .copied()
        else {
            return;
        };
        self.input = TextArea::default();
        self.input.insert_str(cmd);
        self.cmd_popup_dismissed = false;
    }

    /// Enter: run the selected command's first token only — never the
    /// placeholder text (so `/export <file>` bare shows the usage hint) (D033).
    pub fn cmd_popup_run(&mut self) {
        let Some((cmd, _)) = self
            .cmd_popup_matches()
            .get(self.cmd_popup_selected)
            .copied()
        else {
            return;
        };
        let name = cmd
            .trim_start_matches('/')
            .split(' ')
            .next()
            .unwrap_or("")
            .to_string();
        self.input = TextArea::default();
        self.handle_command(&name);
    }

    /// Modal key routing while the picker is open (D013).
    pub fn handle_picker_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.close_picker(),
            KeyCode::Up => self.picker_move(-1),
            KeyCode::Down => self.picker_move(1),
            KeyCode::PageUp => self.picker_move(-10),
            KeyCode::PageDown => self.picker_move(10),
            KeyCode::Enter => self.confirm_load(),
            _ => {}
        }
    }

    /// Wrap-around selection move; delta may be negative.
    pub fn picker_move(&mut self, delta: i32) {
        if let Some(p) = &mut self.picker {
            let n = p.entries.len();
            if n == 0 {
                return;
            }
            p.selected = (p.selected as i32 + delta).rem_euclid(n as i32) as usize;
        }
    }

    /// Enter in the picker: load the highlighted session (D013).
    pub fn confirm_load(&mut self) {
        let id = match self.picker.as_ref().and_then(|p| p.entries.get(p.selected)) {
            Some(e) => e.id.clone(),
            None => return,
        };
        self.picker = None;
        match session::load(&session::sessions_dir(), &id) {
            Ok((title, created, messages)) => {
                // Rough ctx estimate until the next stream sends usage (D009).
                let chars: usize = messages.iter().map(|m| m.content.len()).sum();
                self.messages = messages;
                self.session_id = Some(id);
                self.session_title = title;
                self.session_created = created;
                self.last_saved_len = self.messages.len();
                self.stats.ctx_used = (chars / 4) as u64;
                self.follow = true; // Follow mode pins to bottom, no need to set scroll
                self.status = format!(
                    "Loaded “{}” · {} msgs",
                    self.session_title.as_deref().unwrap_or("session"),
                    self.messages.len()
                );
            }
            Err(e) => self.status = format!("Load failed: {e}"),
        }
    }

    /// `/export <file>` — transcript as markdown (D014).
    fn export_session(&mut self, path: &str) {
        if self.messages.is_empty() {
            self.status = "Nothing to export.".to_string();
            return;
        }
        let md = session::export_markdown(self.session_title.as_deref(), &self.messages);
        match std::fs::write(path, md) {
            Ok(()) => {
                self.status = format!("Exported {} msgs to {path}", self.messages.len());
            }
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }

    /// Copy a code block from the transcript to the clipboard (D026).
    /// `target`: None copies the latest code block; Some(n) copies block n (1-indexed).
    pub fn copy_code_block(&mut self, target: Option<usize>) {
        let contents = self
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .chain(std::iter::once(self.pending.as_str()).filter(|s| !s.is_empty()));
        let blocks = extract_code_blocks(contents);

        if blocks.is_empty() {
            self.status = "No code blocks found in transcript.".to_string();
            return;
        }

        let total = blocks.len();
        let (idx, is_latest) = match target {
            None => (total - 1, true),
            Some(n) => {
                if n == 0 || n > total {
                    self.status = format!("Invalid code block #{n} (available: 1..{total}).");
                    return;
                }
                (n - 1, n == total)
            }
        };

        let block = &blocks[idx];
        let lines_count = block.lines().count();

        if copy_to_clipboard(block) {
            let label = if is_latest {
                format!("block {total}/{total} (latest, {lines_count} lines)")
            } else {
                format!("block {}/{total} ({lines_count} lines)", idx + 1)
            };
            self.status = format!("Copied {label} to clipboard.");
        } else {
            self.status = "Failed to copy to clipboard.".to_string();
        }
    }

    fn history_for_api(&self) -> Vec<WireMessage> {
        self.messages
            .iter()
            .map(|m| WireMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Agent => "assistant".to_string(),
                },
                content: m.content.clone(),
            })
            .collect()
    }

    /// Slash commands. Never enter the chat history (D009).
    fn handle_command(&mut self, cmd: &str) {
        let mut parts = cmd.split_whitespace();
        match parts.next() {
            Some("clear") => {
                self.messages.clear();
                self.stats.ctx_used = 0;
                self.scroll = 0;
                self.follow = true;
                // Fresh identity: the next save must not overwrite the file
                // that held the cleared conversation (D012).
                self.session_id = None;
                self.session_title = None;
                self.session_created = 0;
                self.last_saved_len = 0;
                self.status = "Cleared.".to_string();
            }
            Some("help") => {
                self.open_help();
            }
            Some("copy") => match parts.next() {
                Some(v) => match v.parse::<usize>() {
                    Ok(n) => self.copy_code_block(Some(n)),
                    Err(_) => self.status = "Usage: /copy [1-based index]".to_string(),
                },
                None => self.copy_code_block(None),
            },
            Some("sessions") | Some("session") => self.open_picker(),
            Some("sidebar") => self.toggle_sidebar(),
            Some("quit") => self.quit = true,
            Some("export") => {
                // Take the rest of the line so paths with spaces work.
                let rest = cmd.get("export".len()..).unwrap_or("").trim();
                if rest.is_empty() {
                    self.status = "Usage: /export <file>".to_string();
                } else {
                    self.export_session(rest);
                }
            }
            Some("model") => match parts.next() {
                Some(id) => {
                    self.model = Some(id.to_string());
                    self.ctx_window = None;
                    self.spawn_ctx_probe();
                    self.status = format!("Model set to {id}.");
                }
                None => {
                    // Bare /model: re-detect from the engine (D030).
                    self.ctx_window = None;
                    self.spawn_model_probe();
                    self.status = "Re-detecting model from engine…".to_string();
                }
            },
            Some("temp") => match parts.next() {
                Some(v) => match v.parse::<f32>() {
                    Ok(t) if (0.0..=2.0).contains(&t) => {
                        self.temperature_override = Some(t);
                        self.status = format!("Temperature set to {t} (overrides engine).");
                    }
                    _ => self.status = "Usage: /temp <0.0-2.0>".to_string(),
                },
                None => {
                    self.status = match self.temperature_override {
                        Some(t) => {
                            format!("Temperature: {t} (override; engine default otherwise)")
                        }
                        None => "Temperature: engine default".to_string(),
                    };
                }
            },
            _ => {
                self.status = format!("Unknown command: /{cmd} — try /help");
            }
        }
    }

    fn send_current(&mut self) {
        if self.streaming {
            return;
        }
        let text = self.input.lines().join("\n").trim().to_string();
        if text.is_empty() {
            self.status = "Type something first.".to_string();
            return;
        }
        if let Some(cmd) = text.strip_prefix('/') {
            self.handle_command(cmd);
            self.input = TextArea::default();
            return;
        }
        // No model known: re-probe once instead of firing a doomed request
        // (D030). Keep the typed text so Enter can be pressed again.
        let Some(model) = self.model.clone() else {
            if self.probing_model {
                self.status = "No model yet — probing engine… or /model <id>.".to_string();
            } else {
                self.spawn_model_probe();
                self.status = "No model — probing engine; Enter again or /model <id>.".to_string();
            }
            return;
        };
        self.messages.push(ChatMessage {
            role: Role::User,
            content: text,
        });
        self.input = TextArea::default();
        self.follow = true;
        // Scroll value itself is clamped in ui.rs; follow flag drives bottom stick.

        let (tx, rx) = unbounded_channel::<StreamEvent>();
        self.rx = Some(rx);
        self.pending.clear();
        self.streaming = true;
        self.status = "Streaming… (Esc to stop)".to_string();

        let mut history = self.history_for_api();
        // Rough ctx estimate until/unless the server sends real usage (D009).
        let chars: usize = history.iter().map(|m| m.content.len()).sum();
        self.stats.ctx_used = (chars / 4) as u64;

        let base = self.config.base_url.clone();
        let key = self.config.api_key.clone();
        // None = omit temperature from the request; engine default applies.
        let temperature = self.temperature_override;
        // Fire-and-forget; results come back over `tx`. Dropping `rx` on
        // cancel/stop makes late sends no-ops; the task handle lets Esc
        // abort the download outright.
        let handle = Handle::current().spawn(async move {
            provider::stream_chat(
                base,
                key,
                model,
                temperature,
                std::mem::take(&mut history),
                tx,
            )
            .await;
        });
        self.stream_task = Some(handle);
    }

    fn cancel_stream(&mut self) {
        if let Some(task) = self.stream_task.take() {
            task.abort();
        }
        self.rx = None;
        self.streaming = false;
        if !self.pending.is_empty() {
            self.messages.push(ChatMessage {
                role: Role::Agent,
                content: std::mem::take(&mut self.pending),
            });
        }
        self.auto_save();
        self.status = "Stopped.".to_string();
    }

    fn drain_stream(&mut self) {
        // Field-disjoint borrows: `rx` is read while `pending`/`messages`
        // are mutated below the loop.
        use tokio::sync::mpsc::error::TryRecvError;
        let mut done = false;
        let mut err: Option<String> = None;
        if let Some(rx) = self.rx.as_mut() {
            loop {
                match rx.try_recv() {
                    Ok(ev) => match ev {
                        StreamEvent::Token(t) => {
                            self.pending.push_str(&t);
                            self.stats.ctx_used += (t.len() / 4) as u64;
                        }
                        StreamEvent::Usage {
                            prompt_tokens,
                            completion_tokens,
                        } => {
                            // Server-reported truth beats the chars/4 estimate (D009).
                            self.stats.ctx_used = prompt_tokens + completion_tokens;
                        }
                        StreamEvent::Done => {
                            done = true;
                            break;
                        }
                        StreamEvent::Error(e) => {
                            err = Some(e);
                            break;
                        }
                    },
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // Task died without Done/Error — finish honestly
                        // instead of spinning forever.
                        err = Some("stream disconnected".to_string());
                        break;
                    }
                }
            }
        }
        if err.is_some() || done {
            if let Some(task) = self.stream_task.take() {
                task.abort(); // no-op when the task already finished
            }
        }
        if let Some(e) = err {
            self.streaming = false;
            self.rx = None;
            if !self.pending.is_empty() {
                self.messages.push(ChatMessage {
                    role: Role::Agent,
                    content: std::mem::take(&mut self.pending),
                });
            }
            self.auto_save();
            self.status = format!("Error: {e}");
        } else if done {
            self.streaming = false;
            self.rx = None;
            if !self.pending.is_empty() {
                self.messages.push(ChatMessage {
                    role: Role::Agent,
                    content: std::mem::take(&mut self.pending),
                });
            } else {
                self.messages.push(ChatMessage {
                    role: Role::Agent,
                    content: "(empty response)".to_string(),
                });
            }
            self.auto_save();
            self.status =
                "Ready — Enter send · PgUp/PgDn scroll · Ctrl-S sessions · Ctrl-C quit".to_string();
        }
    }
}

pub fn run<B: Backend>(terminal: &mut Terminal<B>, config: Config) -> io::Result<()>
where
    io::Error: From<B::Error>,
{
    let mut app = App::new(config);
    let mut sampler = crate::sysmon::Sampler::new();
    // Sample once before the first draw so widgets never show placeholder zeros.
    sampler.sample(&mut app.stats);
    let mut last_stats = Instant::now();
    let mut last_spinner = Instant::now();

    loop {
        // `/quit` (D034) — checked at loop top so it fires whether the
        // event path `continue`d or fell through; same cleanup as Ctrl-C.
        if app.quit {
            return Ok(());
        }
        terminal.draw(|f| crate::ui::draw(f, &mut app))?;

        let animating = app.streaming || (app.probing_ctx && app.ctx_window.is_none());
        let timeout = if animating {
            Duration::from_millis(8)
        } else {
            Duration::from_millis(100)
        };

        if last_stats.elapsed() >= Duration::from_millis(500) {
            sampler.sample(&mut app.stats);
            last_stats = Instant::now();
        }
        if animating && last_spinner.elapsed() >= Duration::from_millis(80) {
            app.spinner_tick = app.spinner_tick.wrapping_add(1);
            // Shimmer sweep shares the spinner tick (D017): 80 ms × ~25
            // steps ≈ a 2 s sweep, matching tui-shimmer's default cadence.
            if app.streaming {
                app.shimmer_phase = (app.shimmer_phase + 0.04) % 1.0;
            }
            last_spinner = Instant::now();
        }

        app.drain_stream();
        app.drain_meta();
        app.drain_model();

        if !event::poll(timeout)? {
            continue;
        }
        match event::read()? {
            Event::Key(key) => {
                if key.kind != event::KeyEventKind::Press {
                    continue;
                }
                // Global shortcuts first.
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
                {
                    return Ok(());
                }
                // Picker is modal: it swallows every key while open (D013).
                if app.picker.is_some() {
                    app.handle_picker_key(key.code);
                    continue;
                }
                // Help overlay is modal: Esc closes it.
                if app.help_open {
                    if key.code == KeyCode::Esc {
                        app.close_help();
                    }
                    continue;
                }
                // Ctrl-S: save-then-list the sessions (D012/D013).
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
                    app.open_picker();
                    continue;
                }
                // Ctrl-Y: copy latest code block to clipboard (D026).
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('y') {
                    app.copy_code_block(None);
                    continue;
                }
                // Ctrl-B: toggle sidebar visibility (D032).
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b') {
                    app.toggle_sidebar();
                    continue;
                }
                // Esc: cancel active stream, else dismiss the command popup (D031).
                if key.code == KeyCode::Esc {
                    if app.streaming {
                        app.cancel_stream();
                        continue;
                    }
                    if app.cmd_popup_visible() {
                        app.cmd_popup_dismissed = true;
                        continue;
                    }
                }
                // While the popup is open: ↑/↓ select, Tab completes (D033).
                // Modifier-carrying arrows keep their scroll meaning.
                let unmodified = !key.modifiers.contains(KeyModifiers::SHIFT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL);
                if app.cmd_popup_visible() && unmodified {
                    match key.code {
                        KeyCode::Up => {
                            app.cmd_popup_move(-1);
                            continue;
                        }
                        KeyCode::Down => {
                            app.cmd_popup_move(1);
                            continue;
                        }
                        KeyCode::Tab => {
                            app.cmd_popup_complete();
                            continue;
                        }
                        _ => {}
                    }
                }
                // Enter: send — or run the highlighted command while the
                // popup is open (D033).
                if key.code == KeyCode::Enter && !key.modifiers.contains(KeyModifiers::SHIFT) {
                    if app.cmd_popup_visible() {
                        app.cmd_popup_run();
                    } else {
                        app.send_current();
                    }
                    continue;
                }
                // Page up / scroll up: PgUp, Shift+Up, Ctrl+Up, Ctrl-U
                let is_scroll_up = key.code == KeyCode::PageUp
                    || (key.code == KeyCode::Up
                        && (key.modifiers.contains(KeyModifiers::SHIFT)
                            || key.modifiers.contains(KeyModifiers::CONTROL)))
                    || (key.code == KeyCode::Char('u')
                        && key.modifiers.contains(KeyModifiers::CONTROL));
                if is_scroll_up {
                    app.follow = false;
                    app.scroll = app.scroll.saturating_sub(15);
                    continue;
                }
                // Page down / scroll down: PgDn, Shift+Down, Ctrl+Down, Ctrl-D
                let is_scroll_down = key.code == KeyCode::PageDown
                    || (key.code == KeyCode::Down
                        && (key.modifiers.contains(KeyModifiers::SHIFT)
                            || key.modifiers.contains(KeyModifiers::CONTROL)))
                    || (key.code == KeyCode::Char('d')
                        && key.modifiers.contains(KeyModifiers::CONTROL));
                if is_scroll_down {
                    app.scroll = app.scroll.saturating_add(15).min(MAX_SCROLL);
                    continue;
                }
                // Everything else goes to the textarea. A text change
                // re-opens a dismissed command popup (D031).
                let before = app.input.lines().join("\n");
                app.input.input(Input::from(Event::Key(key)));
                if app.input.lines().join("\n") != before {
                    app.cmd_popup_dismissed = false;
                    app.cmd_popup_selected = 0;
                }
            }
            Event::Mouse(me) => match me.kind {
                MouseEventKind::ScrollUp => {
                    app.follow = false;
                    app.scroll = app.scroll.saturating_sub(3);
                }
                MouseEventKind::ScrollDown => {
                    app.scroll = app.scroll.saturating_add(3).min(MAX_SCROLL);
                }
                _ => {}
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

/// RFC 4648 standard base64 encoding (zero dependencies).
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        result.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        result.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn copy_via_osc52(text: &str) -> io::Result<()> {
    let encoded = base64_encode(text.as_bytes());
    let mut stdout = io::stdout().lock();
    if std::env::var("TMUX").is_ok() {
        write!(stdout, "\x1bPtmux;\x1b\x1b]52;c;{}\x07\x1b\\", encoded)?;
    } else {
        write!(stdout, "\x1b]52;c;{}\x07", encoded)?;
    }
    stdout.flush()
}

fn copy_via_command(cmd: &str, args: &[&str], text: &str) -> io::Result<()> {
    use std::process::{Command, Stdio};
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("command failed"))
    }
}

pub fn copy_to_clipboard(text: &str) -> bool {
    let mut command_success = false;

    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        if copy_via_command("wl-copy", &[], text).is_ok() {
            command_success = true;
        }
    } else if std::env::var("DISPLAY").is_ok() {
        if copy_via_command("xclip", &["-selection", "clipboard"], text).is_ok()
            || copy_via_command("xsel", &["--clipboard", "--input"], text).is_ok()
        {
            command_success = true;
        }
    } else if cfg!(target_os = "macos") && copy_via_command("pbcopy", &[], text).is_ok() {
        command_success = true;
    }

    let osc52_success = copy_via_osc52(text).is_ok();
    command_success || osc52_success
}

pub fn extract_code_blocks<'a>(contents: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut blocks = Vec::new();
    for content in contents {
        let mut fence_len = 0;
        let mut current_block = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim_start();
            if fence_len == 0 {
                let backticks = trimmed.chars().take_while(|&c| c == '`').count();
                if backticks >= 3 {
                    fence_len = backticks;
                    current_block.clear();
                }
            } else {
                let fence_trimmed = trimmed.trim_end();
                let backticks = fence_trimmed.chars().take_while(|&c| c == '`').count();
                if backticks >= fence_len && fence_trimmed.len() == backticks {
                    fence_len = 0;
                    blocks.push(current_block.join("\n"));
                    current_block.clear();
                } else {
                    current_block.push(line);
                }
            }
        }
        if fence_len > 0 && !current_block.is_empty() {
            blocks.push(current_block.join("\n"));
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_extract_code_blocks() {
        let text1 = "Here is some text:\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\nAnd more text.";
        let text2 = "```python\nprint('hello')\n```";
        let blocks = extract_code_blocks(vec![text1, text2]);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0], "fn main() {\n    println!(\"hi\");\n}");
        assert_eq!(blocks[1], "print('hello')");
    }

    #[test]
    fn test_extract_nested_fences() {
        let text = "````markdown\n```rust\nlet x = 1;\n```\n````";
        let blocks = extract_code_blocks(vec![text]);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], "```rust\nlet x = 1;\n```");
    }

    #[test]
    fn test_extract_unclosed_block() {
        let text = "```c\nint x = 42;";
        let blocks = extract_code_blocks(vec![text]);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], "int x = 42;");
    }

    #[test]
    fn test_handle_command_session_alias() {
        let (meta_tx, _meta_rx) = unbounded_channel();
        let (model_tx, _model_rx) = unbounded_channel::<Option<String>>();
        let mut app = App {
            messages: Vec::new(),
            input: TextArea::default(),
            scroll: 0,
            follow: true,
            streaming: false,
            spinner_tick: 0,
            probing_ctx: false,
            model: Some("test-model".to_string()),
            probing_model: false,
            status: String::new(),
            config: Config {
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: String::new(),
                model: Some("test-model".to_string()),
            },
            stats: SysStats::new(),
            rx: None,
            stream_task: None,
            pending: String::new(),
            temperature_override: None,
            ctx_window: None,
            session_id: None,
            session_title: None,
            session_created: 0,
            last_saved_len: 0,
            picker: None,
            help_open: false,
            sidebar_override: None,
            sidebar_effective: true,
            cmd_popup_dismissed: false,
            cmd_popup_selected: 0,
            quit: false,
            shimmer_phase: 0.0,
            meta_tx,
            meta_rx: None,
            model_tx,
            model_rx: None,
        };

        // Test /sessions opens picker
        app.handle_command("sessions");
        assert!(app.picker.is_some());
        app.close_picker();
        assert!(app.picker.is_none());

        // Test /session alias opens picker
        app.handle_command("session");
        assert!(app.picker.is_some());
    }

    #[test]
    fn fuzzy_score_ranks_prefix_and_subsequence() {
        assert_eq!(fuzzy_score("", "help"), Some(0));
        assert_eq!(fuzzy_score("he", "help"), Some(0), "prefix scores 0");
        assert_eq!(fuzzy_score("HELP", "help"), Some(0), "case-insensitive");
        assert_eq!(fuzzy_score("hl", "help"), Some(1), "one skipped char");
        assert_eq!(fuzzy_score("zzz", "help"), None);
        assert_eq!(
            fuzzy_score("export", "exp"),
            None,
            "query longer than target"
        );
        // Scattered match ranks worse than a contiguous prefix.
        let export = fuzzy_score("ep", "export").expect("export matches");
        let help = fuzzy_score("ep", "help").expect("help matches");
        assert!(export < help, "export({export}) < help({help})");
    }

    #[test]
    fn match_commands_finds_and_ranks_closest() {
        let all = match_commands("");
        assert_eq!(all.len(), COMMANDS.len(), "bare / lists every command");

        let he = match_commands("he");
        assert_eq!(he[0].0, "/help", "closest match first: {he:?}");
        assert!(he.iter().all(|(c, _)| *c == "/help"), "only help matches");

        let s = match_commands("s");
        let names: Vec<&str> = s.iter().map(|(c, _)| *c).collect();
        assert!(names.contains(&"/sessions"));
        assert!(names.contains(&"/sidebar"));
        assert!(!names.contains(&"/help"), "help has no 's'");

        assert!(match_commands("zz").is_empty());
        // Full command with arguments no longer matches the bare name.
        assert!(match_commands("export myfile.md").is_empty());
    }

    #[test]
    fn sidebar_command_toggles_relative_to_effective() {
        let (meta_tx, _meta_rx) = unbounded_channel();
        let (model_tx, _model_rx) = unbounded_channel::<Option<String>>();
        let mut app = App {
            messages: Vec::new(),
            input: TextArea::default(),
            scroll: 0,
            follow: true,
            streaming: false,
            spinner_tick: 0,
            probing_ctx: false,
            model: Some("test-model".to_string()),
            probing_model: false,
            status: String::new(),
            config: Config {
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: String::new(),
                model: Some("test-model".to_string()),
            },
            stats: SysStats::new(),
            rx: None,
            stream_task: None,
            pending: String::new(),
            temperature_override: None,
            ctx_window: None,
            session_id: None,
            session_title: None,
            session_created: 0,
            last_saved_len: 0,
            picker: None,
            help_open: false,
            sidebar_override: None,
            sidebar_effective: true,
            cmd_popup_dismissed: false,
            cmd_popup_selected: 0,
            quit: false,
            shimmer_phase: 0.0,
            meta_tx,
            meta_rx: None,
            model_tx,
            model_rx: None,
        };

        // Auto-visible → toggle hides.
        app.handle_command("sidebar");
        assert_eq!(app.sidebar_override, Some(false));
        assert_eq!(app.status, "Sidebar hidden.");

        // Currently hidden → toggle force-shows (even if auto would hide).
        app.sidebar_effective = false;
        app.handle_command("sidebar");
        assert_eq!(app.sidebar_override, Some(true));
        assert_eq!(app.status, "Sidebar shown.");

        // Override beats auto: narrow terminal still shows when forced.
        let narrow = Rect::new(0, 0, 60, 30);
        assert!(app.sidebar_visible(narrow));
        app.sidebar_override = Some(false);
        assert!(!app.sidebar_visible(Rect::new(0, 0, 80, 24)));

        // No override → auto thresholds.
        app.sidebar_override = None;
        assert!(app.sidebar_visible(Rect::new(0, 0, 80, 24)));
        assert!(!app.sidebar_visible(Rect::new(0, 0, 71, 24)));
        assert!(!app.sidebar_visible(Rect::new(0, 0, 80, 19)));
    }

    #[test]
    fn cmd_popup_visibility_rules() {
        let (meta_tx, _meta_rx) = unbounded_channel();
        let (model_tx, _model_rx) = unbounded_channel::<Option<String>>();
        let mut app = App {
            messages: Vec::new(),
            input: TextArea::default(),
            scroll: 0,
            follow: true,
            streaming: false,
            spinner_tick: 0,
            probing_ctx: false,
            model: Some("test-model".to_string()),
            probing_model: false,
            status: String::new(),
            config: Config {
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: String::new(),
                model: Some("test-model".to_string()),
            },
            stats: SysStats::new(),
            rx: None,
            stream_task: None,
            pending: String::new(),
            temperature_override: None,
            ctx_window: None,
            session_id: None,
            session_title: None,
            session_created: 0,
            last_saved_len: 0,
            picker: None,
            help_open: false,
            sidebar_override: None,
            sidebar_effective: true,
            cmd_popup_dismissed: false,
            cmd_popup_selected: 0,
            quit: false,
            shimmer_phase: 0.0,
            meta_tx,
            meta_rx: None,
            model_tx,
            model_rx: None,
        };

        assert!(!app.cmd_popup_visible(), "empty input hides popup");

        app.input.insert_str("hello");
        assert!(!app.cmd_popup_visible(), "non-slash input hides popup");

        app.input = TextArea::default();
        app.input.insert_str("/he");
        assert!(app.cmd_popup_visible(), "slash prefix shows popup");
        assert_eq!(app.cmd_popup_matches()[0].0, "/help");

        app.cmd_popup_dismissed = true;
        assert!(!app.cmd_popup_visible(), "Esc dismiss hides popup");

        // Any text change clears the dismiss flag (run loop does this);
        // "/hel" still fuzzy-matches help, so the popup returns.
        app.input.insert_str("l");
        app.cmd_popup_dismissed = false;
        assert!(app.cmd_popup_visible(), "editing re-opens the popup");

        // Query with no matches hides (e.g. after args / typos).
        app.input = TextArea::default();
        app.input.insert_str("/zz");
        assert!(!app.cmd_popup_visible());
    }

    #[test]
    fn popup_arrows_select_tab_completes_enter_runs() {
        let (meta_tx, _meta_rx) = unbounded_channel();
        let (model_tx, _model_rx) = unbounded_channel::<Option<String>>();
        let mut app = App {
            messages: Vec::new(),
            input: TextArea::default(),
            scroll: 0,
            follow: true,
            streaming: false,
            spinner_tick: 0,
            probing_ctx: false,
            model: Some("test-model".to_string()),
            probing_model: false,
            status: String::new(),
            config: Config {
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: String::new(),
                model: Some("test-model".to_string()),
            },
            stats: SysStats::new(),
            rx: None,
            stream_task: None,
            pending: String::new(),
            temperature_override: None,
            ctx_window: None,
            session_id: None,
            session_title: None,
            session_created: 0,
            last_saved_len: 0,
            picker: None,
            help_open: false,
            sidebar_override: None,
            sidebar_effective: true,
            cmd_popup_dismissed: false,
            cmd_popup_selected: 0,
            quit: false,
            shimmer_phase: 0.0,
            meta_tx,
            meta_rx: None,
            model_tx,
            model_rx: None,
        };

        // `/s` matches session, sessions, sidebar (declaration order).
        app.input.insert_str("/s");
        let n = app.cmd_popup_matches().len();
        assert!(n >= 3, "need several matches, got {n}");

        // ↓ / ↑ wrap around.
        app.cmd_popup_move(1);
        assert_eq!(app.cmd_popup_selected, 1);
        app.cmd_popup_move(-1);
        assert_eq!(app.cmd_popup_selected, 0);
        app.cmd_popup_move(-1);
        assert_eq!(app.cmd_popup_selected, n - 1, "wrap past top");

        // Tab completes the highlighted signature into the input.
        app.cmd_popup_selected = 0; // /session
        app.cmd_popup_complete();
        assert_eq!(app.input.lines().join("\n"), "/session");

        // Enter on /help runs it (opens overlay); input is cleared.
        app.input = TextArea::default();
        app.input.insert_str("/he");
        app.cmd_popup_selected = 0;
        app.cmd_popup_run();
        assert!(app.help_open, "Enter runs the highlighted command");
        assert!(
            app.input.lines().join("\n").is_empty(),
            "input cleared after run"
        );
        app.close_help();

        // /export <file> runs as bare `export` → usage hint, never writing
        // a literal file named `<file>`.
        app.input = TextArea::default();
        app.input.insert_str("/export");
        app.cmd_popup_selected = 0;
        assert_eq!(app.cmd_popup_matches()[0].0, "/export <file>");
        app.cmd_popup_run();
        assert_eq!(app.status, "Usage: /export <file>");

        // /quit sets the exit flag; the run loop returns on it.
        app.input = TextArea::default();
        app.input.insert_str("/qu");
        app.cmd_popup_selected = 0;
        assert_eq!(app.cmd_popup_matches()[0].0, "/quit");
        assert!(!app.quit);
        app.cmd_popup_run();
        assert!(app.quit, "/quit raises the exit flag");
    }
}

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::Backend;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tui_textarea::{Input, TextArea};

use crate::provider::{self, StreamEvent, WireMessage};

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

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl Config {
    pub fn from_env() -> Self {
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        let model =
            std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "llama3.1".to_string());
        Self {
            base_url,
            api_key,
            model,
        }
    }
}

/// Mock system stats for the sidebar. Real NVML/sysinfo wiring comes later.
#[derive(Debug, Clone)]
pub struct SysStats {
    pub vram_ratio: f64, // 0.0..1.0
    pub ram_ratio: f64,
    pub gpu_history: Vec<u64>, // 0..100
    pub ctx_used: u64,
    pub ctx_max: u64,
    tick: u64,
}

impl SysStats {
    pub fn new() -> Self {
        Self {
            vram_ratio: 0.42,
            ram_ratio: 0.55,
            gpu_history: vec![20, 25, 30, 28, 35],
            ctx_used: 0,
            ctx_max: 128_000,
            tick: 0,
        }
    }
    pub fn tick(&mut self) {
        self.tick += 1;
        let t = self.tick as f64;
        // Gentle fake waveforms so gauges/sparkline visibly move.
        self.vram_ratio = 0.45 + 0.08 * (t / 7.0).sin();
        self.ram_ratio = 0.55 + 0.05 * (t / 11.0).cos();
        let gpu = (45.0 + 30.0 * (t / 5.0).sin() + (t % 7.0)) as u64;
        self.gpu_history.push(gpu.min(100));
        if self.gpu_history.len() > 60 {
            self.gpu_history.remove(0);
        }
    }
}

pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// Keep well below u16::MAX so `area.height + scroll` in ratatui 0.29 never overflows.
pub const MAX_SCROLL: u16 = 60_000;

pub struct App<'a> {
    pub messages: Vec<ChatMessage>,
    pub input: TextArea<'a>,
    pub scroll: u16,
    pub follow: bool,
    pub streaming: bool,
    pub spinner_idx: usize,
    pub status: String,
    pub config: Config,
    pub stats: SysStats,
    rx: Option<UnboundedReceiver<StreamEvent>>,
    pub pending: String,
}

impl<'a> App<'a> {
    pub fn new(config: Config) -> Self {
        let mut input = TextArea::default();
        input.set_placeholder_text("Type a message — Enter to send, Shift+Enter newline…");
        Self {
            messages: vec![ChatMessage {
                role: Role::Agent,
                content: "Veritas ready. Set OPENAI_BASE_URL / OPENAI_MODEL and hit Enter."
                    .to_string(),
            }],
            input,
            scroll: 0,
            follow: true,
            streaming: false,
            spinner_idx: 0,
            status: "Ready — Enter send · PgUp/PgDn scroll · Ctrl-C quit".to_string(),
            config,
            stats: SysStats::new(),
            rx: None,
            pending: String::new(),
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

    fn send_current(&mut self) {
        if self.streaming {
            return;
        }
        let text = self.input.lines().join("\n").trim().to_string();
        if text.is_empty() {
            self.status = "Type something first.".to_string();
            return;
        }
        if text == "/clear" {
            self.messages.clear();
            self.input = TextArea::default();
            self.status = "Cleared.".to_string();
            return;
        }
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
        // Rough ctx estimate for the sidebar gauge.
        let chars: usize = history.iter().map(|m| m.content.len()).sum();
        self.stats.ctx_used = (chars / 4) as u64;

        let base = self.config.base_url.clone();
        let key = self.config.api_key.clone();
        let model = self.config.model.clone();
        // Fire-and-forget; results come back over `tx`. Dropping `rx` on
        // cancel/stop makes late sends no-ops.
        Handle::current().spawn(async move {
            provider::stream_chat(base, key, model, std::mem::take(&mut history), tx).await;
        });
    }

    fn cancel_stream(&mut self) {
        self.rx = None;
        self.streaming = false;
        if !self.pending.is_empty() {
            self.messages.push(ChatMessage {
                role: Role::Agent,
                content: std::mem::take(&mut self.pending),
            });
        }
        self.status = "Stopped.".to_string();
    }

    fn drain_stream(&mut self) {
        // Take the receiver out briefly so we can mutably borrow self inside.
        let mut done = false;
        let mut err: Option<String> = None;
        if let Some(rx) = self.rx.as_mut() {
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    StreamEvent::Token(t) => {
                        self.pending.push_str(&t);
                        self.stats.ctx_used += (t.len() / 4) as u64;
                    }
                    StreamEvent::Done => {
                        done = true;
                        break;
                    }
                    StreamEvent::Error(e) => {
                        err = Some(e);
                        done = true;
                        break;
                    }
                }
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
            self.status = "Ready — Enter send · PgUp/PgDn scroll · Ctrl-C quit".to_string();
        }
    }
}

pub fn run<B: Backend>(terminal: &mut Terminal<B>, config: Config) -> io::Result<()> {
    let mut app = App::new(config);
    let mut last_stats = Instant::now();
    let mut last_spinner = Instant::now();

    loop {
        terminal.draw(|f| crate::ui::draw(f, &mut app))?;

        let timeout = if app.streaming {
            Duration::from_millis(8)
        } else {
            Duration::from_millis(100)
        };

        if last_stats.elapsed() >= Duration::from_millis(500) {
            app.stats.tick();
            last_stats = Instant::now();
        }
        if app.streaming && last_spinner.elapsed() >= Duration::from_millis(80) {
            app.spinner_idx = (app.spinner_idx + 1) % SPINNER_FRAMES.len();
            last_spinner = Instant::now();
        }

        app.drain_stream();

        if !event::poll(timeout)? {
            continue;
        }
        match event::read()? {
            Event::Key(key) => {
                // Global shortcuts first.
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
                {
                    return Ok(());
                }
                match key.code {
                    KeyCode::Esc if app.streaming => {
                        app.cancel_stream();
                        continue;
                    }
                    KeyCode::PageUp => {
                        app.follow = false;
                        app.scroll = app.scroll.saturating_sub(10);
                        continue;
                    }
                    KeyCode::PageDown => {
                        app.scroll = app.scroll.saturating_add(10).min(MAX_SCROLL);
                        continue;
                    }
                    KeyCode::Enter if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.send_current();
                        continue;
                    }
                    _ => {}
                }
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('u')
                {
                    app.follow = false;
                    app.scroll = app.scroll.saturating_sub(10);
                    continue;
                }
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('d')
                {
                    app.scroll = app.scroll.saturating_add(10).min(MAX_SCROLL);
                    continue;
                }
                // Everything else goes to the textarea.
                let input = Input::from(Event::Key(key));
                app.input.input(input);
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

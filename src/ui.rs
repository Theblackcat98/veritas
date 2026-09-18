use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Sparkline, Table,
};
use ratatui::Frame;

use crate::app::{App, Role, MAX_SCROLL, SPINNER_FRAMES};
use crate::session;
use crate::theme::Theme;

/// Outer: horizontal [main | sidebar(30)].
/// Main: vertical [transcript | spinner(1) | input(6)].
/// Sidebar: vertical [header(3) | model table(8) | spacer | gauges].
pub fn draw(f: &mut Frame, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(40), Constraint::Length(32)])
        .split(f.area());

    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(6),
        ])
        .split(outer[0]);

    draw_transcript(f, app, main[0]);
    draw_spinner(f, app, main[1]);
    draw_input(f, app, main[2]);
    draw_sidebar(f, app, outer[1]);

    // Modal overlays render last, on top of everything (D013).
    if app.picker.is_some() {
        draw_picker(f, app, f.area());
    }
}

/// Centered modal session picker: `Clear` punches a hole in the buffer
/// (ROADMAP widget inventory, Phase 2), ratatui `List` for rows (no new
/// widget crate, rule 8). Keyboard-only by charter.
fn draw_picker(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    // 60% width, 60% height, centered.
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(area);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(vert[1]);

    let popup = horiz[1];
    let picker = app.picker.as_mut().expect("checked by caller");
    let now = session::now_unix();

    let items: Vec<ListItem> = if picker.entries.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No saved sessions yet — chat, then press Ctrl-S.",
            Theme::picker_dim(),
        )))]
    } else {
        picker
            .entries
            .iter()
            .map(|e| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {} ", truncate_chars(&e.title, 32)),
                        Theme::agent_text(),
                    ),
                    Span::styled(
                        format!("· {} msgs · {}", e.msg_count, session::rel_time(e.updated_at, now)),
                        Theme::picker_dim(),
                    ),
                ]))
            })
            .collect()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Theme::border())
                .title(Span::styled(
                    " Sessions — ↑/↓ select · Enter load · Esc cancel ",
                    Theme::title(),
                )),
        )
        .highlight_style(Theme::picker_selected());
    let mut state = ListState::default();
    state.select(Some(picker.selected));
    f.render_widget(Clear, popup);
    f.render_stateful_widget(list, popup, &mut state);
}

/// Char-boundary-safe truncation for picker rows.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Wrap one logical line to the transcript's inner width and push every
/// rendered row. Pre-wrapping is what makes follow-tail exact: rows.len()
/// is the true height ratatui will paint (D011).
fn push_wrapped(rows: &mut Vec<Line>, width: usize, s: &str, style: Style) {
    if s.is_empty() {
        rows.push(Line::from(""));
        return;
    }
    for frag in textwrap::fill(s, width).split('\n') {
        rows.push(Line::from(Span::styled(frag.to_string(), style)));
    }
}

fn draw_transcript(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    // Inner text area: the block border takes one column/row on each side.
    let text_width = (area.width as usize).saturating_sub(2).max(1);
    let view_h = area.height.saturating_sub(2);

    let mut rows: Vec<Line> = Vec::new();
    for msg in &app.messages {
        let (label, style) = match msg.role {
            Role::User => ("You", Theme::user_label()),
            Role::Agent => ("Agent", Theme::agent_label()),
        };
        // Short header line — never wraps, so it stays exactly one row.
        rows.push(Line::from(vec![
            Span::styled(format!("◆ {label} "), style),
            Span::styled("─".repeat(8), Theme::separator()),
        ]));
        let body_style = match msg.role {
            Role::User => Theme::user_text(),
            Role::Agent => Theme::agent_text(),
        };
        for chunk in msg.content.split('\n') {
            push_wrapped(&mut rows, text_width, chunk, body_style);
        }
        rows.push(Line::from(""));
    }
    if app.streaming {
        rows.push(Line::from(vec![
            Span::styled("◆ Agent ", Theme::agent_label()),
            Span::styled("─".repeat(8), Theme::separator()),
        ]));
        if app.pending.is_empty() {
            rows.push(Line::from(Span::styled("…", Theme::status())));
        } else {
            // Show last ~2000 chars of the in-flight response to keep render cheap.
            let tail: String = app
                .pending
                .chars()
                .rev()
                .take(2000)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            for chunk in tail.split('\n') {
                push_wrapped(&mut rows, text_width, chunk, Theme::agent_text());
            }
            rows.push(Line::from(Span::styled("▊", Theme::spinner())));
        }
        rows.push(Line::from(""));
    }

    // Follow-tail with exact row math. ratatui's Paragraph does NOT saturate
    // scroll.y — an offset past the content paints blank — so pinning to the
    // bottom must use the real row count, not a sentinel like u16::MAX (D011).
    let total = rows.len();
    let view = view_h.max(1) as usize;
    // MAX_SCROLL also keeps `area.height + scroll.y` inside u16 (D007).
    let max_scroll = total.saturating_sub(view).min(MAX_SCROLL as usize);
    let scroll = if app.follow {
        max_scroll
    } else {
        (app.scroll as usize).min(max_scroll)
    } as u16;

    let para = Paragraph::new(rows)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Theme::border())
                .title(Span::styled(" Chat ", Theme::title())),
        )
        .scroll((scroll, 0));
    f.render_widget(para, area);

    let mut state = ScrollbarState::new(max_scroll).position(scroll as usize);
    let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None);
    f.render_stateful_widget(bar, area, &mut state);
}

fn draw_spinner(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let text = if app.streaming {
        format!(
            " {} Thinking… streaming (Esc to stop)",
            SPINNER_FRAMES[app.spinner_idx]
        )
    } else {
        app.status.clone()
    };
    let style = if app.streaming {
        Theme::spinner()
    } else {
        Theme::status()
    };
    f.render_widget(Paragraph::new(Line::from(Span::styled(text, style))), area);
}

fn draw_input(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Theme::border())
        .title(Span::styled(
            " Input — Enter send, Shift+Enter newline ",
            Theme::title(),
        ));
    app.input.set_block(block);
    f.render_widget(&app.input, area);
}

fn draw_sidebar(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    // Fixed layout — every widget keeps its slot whether or not its data
    // source is available; unavailable sources render n/a in place (rule 9).
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(9), // model table
            Constraint::Min(1),    // filler
            Constraint::Length(3), // VRAM
            Constraint::Length(3), // RAM
            Constraint::Length(6), // GPU sparkline
            Constraint::Length(3), // CTX
        ])
        .split(area);

    // Header
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Veritas Agent ",
            Theme::sidebar_title(),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Theme::border()),
        ),
        rows[0],
    );

    // Model params table — live values: engine-probed ctx, /temp override or
    // engine default (D009).
    let base_short = short_base(&app.config.base_url);
    let temp_str = match app.temperature_override {
        Some(t) => t.to_string(),
        None => "engine".to_string(),
    };
    let ctx_str = match app.ctx_window {
        Some(n) => format!("{n} tok"),
        None => "n/a".to_string(),
    };
    let table = Table::new(
        vec![
            Row::new(vec!["model", app.config.model.as_str()]),
            Row::new(vec!["ctx", ctx_str.as_str()]),
            Row::new(vec!["temp", temp_str.as_str()]),
            Row::new(vec!["base", base_short.as_str()]),
        ],
        [Constraint::Length(6), Constraint::Min(8)],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::border())
            .title(Span::styled(" Model ", Theme::title())),
    );
    f.render_widget(table, rows[1]);

    // VRAM gauge — real when a GPU reports data, else n/a in the same slot.
    let vram = app.stats.vram;
    let vram_ratio = match vram {
        Some((used, total)) if total > 0 => used as f64 / total as f64,
        _ => 0.0,
    };
    let vram_title = if vram.is_some() {
        " VRAM "
    } else {
        " VRAM n/a "
    };
    f.render_widget(
        Gauge::default()
            .block(Block::default().title(vram_title).borders(Borders::ALL))
            .gauge_style(Style::default().fg(Theme::gauge_vram()))
            .ratio(vram_ratio.clamp(0.0, 1.0)),
        rows[3],
    );

    // RAM gauge — always available via sysinfo (D008).
    let ram_ratio = if app.stats.ram_total == 0 {
        0.0
    } else {
        app.stats.ram_used as f64 / app.stats.ram_total as f64
    };
    f.render_widget(
        Gauge::default()
            .block(Block::default().title(" RAM ").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Theme::gauge_ram()))
            .ratio(ram_ratio.clamp(0.0, 1.0)),
        rows[4],
    );

    // GPU sparkline — real history; blank box until data exists.
    let gpu_title = if vram.is_some() {
        " GPU % "
    } else {
        " GPU % n/a "
    };
    let spark = Sparkline::default()
        .block(Block::default().title(gpu_title).borders(Borders::ALL))
        .data(&app.stats.gpu_history)
        .max(100);
    f.render_widget(spark, rows[5]);

    // CTX usage — ratio gauge when the window is probed; otherwise an
    // empty gauge with an n/a label, never a fake ratio (D009 / rule 9).
    let ctx_ratio = match app.ctx_window {
        Some(max) if max > 0 => (app.stats.ctx_used as f64 / max as f64).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let ctx_title = match app.ctx_window {
        Some(max) => format!(" CTX {}/{} ", app.stats.ctx_used, max),
        None => format!(" CTX {} tok · n/a ", app.stats.ctx_used),
    };
    f.render_widget(
        Gauge::default()
            .block(Block::default().title(ctx_title).borders(Borders::ALL))
            .gauge_style(Style::default().fg(Theme::gauge_ctx(ctx_ratio)))
            .ratio(ctx_ratio),
        rows[6],
    );
}

fn short_base(base: &str) -> String {
    let s = base
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    if s.len() > 18 {
        format!("…{}", &s[s.len() - 17..])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;
    use crate::app::{ChatMessage, Config};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn test_app() -> App<'static> {
        // Port 9 refuses instantly; the spawned ctx probe never matters here.
        App::new(Config {
            base_url: "http://127.0.0.1:9/v1".to_string(),
            api_key: String::new(),
            model: "test".to_string(),
        })
    }

    fn render(app: &mut App<'_>, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|f| draw(f, app)).expect("draw");
        let buf = terminal.backend().buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn long_transcript(app: &mut App<'_>) {
        for i in 1..=40 {
            app.messages.push(ChatMessage {
                role: if i % 2 == 0 { Role::User } else { Role::Agent },
                content: format!("message {i:02} with some padding so lines exist"),
            });
        }
    }

    #[tokio::test]
    async fn follow_pins_tail_to_bottom() {
        let mut app = test_app();
        long_transcript(&mut app);
        app.follow = true;
        let text = render(&mut app, 60, 20);
        assert!(
            text.contains("message 40"),
            "last message must be visible in follow mode:\n{text}"
        );
        assert!(
            !text.contains("message 01 "),
            "top must be scrolled away in follow mode:\n{text}"
        );
    }

    #[tokio::test]
    async fn streaming_keeps_tail_visible() {
        let mut app = test_app();
        long_transcript(&mut app);
        app.streaming = true;
        app.pending = "partial answer ".repeat(40);
        app.follow = true;
        let text = render(&mut app, 60, 20);
        assert!(
            text.contains("partial answer"),
            "streaming tail must stay visible:\n{text}"
        );
    }

    #[tokio::test]
    async fn scroll_top_shows_first_message() {
        let mut app = test_app();
        long_transcript(&mut app);
        app.follow = false;
        app.scroll = 0;
        let text = render(&mut app, 60, 20);
        assert!(
            text.contains("message 01"),
            "scroll 0 must show the first message:\n{text}"
        );
    }

    #[tokio::test]
    async fn no_seeded_greeting_in_history() {
        let app = test_app();
        assert!(app.messages.is_empty(), "history must start empty");
    }

    fn entry(id: &str, title: &str, msgs: usize) -> crate::session::SessionEntry {
        crate::session::SessionEntry {
            id: id.to_string(),
            title: title.to_string(),
            updated_at: 100,
            msg_count: msgs,
        }
    }

    #[tokio::test]
    async fn picker_renders_entries_and_hides_background() {
        let mut app = test_app();
        long_transcript(&mut app);
        app.picker = Some(crate::app::Picker {
            entries: vec![
                entry("sess-1", "First chat", 4),
                entry("sess-2", "Second chat", 9),
            ],
            selected: 1,
        });
        // 60x30: popup covers rows 6..24, cols 12..48 — the sidebar gauges
        // (VRAM/RAM/CTX titles at x≈29) must be punched out by Clear.
        let text = render(&mut app, 60, 30);
        assert!(text.contains("Sessions —"), "picker title visible:\n{text}");
        assert!(text.contains("First chat"), "entries listed:\n{text}");
        assert!(text.contains("Second chat"), "entries listed:\n{text}");
        assert!(text.contains("9 msgs"), "metadata shown:\n{text}");
        assert!(
            !text.contains("VRAM"),
            "overlay must hide widgets behind it:\n{text}"
        );
    }

    #[tokio::test]
    async fn picker_empty_state_hint() {
        let mut app = test_app();
        app.picker = Some(crate::app::Picker {
            entries: Vec::new(),
            selected: 0,
        });
        let text = render(&mut app, 60, 30);
        assert!(
            text.contains("No saved sessions"),
            "empty state must explain itself:\n{text}"
        );
    }

    #[tokio::test]
    async fn picker_keys_move_with_wrap_and_esc_closes() {
        let mut app = test_app();
        app.picker = Some(crate::app::Picker {
            entries: vec![entry("a", "A", 1), entry("b", "B", 2), entry("c", "C", 3)],
            selected: 0,
        });
        app.handle_picker_key(KeyCode::Down);
        assert_eq!(app.picker.as_ref().expect("open").selected, 1);
        app.handle_picker_key(KeyCode::Up);
        app.handle_picker_key(KeyCode::Up);
        assert_eq!(
            app.picker.as_ref().expect("open").selected,
            2,
            "wrap past top lands on last entry"
        );
        app.handle_picker_key(KeyCode::PageDown);
        assert_eq!(
            app.picker.as_ref().expect("open").selected,
            0,
            "PageDown wraps: (2+10) mod 3 = 0"
        );
        app.handle_picker_key(KeyCode::Esc);
        assert!(app.picker.is_none(), "Esc closes the picker");
    }
}

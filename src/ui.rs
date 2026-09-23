use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Sparkline, Table,
};
use ratatui::Frame;
use tui_spinner::{BarMotion, BarSpinner, BarStyle, FluxFrames, FluxSpinner};

use crate::app::{App, Role, MAX_SCROLL};
use crate::md_style;
use crate::session;
use crate::theme::Theme;

/// Outer: horizontal [main | sidebar(30)].
/// Main: vertical [transcript | spinner(1) | input(6)].
/// Sidebar: vertical [header(3) | model table(8) | spacer | gauges].
pub fn draw(f: &mut Frame, app: &mut App) {
    f.render_widget(Block::default().style(Theme::canvas()), f.area());

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
    if app.help_open {
        draw_help(f, app, f.area());
    }
}

/// Shared centered modal frame for keyboard-first overlays.
fn modal_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Theme::border_focus())
        .style(Theme::modal_surface())
        .title(Span::styled(format!(" {title} "), Theme::title()))
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
        vec![ListItem::new(Line::from(vec![
            Span::styled("No saved sessions yet. ", Theme::empty_state()),
            Span::styled("Chat, then press Ctrl-S.", Theme::hint()),
        ]))]
    } else {
        picker
            .entries
            .iter()
            .map(|e| {
                ListItem::new(Line::from(vec![
                    Span::styled(truncate_chars(&e.title, 32), Theme::text()),
                    Span::styled(
                        format!(
                            "  · {} msgs · {}",
                            e.msg_count,
                            session::rel_time(e.updated_at, now)
                        ),
                        Theme::faint(),
                    ),
                ]))
            })
            .collect()
    };

    let modal = modal_block("Sessions");
    let inner = modal.inner(popup);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    f.render_widget(Clear, popup);
    f.render_widget(modal, popup);

    let list = List::new(items)
        .style(Theme::modal_surface())
        .highlight_style(Theme::selected_row())
        .highlight_symbol("› ");
    let mut state = ListState::default();
    if !picker.entries.is_empty() {
        state.select(Some(picker.selected));
    }
    f.render_stateful_widget(list, sections[0], &mut state);
    f.render_widget(
        Paragraph::new("↑↓ navigate · PgUp/PgDn jump · Enter load · Esc close")
            .style(Theme::hint())
            .alignment(Alignment::Right),
        sections[1],
    );
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

fn key_row(key: &str, desc: &str) -> Line<'static> {
    let pad = 16usize.saturating_sub(key.len()).max(2);
    Line::from(vec![
        Span::styled(key.to_string(), Theme::key_hint()),
        Span::styled(" ".repeat(pad), Theme::muted()),
        Span::styled(desc.to_string(), Theme::muted()),
    ])
}

fn cmd_row(cmd: &str, desc: &str) -> Line<'static> {
    let pad = 16usize.saturating_sub(cmd.len()).max(2);
    Line::from(vec![
        Span::styled(cmd.to_string(), Theme::command_hint()),
        Span::styled(" ".repeat(pad), Theme::muted()),
        Span::styled(desc.to_string(), Theme::muted()),
    ])
}

/// Centered help overlay showing all keybindings (like the session picker).
fn draw_help(f: &mut Frame, _app: &App, area: ratatui::layout::Rect) {
    let popup_h = 22.min(area.height);
    let top_pad = area.height.saturating_sub(popup_h) / 2;
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_pad),
            Constraint::Length(popup_h),
            Constraint::Min(0),
        ])
        .split(area);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(vert[1]);

    let popup = horiz[1];
    let help_text = vec![
        Line::from(Span::styled("KEYBOARD", Theme::section_title())),
        key_row("Enter", "Send message"),
        key_row("Shift+Enter", "Newline in input"),
        key_row("Esc", "Cancel stream / close overlay"),
        key_row("Ctrl-C / Ctrl-Q", "Quit"),
        key_row("PgUp / PgDn", "Scroll transcript"),
        key_row("Ctrl-U / Ctrl-D", "Scroll up / down"),
        key_row("Ctrl-Y", "Copy latest code block"),
        key_row("Ctrl-S", "Open session picker"),
        Line::from(""),
        Line::from(Span::styled("COMMANDS", Theme::section_title())),
        cmd_row("/help", "Show this help"),
        cmd_row("/clear", "Clear transcript"),
        cmd_row("/copy [n]", "Copy code block to clipboard"),
        cmd_row("/sessions", "Open session picker"),
        cmd_row("/export <file>", "Export transcript as markdown"),
        cmd_row("/model <id>", "Set model"),
        cmd_row("/temp <0.0-2.0>", "Set temperature (overrides engine)"),
    ];

    let modal = modal_block("Help");
    let inner = modal.inner(popup);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    f.render_widget(Clear, popup);
    f.render_widget(modal, popup);
    f.render_widget(
        Paragraph::new(help_text).style(Theme::modal_surface()),
        sections[0],
    );
    f.render_widget(
        Paragraph::new("Esc close")
            .style(Theme::hint())
            .alignment(Alignment::Right),
        sections[1],
    );
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
        match msg.role {
            Role::User => {
                for chunk in msg.content.split('\n') {
                    push_wrapped(&mut rows, text_width, chunk, Theme::user_text());
                }
            }
            // Agent messages are markdown (D012): styled structure beats a
            // flat block. md_style pre-wraps to text_width, so follow-tail
            // row math is untouched (D011).
            Role::Agent => {
                rows.extend(md_style::render_wrapped(&msg.content, text_width));
            }
        }
        rows.push(Line::from(""));
    }
    if app.streaming {
        rows.push(Line::from(vec![
            Span::styled("◆ Agent ", Theme::agent_label()),
            Span::styled("─".repeat(8), Theme::separator()),
        ]));
        if app.pending.is_empty() {
            // Pre-token thinking phase (TTFT):
            // Render a horizontal BarSpinner indicating active reasoning/generation.
            let bar_width = 16.min(text_width.saturating_sub(12).max(4));
            let bar = BarSpinner::new(app.spinner_tick)
                .width(bar_width)
                .bar_style(BarStyle::Braille)
                .motion(BarMotion::Bounce)
                .arc_color(Theme::activity_color())
                .dim_color(Theme::panel_color());
            let bar_lines = bar.to_lines(bar_width, 1);
            if let Some(first) = bar_lines.into_iter().next() {
                let mut spans = vec![Span::styled("Thinking  ", Theme::spinner())];
                spans.extend(first.spans);
                rows.push(Line::from(spans));
            } else {
                rows.push(Line::from(Span::styled("Thinking…", Theme::spinner())));
            }
        } else {
            for chunk in app.pending.split('\n') {
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

    // Clamp scroll to actual content height to prevent scrolling past bottom.
    // When manual scrolling down reaches or passes the tail, re-engage follow mode.
    if !app.follow {
        if (app.scroll as usize) >= max_scroll {
            app.follow = true;
        } else {
            app.scroll = (app.scroll as usize).min(max_scroll) as u16;
        }
    }

    let scroll = if app.follow {
        // Keep the stored offset in sync with the visible tail. This matters
        // when the first manual action is wheel-up: it must move up from the
        // current bottom, not subtract from a stale zero offset.
        app.scroll = max_scroll as u16;
        app.scroll
    } else {
        app.scroll
    };

    let para = Paragraph::new(rows)
        .style(Theme::panel())
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
    if app.streaming {
        // Shimmer sweep driven from the spinner tick (D017): phase advances
        // in app::run so the animation cadence matches the rest of the UI.
        // Status line pairs a FluxSpinner glyph with the shimmering label (D029).
        let parts = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(1), // leading margin
                Constraint::Length(1), // FluxSpinner glyph
                Constraint::Min(1),    // shimmering label
            ])
            .split(area);

        let spinner = FluxSpinner::new(app.spinner_tick)
            .frames(FluxFrames::BRAILLE)
            .color(Theme::activity_color());
        f.render_widget(spinner, parts[1]);

        let label = " Thinking… streaming (Esc to stop)";
        let spans = tui_shimmer::shimmer_spans_with_style_at_phase(
            label,
            Theme::spinner(),
            app.shimmer_phase,
        );
        f.render_widget(Paragraph::new(Line::from(spans)), parts[2]);
        return;
    }
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            app.status.clone(),
            Theme::status(),
        ))),
        area,
    );
}

fn draw_input(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Theme::border_focus())
        .style(Theme::panel())
        .title(Span::styled(" Input ", Theme::title()));
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
            Constraint::Length(6), // model table (4 rows + 2 borders = 6)
            Constraint::Min(0),    // filler
            Constraint::Length(3), // VRAM
            Constraint::Length(3), // RAM
            Constraint::Length(6), // GPU sparkline
            Constraint::Length(3), // CTX
        ])
        .split(area);

    // Header
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(" Veritas Agent ", Theme::title()))).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Theme::border())
                .style(Theme::panel()),
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
    let ctx_cell = match app.ctx_window {
        Some(n) => Cell::from(format!("{n} tok")),
        None if app.probing_ctx => {
            let spinner_line = FluxSpinner::new(app.spinner_tick)
                .frames(FluxFrames::BRAILLE)
                .color(Theme::activity_color())
                .to_lines()
                .into_iter()
                .next()
                .unwrap_or_else(|| Line::from("…"));
            let mut spans = spinner_line.spans;
            spans.push(Span::styled(" probing…", Theme::muted()));
            Cell::from(Line::from(spans))
        }
        None => Cell::from("n/a"),
    };
    let table = Table::new(
        vec![
            Row::new(vec![Cell::from("model"), Cell::from(app.config.model.as_str())]),
            Row::new(vec![Cell::from("ctx"), ctx_cell]),
            Row::new(vec![Cell::from("temp"), Cell::from(temp_str.as_str())]),
            Row::new(vec![Cell::from("base"), Cell::from(base_short.as_str())]),
        ],
        [Constraint::Length(6), Constraint::Min(8)],
    )
    .style(Theme::telemetry_value())
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::border())
            .style(Theme::panel())
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
            .block(
                Block::default()
                    .title(vram_title)
                    .borders(Borders::ALL)
                    .border_style(Theme::border())
                    .style(Theme::panel()),
            )
            .gauge_style(Theme::gauge_vram())
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
            .block(
                Block::default()
                    .title(" RAM ")
                    .borders(Borders::ALL)
                    .border_style(Theme::border())
                    .style(Theme::panel()),
            )
            .gauge_style(Theme::gauge_ram())
            .ratio(ram_ratio.clamp(0.0, 1.0)),
        rows[4],
    );

    // GPU sparkline — real history; blank box until data exists.
    // Ratatui's Sparkline takes data.iter().take(width), so we pass the most
    // recent window of samples so the right edge is always real-time.
    let gpu_title = if vram.is_some() {
        " GPU % "
    } else {
        " GPU % n/a "
    };
    let inner_width = (rows[5].width as usize).saturating_sub(2);
    let history_slice = if app.stats.gpu_history.len() > inner_width {
        &app.stats.gpu_history[app.stats.gpu_history.len() - inner_width..]
    } else {
        &app.stats.gpu_history[..]
    };
    let spark = Sparkline::default()
        .block(
            Block::default()
                .title(gpu_title)
                .borders(Borders::ALL)
                .border_style(Theme::border())
                .style(Theme::panel()),
        )
        .style(Theme::gauge_gpu())
        .data(history_slice)
        .max(100);
    f.render_widget(spark, rows[5]);

    // CTX usage — ratio gauge when the window is probed; otherwise an
    // empty gauge with an n/a label, never a fake ratio (D009 / rule 9).
    let ctx_ratio = match app.ctx_window {
        Some(max) if max > 0 => (app.stats.ctx_used as f64 / max as f64).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let ctx_title = match app.ctx_window {
        Some(max) => Line::from(format!(" CTX {}/{} ", app.stats.ctx_used, max)),
        None => Line::from(vec![
            Span::raw(format!(" CTX {} tok · ", app.stats.ctx_used)),
            Span::styled("n/a", Theme::telemetry_unavailable()),
            Span::raw(" "),
        ]),
    };
    f.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .title(ctx_title)
                    .borders(Borders::ALL)
                    .border_style(Theme::border())
                    .style(Theme::panel()),
            )
            .gauge_style(Theme::gauge_ctx(ctx_ratio))
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
    use crate::app::{ChatMessage, Config};
    use crossterm::event::KeyCode;
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
    async fn first_manual_scroll_starts_at_follow_tail() {
        let mut app = test_app();
        long_transcript(&mut app);
        app.follow = true;
        render(&mut app, 60, 20);
        let tail = app.scroll;
        assert!(tail > 3, "test transcript must have room to scroll");

        app.follow = false;
        app.scroll = app.scroll.saturating_sub(3);
        let text = render(&mut app, 60, 20);

        assert_eq!(app.scroll, tail - 3);
        assert!(
            !text.contains("message 01 "),
            "one wheel tick must not jump to the top:\n{text}"
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
        assert!(text.contains("Sessions"), "picker title visible:\n{text}");
        assert!(text.contains("First chat"), "entries listed:\n{text}");
        assert!(text.contains("Second chat"), "entries listed:\n{text}");
        assert!(text.contains("9 msgs"), "metadata shown:\n{text}");
        assert!(
            !text.contains("VRAM"),
            "overlay must hide widgets behind it:\n{text}"
        );
    }

    #[tokio::test]
    async fn help_renders_modal_hierarchy_and_footer() {
        let mut app = test_app();
        app.help_open = true;
        let text = render(&mut app, 60, 30);
        assert!(
            text.contains("Help"),
            "help title visible:
{text}"
        );
        assert!(
            text.contains("KEYBOARD"),
            "keyboard section visible:
{text}"
        );
        assert!(
            text.contains("COMMANDS"),
            "commands section visible:
{text}"
        );
        assert!(
            text.contains("/temp <0.0-2.0>"),
            "last command visible:
{text}"
        );
        assert!(
            text.contains("Esc close"),
            "modal footer visible:
{text}"
        );
        assert!(
            !text.contains("VRAM"),
            "overlay must hide widgets behind it:
{text}"
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

    #[tokio::test]
    async fn standard_80x24_layout_renders_all_widgets_and_help() {
        let mut app = test_app();
        // Set mock context window so CTX ratio title is tested
        app.ctx_window = Some(8192);
        app.stats.ctx_used = 1024;
        let text = render(&mut app, 80, 24);
        assert!(text.contains("Veritas Agent"), "header visible in 80x24:\n{text}");
        assert!(text.contains("Model"), "model table visible in 80x24:\n{text}");
        assert!(text.contains("RAM"), "RAM gauge visible in 80x24:\n{text}");
        assert!(text.contains("CTX"), "CTX gauge visible in 80x24:\n{text}");

        // Now test help overlay in 80x24
        app.help_open = true;
        let help_view = render(&mut app, 80, 24);
        assert!(help_view.contains("KEYBOARD"), "keyboard section visible in 80x24:\n{help_view}");
        assert!(help_view.contains("COMMANDS"), "commands section visible in 80x24:\n{help_view}");
        assert!(help_view.contains("/temp <0.0-2.0>"), "last command visible in 80x24:\n{help_view}");
        assert!(help_view.contains("Esc close"), "footer visible in 80x24:\n{help_view}");
    }

    #[tokio::test]
    async fn thinking_state_renders_bar_spinner_before_tokens() {
        let mut app = test_app();
        app.streaming = true;
        app.pending.clear();
        let text = render(&mut app, 80, 24);
        assert!(
            text.contains("Thinking"),
            "transcript must show thinking bar before first token:\n{text}"
        );
        assert!(
            text.contains("streaming (Esc to stop)"),
            "status line must show streaming hint:\n{text}"
        );
    }

    #[tokio::test]
    async fn streaming_tokens_replaces_thinking_bar_with_stream_and_cursor() {
        let mut app = test_app();
        app.streaming = true;
        app.pending = "Streaming token response".to_string();
        let text = render(&mut app, 80, 24);
        assert!(
            text.contains("Streaming token response"),
            "streamed text must appear in transcript:\n{text}"
        );
        assert!(
            text.contains('▊'),
            "trailing cursor block must appear after stream:\n{text}"
        );
        assert!(
            !text.contains("Thinking  "),
            "thinking bar must be removed once tokens arrive:\n{text}"
        );
    }

    #[tokio::test]
    async fn model_table_renders_probing_spinner_while_probing() {
        let mut app = test_app();
        app.ctx_window = None;
        app.probing_ctx = true;
        let text = render(&mut app, 80, 24);
        assert!(
            text.contains("probing…"),
            "model table must show probing indicator:\n{text}"
        );

        app.probing_ctx = false;
        let text_resolved = render(&mut app, 80, 24);
        assert!(
            text_resolved.contains("n/a"),
            "model table must show n/a when not probing and ctx unknown:\n{text_resolved}"
        );
    }
}

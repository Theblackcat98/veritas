use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Gauge, Paragraph, Row, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Sparkline, Table, Wrap,
};

use crate::app::{Role, SPINNER_FRAMES, App};
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
}

fn draw_transcript(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();
    for msg in &app.messages {
        let (label, style) = match msg.role {
            Role::User => ("You", Theme::user_label()),
            Role::Agent => ("Agent", Theme::agent_label()),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("◆ {label} ",), style),
            Span::styled("─".repeat(8), Theme::separator()),
        ]));
        let body_style = match msg.role {
            Role::User => Theme::user_text(),
            Role::Agent => Theme::agent_text(),
        };
        for chunk in msg.content.split('\n') {
            // Wrap is handled by Paragraph, but keep explicit lines for blank lines.
            if chunk.is_empty() {
                lines.push(Line::from(""));
            } else {
                lines.push(Line::from(Span::styled(chunk.to_string(), body_style)));
            }
        }
        lines.push(Line::from(""));
    }
    if app.streaming {
        lines.push(Line::from(vec![
            Span::styled("◆ Agent ", Theme::agent_label()),
            Span::styled("─".repeat(8), Theme::separator()),
        ]));
        if app.pending.is_empty() {
            lines.push(Line::from(Span::styled("…", Theme::status())));
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
                lines.push(Line::from(Span::styled(
                    chunk.to_string(),
                    Theme::agent_text(),
                )));
            }
            lines.push(Line::from(Span::styled("▊", Theme::spinner())));
        }
        lines.push(Line::from(""));
    }

    // NB: ratatui 0.29 Paragraph does `area.height + scroll.y` in u16,
    // so u16::MAX overflows. Clamp to a safe max that still follows to bottom.
    let max_safe = u16::MAX.saturating_sub(area.height).saturating_sub(1);
    let raw = if app.follow { u16::MAX } else { app.scroll };
    let scroll = raw.min(max_safe);
    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Theme::border())
                .title(Span::styled(" Chat ", Theme::title())),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(para, area);

    let mut state = ScrollbarState::new(app.messages.len()).position(scroll as usize);
    let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None);
    f.render_stateful_widget(bar, area, &mut state);
}

fn draw_spinner(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let text = if app.streaming {
        format!(" {} Thinking… streaming (Esc to stop)", SPINNER_FRAMES[app.spinner_idx])
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
        .title(Span::styled(" Input — Enter send, Shift+Enter newline ", Theme::title()));
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
        Paragraph::new(Line::from(Span::styled(" Veritas Agent ", Theme::sidebar_title())))
            .block(Block::default().borders(Borders::ALL).border_style(Theme::border())),
        rows[0],
    );

    // Model params table — live values: engine-probed ctx, /temp override or
    // engine default (D013).
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
    // empty gauge with an n/a label, never a fake ratio (D013 / rule 9).
    let ctx_ratio = match app.ctx_window {
        Some(max) if max > 0 => {
            (app.stats.ctx_used as f64 / max as f64).clamp(0.0, 1.0)
        }
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
    let s = base.trim_start_matches("http://").trim_start_matches("https://");
    if s.len() > 18 {
        format!("…{}", &s[s.len() - 17..])
    } else {
        s.to_string()
    }
}

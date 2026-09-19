use ratatui::style::{Color, Modifier, Style};

/// Single place to reskin the app. The UI consumes semantic roles from this
/// type instead of choosing colors for individual widgets.
pub struct Theme;

impl Theme {
    // Surfaces
    pub fn canvas() -> Style {
        Style::default().bg(Color::Rgb(8, 12, 16))
    }

    pub fn panel() -> Style {
        Style::default().bg(Color::Rgb(15, 23, 29))
    }

    pub fn modal_surface() -> Style {
        Style::default().bg(Color::Rgb(17, 29, 36))
    }

    pub fn code_surface() -> Style {
        Style::default().bg(Color::Rgb(22, 33, 42))
    }

    // Chrome
    pub fn border() -> Style {
        Style::default().fg(Color::Rgb(51, 67, 78))
    }

    pub fn border_focus() -> Style {
        Style::default().fg(Color::Rgb(34, 211, 238))
    }

    pub fn title() -> Style {
        Style::default()
            .fg(Color::Rgb(34, 211, 238))
            .add_modifier(Modifier::BOLD)
    }

    pub fn section_title() -> Style {
        Style::default()
            .fg(Color::Rgb(45, 212, 191))
            .add_modifier(Modifier::BOLD)
    }

    pub fn separator() -> Style {
        Style::default().fg(Color::Rgb(100, 116, 139))
    }

    // Text
    pub fn text() -> Style {
        Style::default().fg(Color::Rgb(230, 237, 243))
    }

    pub fn muted() -> Style {
        Style::default().fg(Color::Rgb(148, 163, 184))
    }

    pub fn faint() -> Style {
        Style::default().fg(Color::Rgb(100, 116, 139))
    }

    pub fn hint() -> Style {
        Self::faint()
    }

    // Interaction
    pub fn key_hint() -> Style {
        Style::default()
            .fg(Color::Rgb(8, 12, 16))
            .bg(Color::Rgb(34, 211, 238))
            .add_modifier(Modifier::BOLD)
    }

    pub fn command_hint() -> Style {
        Style::default()
            .fg(Color::Rgb(125, 211, 252))
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected_row() -> Style {
        Style::default()
            .fg(Color::Rgb(8, 12, 16))
            .bg(Color::Rgb(34, 211, 238))
            .add_modifier(Modifier::BOLD)
    }

    pub fn empty_state() -> Style {
        Self::muted()
    }

    // Conversation
    pub fn user_label() -> Style {
        Style::default()
            .fg(Color::Rgb(125, 211, 252))
            .add_modifier(Modifier::BOLD)
    }

    pub fn agent_label() -> Style {
        Style::default()
            .fg(Color::Rgb(167, 243, 208))
            .add_modifier(Modifier::BOLD)
    }

    pub fn user_text() -> Style {
        Self::text()
    }

    pub fn agent_text() -> Style {
        Style::default().fg(Color::Rgb(203, 213, 225))
    }

    // State
    pub fn status() -> Style {
        Self::faint()
    }

    pub fn activity() -> Style {
        Style::default()
            .fg(Color::Rgb(251, 191, 36))
            .add_modifier(Modifier::BOLD)
    }

    pub fn spinner() -> Style {
        Self::activity()
    }

    pub fn success() -> Style {
        Style::default().fg(Color::Rgb(52, 211, 153))
    }

    pub fn warning() -> Style {
        Style::default().fg(Color::Rgb(245, 158, 11))
    }

    pub fn error() -> Style {
        Style::default().fg(Color::Rgb(248, 113, 113))
    }

    // Telemetry
    pub fn gauge_vram() -> Style {
        Style::default().fg(Color::Rgb(45, 212, 191))
    }

    pub fn gauge_ram() -> Style {
        Style::default().fg(Color::Rgb(125, 211, 252))
    }

    pub fn gauge_gpu() -> Style {
        Style::default().fg(Color::Rgb(45, 212, 191))
    }

    /// CTX gauge shifts success → warning → error as the window fills.
    pub fn gauge_ctx(ratio: f64) -> Style {
        if ratio >= 0.9 {
            Self::error()
        } else if ratio >= 0.7 {
            Self::warning()
        } else {
            Self::success()
        }
    }

    pub fn telemetry_value() -> Style {
        Self::text()
    }

    pub fn telemetry_unavailable() -> Style {
        Self::faint()
    }

    // Markdown
    pub fn md_code() -> Style {
        Self::code_surface().fg(Color::Rgb(125, 211, 252))
    }

    pub fn md_link() -> Style {
        Style::default()
            .fg(Color::Rgb(125, 211, 252))
            .add_modifier(Modifier::UNDERLINED)
    }

    pub fn md_blockquote() -> Style {
        Style::default()
            .fg(Color::Rgb(167, 243, 208))
            .add_modifier(Modifier::ITALIC)
    }

    pub fn md_h1() -> Style {
        Style::default()
            .fg(Color::Rgb(34, 211, 238))
            .add_modifier(Modifier::BOLD)
    }

    pub fn md_h2() -> Style {
        Style::default()
            .fg(Color::Rgb(45, 212, 191))
            .add_modifier(Modifier::BOLD)
    }

    pub fn md_h3() -> Style {
        Style::default()
            .fg(Color::Rgb(251, 191, 36))
            .add_modifier(Modifier::ITALIC)
            .add_modifier(Modifier::BOLD)
    }

    pub fn md_bold() -> Style {
        Style::default()
            .fg(Color::Rgb(230, 237, 243))
            .add_modifier(Modifier::BOLD)
    }

    pub fn md_italic() -> Style {
        Style::default()
            .fg(Color::Rgb(148, 163, 184))
            .add_modifier(Modifier::ITALIC)
    }

    pub fn md_hr() -> Style {
        Self::faint()
    }
}

use ratatui::style::{Color, Modifier, Style};

/// Single place to reskin the app. Mockup colors were structural only,
/// so everything here is a neutral default. Tweak freely.
pub struct Theme;

impl Theme {
    pub fn border() -> Style {
        Style::default().fg(Color::DarkGray)
    }
    pub fn title() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }
    pub fn user_label() -> Style {
        Style::default()
            .fg(Color::LightBlue)
            .add_modifier(Modifier::BOLD)
    }
    pub fn agent_label() -> Style {
        Style::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD)
    }
    pub fn user_text() -> Style {
        Style::default().fg(Color::White)
    }
    pub fn agent_text() -> Style {
        Style::default().fg(Color::Gray)
    }
    pub fn status() -> Style {
        Style::default().fg(Color::DarkGray)
    }
    pub fn separator() -> Style {
        Style::default().fg(Color::DarkGray)
    }
    pub fn spinner() -> Style {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }
    pub fn sidebar_title() -> Style {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    }
    pub fn gauge_vram() -> Color {
        Color::Yellow
    }
    pub fn gauge_ram() -> Color {
        Color::Blue
    }
    /// Picker selection highlight (session list rows).
    pub fn picker_selected() -> Style {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }
    /// Dimmed metadata text in the picker (ages, counts, hints).
    pub fn picker_dim() -> Style {
        Style::default().fg(Color::DarkGray)
    }
    /// CTX gauge shifts green → yellow → red as the window fills.
    pub fn gauge_ctx(ratio: f64) -> Color {
        if ratio >= 0.9 {
            Color::Red
        } else if ratio >= 0.7 {
            Color::Yellow
        } else {
            Color::Green
        }
    }
    /// Inline `code` spans and fenced blocks in agent messages (D015).
    pub fn md_code() -> Style {
        Style::default().fg(Color::LightCyan)
    }
    /// Link labels in agent messages (D015).
    pub fn md_link() -> Style {
        Style::default()
            .fg(Color::LightBlue)
            .add_modifier(Modifier::UNDERLINED)
    }
    /// Blockquoted lines in agent messages (D015).
    pub fn md_blockquote() -> Style {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::ITALIC)
    }
    /// Heading level 1 style
    pub fn md_h1() -> Style {
        Style::default()
            .fg(Color::LightMagenta)
            .add_modifier(Modifier::BOLD)
    }
    /// Heading level 2 style
    pub fn md_h2() -> Style {
        Style::default()
            .fg(Color::LightCyan)
            .add_modifier(Modifier::BOLD)
    }
    /// Heading level 3 style
    pub fn md_h3() -> Style {
        Style::default()
            .fg(Color::LightYellow)
            .add_modifier(Modifier::ITALIC)
    }
    /// Bold text with color
    pub fn md_bold() -> Style {
        Style::default()
            .fg(Color::LightRed)
            .add_modifier(Modifier::BOLD)
    }
    /// Italic text with color
    pub fn md_italic() -> Style {
        Style::default()
            .fg(Color::LightBlue)
            .add_modifier(Modifier::ITALIC)
    }
    /// Horizontal rule style
    pub fn md_hr() -> Style {
        Style::default().fg(Color::DarkGray)
    }
}

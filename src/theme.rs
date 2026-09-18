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
}

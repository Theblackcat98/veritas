use std::sync::OnceLock;
use ratatui::style::{Color, Modifier, Style};

/// Theme presets selectable via the `VERITAS_THEME` environment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Default,    // Quiet Mission Control (cyan / teal / mint)
    Amber,      // Amber CRT Console (amber / gold)
    Green,      // Phosphor Green CRT (matrix / green)
    Monochrome, // High-contrast grayscale
}

static CURRENT_PRESET: OnceLock<Preset> = OnceLock::new();

pub fn active_preset() -> Preset {
    *CURRENT_PRESET.get_or_init(|| {
        match std::env::var("VERITAS_THEME")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "amber" => Preset::Amber,
            "green" | "matrix" | "phosphor" => Preset::Green,
            "monochrome" | "mono" | "gray" => Preset::Monochrome,
            _ => Preset::Default,
        }
    })
}

struct Palette {
    canvas: Color,
    panel: Color,
    modal_surface: Color,
    code_surface: Color,
    border: Color,
    border_focus: Color,
    title: Color,
    section_title: Color,
    separator: Color,
    text: Color,
    muted: Color,
    faint: Color,
    user: Color,
    agent: Color,
    activity: Color,
    success: Color,
    warning: Color,
    error: Color,
}

const DEFAULT_PALETTE: Palette = Palette {
    canvas: Color::Rgb(8, 12, 16),
    panel: Color::Rgb(15, 23, 29),
    modal_surface: Color::Rgb(17, 29, 36),
    code_surface: Color::Rgb(22, 33, 42),
    border: Color::Rgb(51, 67, 78),
    border_focus: Color::Rgb(34, 211, 238),
    title: Color::Rgb(34, 211, 238),
    section_title: Color::Rgb(45, 212, 191),
    separator: Color::Rgb(100, 116, 139),
    text: Color::Rgb(230, 237, 243),
    muted: Color::Rgb(148, 163, 184),
    faint: Color::Rgb(100, 116, 139),
    user: Color::Rgb(125, 211, 252),
    agent: Color::Rgb(167, 243, 208),
    activity: Color::Rgb(251, 191, 36),
    success: Color::Rgb(52, 211, 153),
    warning: Color::Rgb(245, 158, 11),
    error: Color::Rgb(248, 113, 113),
};

const AMBER_PALETTE: Palette = Palette {
    canvas: Color::Rgb(16, 12, 6),
    panel: Color::Rgb(26, 18, 10),
    modal_surface: Color::Rgb(32, 22, 12),
    code_surface: Color::Rgb(38, 26, 14),
    border: Color::Rgb(90, 65, 35),
    border_focus: Color::Rgb(251, 191, 36),
    title: Color::Rgb(251, 191, 36),
    section_title: Color::Rgb(245, 158, 11),
    separator: Color::Rgb(120, 85, 45),
    text: Color::Rgb(254, 243, 199),
    muted: Color::Rgb(217, 119, 6),
    faint: Color::Rgb(146, 64, 14),
    user: Color::Rgb(253, 230, 138),
    agent: Color::Rgb(245, 158, 11),
    activity: Color::Rgb(251, 191, 36),
    success: Color::Rgb(52, 211, 153),
    warning: Color::Rgb(245, 158, 11),
    error: Color::Rgb(239, 68, 68),
};

const GREEN_PALETTE: Palette = Palette {
    canvas: Color::Rgb(6, 16, 10),
    panel: Color::Rgb(10, 26, 16),
    modal_surface: Color::Rgb(14, 34, 20),
    code_surface: Color::Rgb(18, 42, 26),
    border: Color::Rgb(35, 80, 50),
    border_focus: Color::Rgb(52, 211, 153),
    title: Color::Rgb(52, 211, 153),
    section_title: Color::Rgb(110, 231, 183),
    separator: Color::Rgb(50, 110, 70),
    text: Color::Rgb(209, 250, 229),
    muted: Color::Rgb(52, 211, 153),
    faint: Color::Rgb(30, 110, 70),
    user: Color::Rgb(167, 243, 208),
    agent: Color::Rgb(52, 211, 153),
    activity: Color::Rgb(251, 191, 36),
    success: Color::Rgb(52, 211, 153),
    warning: Color::Rgb(245, 158, 11),
    error: Color::Rgb(239, 68, 68),
};

const MONOCHROME_PALETTE: Palette = Palette {
    canvas: Color::Rgb(12, 12, 12),
    panel: Color::Rgb(22, 22, 22),
    modal_surface: Color::Rgb(28, 28, 28),
    code_surface: Color::Rgb(34, 34, 34),
    border: Color::Rgb(75, 75, 75),
    border_focus: Color::Rgb(240, 240, 240),
    title: Color::Rgb(240, 240, 240),
    section_title: Color::Rgb(190, 190, 190),
    separator: Color::Rgb(90, 90, 90),
    text: Color::Rgb(240, 240, 240),
    muted: Color::Rgb(160, 160, 160),
    faint: Color::Rgb(100, 100, 100),
    user: Color::Rgb(255, 255, 255),
    agent: Color::Rgb(210, 210, 210),
    activity: Color::Rgb(251, 191, 36),
    success: Color::Rgb(52, 211, 153),
    warning: Color::Rgb(245, 158, 11),
    error: Color::Rgb(239, 68, 68),
};

/// Single place to reskin the app. The UI consumes semantic roles from this
/// type instead of choosing colors for individual widgets.
pub struct Theme;

impl Theme {
    fn palette() -> &'static Palette {
        match active_preset() {
            Preset::Amber => &AMBER_PALETTE,
            Preset::Green => &GREEN_PALETTE,
            Preset::Monochrome => &MONOCHROME_PALETTE,
            Preset::Default => &DEFAULT_PALETTE,
        }
    }

    // Surfaces
    pub fn canvas() -> Style {
        Style::default().bg(Self::palette().canvas)
    }

    pub fn panel() -> Style {
        Style::default().bg(Self::palette().panel)
    }

    pub fn modal_surface() -> Style {
        Style::default().bg(Self::palette().modal_surface)
    }

    pub fn code_surface() -> Style {
        Style::default().bg(Self::palette().code_surface)
    }

    // Chrome
    pub fn border() -> Style {
        Style::default().fg(Self::palette().border)
    }

    pub fn border_focus() -> Style {
        Style::default().fg(Self::palette().border_focus)
    }

    pub fn title() -> Style {
        Style::default()
            .fg(Self::palette().title)
            .add_modifier(Modifier::BOLD)
    }

    pub fn section_title() -> Style {
        Style::default()
            .fg(Self::palette().section_title)
            .add_modifier(Modifier::BOLD)
    }

    pub fn separator() -> Style {
        Style::default().fg(Self::palette().separator)
    }

    // Text
    pub fn text() -> Style {
        Style::default().fg(Self::palette().text)
    }

    pub fn muted() -> Style {
        Style::default().fg(Self::palette().muted)
    }

    pub fn faint() -> Style {
        Style::default().fg(Self::palette().faint)
    }

    pub fn hint() -> Style {
        Self::faint()
    }

    // Interaction
    pub fn key_hint() -> Style {
        Style::default()
            .fg(Self::palette().canvas)
            .bg(Self::palette().border_focus)
            .add_modifier(Modifier::BOLD)
    }

    pub fn command_hint() -> Style {
        Style::default()
            .fg(Self::palette().user)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected_row() -> Style {
        Style::default()
            .fg(Self::palette().canvas)
            .bg(Self::palette().border_focus)
            .add_modifier(Modifier::BOLD)
    }

    pub fn empty_state() -> Style {
        Self::muted()
    }

    // Conversation
    pub fn user_label() -> Style {
        Style::default()
            .fg(Self::palette().user)
            .add_modifier(Modifier::BOLD)
    }

    pub fn agent_label() -> Style {
        Style::default()
            .fg(Self::palette().agent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn user_text() -> Style {
        Self::text()
    }

    pub fn agent_text() -> Style {
        Style::default().fg(Self::palette().text)
    }

    // State
    pub fn status() -> Style {
        Self::faint()
    }

    pub fn activity() -> Style {
        Style::default()
            .fg(Self::palette().activity)
            .add_modifier(Modifier::BOLD)
    }

    pub fn spinner() -> Style {
        Self::activity()
    }

    pub fn success() -> Style {
        Style::default().fg(Self::palette().success)
    }

    pub fn warning() -> Style {
        Style::default().fg(Self::palette().warning)
    }

    pub fn error() -> Style {
        Style::default().fg(Self::palette().error)
    }

    // Telemetry
    pub fn gauge_vram() -> Style {
        Style::default().fg(Self::palette().section_title)
    }

    pub fn gauge_ram() -> Style {
        Style::default().fg(Self::palette().user)
    }

    pub fn gauge_gpu() -> Style {
        Style::default().fg(Self::palette().section_title)
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
        Self::code_surface().fg(Self::palette().user)
    }

    pub fn md_link() -> Style {
        Style::default()
            .fg(Self::palette().user)
            .add_modifier(Modifier::UNDERLINED)
    }

    pub fn md_blockquote() -> Style {
        Style::default()
            .fg(Self::palette().agent)
            .add_modifier(Modifier::ITALIC)
    }

    pub fn md_h1() -> Style {
        Style::default()
            .fg(Self::palette().title)
            .add_modifier(Modifier::BOLD)
    }

    pub fn md_h2() -> Style {
        Style::default()
            .fg(Self::palette().section_title)
            .add_modifier(Modifier::BOLD)
    }

    pub fn md_h3() -> Style {
        Style::default()
            .fg(Self::palette().activity)
            .add_modifier(Modifier::ITALIC)
            .add_modifier(Modifier::BOLD)
    }

    pub fn md_bold() -> Style {
        Style::default()
            .fg(Self::palette().text)
            .add_modifier(Modifier::BOLD)
    }

    pub fn md_italic() -> Style {
        Style::default()
            .fg(Self::palette().muted)
            .add_modifier(Modifier::ITALIC)
    }

    pub fn md_hr() -> Style {
        Self::faint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preset_styles_have_expected_structure() {
        let canvas = Theme::canvas();
        assert!(canvas.bg.is_some());
        let title = Theme::title();
        assert!(title.add_modifier.contains(Modifier::BOLD));
        assert!(title.fg.is_some());
    }

    #[test]
    fn gauge_ctx_shifts_color() {
        let low = Theme::gauge_ctx(0.2);
        let mid = Theme::gauge_ctx(0.75);
        let high = Theme::gauge_ctx(0.95);
        assert_ne!(low, mid);
        assert_ne!(mid, high);
    }
}

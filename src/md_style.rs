//! Markdown styling bridge: maps tui-markdown's StyleSheet hooks to Veritas
//! theme tokens (rule 4: no colors outside theme.rs) and wraps rendered
//! lines to the transcript width so follow-tail row math stays exact (D011).

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use tui_markdown::StyleSheet;
use unicode_width::UnicodeWidthChar;

use crate::theme::Theme;

/// Veritas look for markdown output. Unoverridden hooks keep the library's
/// defaults; anything we restyle reads its color from `Theme`.
#[derive(Clone, Copy, Default)]
pub struct VeritasMd;

impl StyleSheet for VeritasMd {
    fn code(&self) -> Style {
        Theme::md_code()
    }

    fn link(&self) -> Style {
        Theme::md_link()
    }

    fn blockquote(&self) -> Style {
        Theme::md_blockquote()
    }

    // Chat reader, not a markdown source view: heading level is conveyed by
    // the heading style, not literal `###` markers; fenced blocks by their
    // highlighted content, not ``` delimiters. The hooks exist for this.
    fn heading_marker(&self, _level: u8) -> &str {
        ""
    }

    fn code_block_fence(&self) -> &str {
        ""
    }
}

/// Wrap one styled ratatui line to `width` display columns, preserving the
/// style of every span. Line-aware (respects the renderer's indentation
/// prefixes); Unicode-width aware so CJK content doesn't overflow.
fn wrap_line(line: &Line<'_>, width: usize, out: &mut Vec<Line<'static>>) {
    let total: usize = line.spans.iter().map(|s| s.width()).sum();
    if total <= width {
        let owned: Vec<Span<'static>> = line
            .spans
            .iter()
            .map(|s| Span::styled(s.content.to_string(), s.style))
            .collect();
        out.push(Line::from(owned));
        return;
    }
    let mut row: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for span in &line.spans {
        let mut chunk = String::new();
        for ch in span.content.chars() {
            let cw = ch.width().unwrap_or(0);
            if used + cw > width && (!chunk.is_empty() || !row.is_empty()) {
                if !chunk.is_empty() {
                    row.push(Span::styled(std::mem::take(&mut chunk), span.style));
                }
                out.push(Line::from(std::mem::take(&mut row)));
                used = 0;
            }
            chunk.push(ch);
            used += cw;
        }
        if !chunk.is_empty() {
            row.push(Span::styled(chunk, span.style));
        }
    }
    if !row.is_empty() {
        out.push(Line::from(row));
    }
}

/// Render markdown to transcript rows pre-wrapped to `width` columns.
/// Word-level wrapping happens inside the library; this is a column guard
/// that keeps D011's exact row-count contract intact.
pub fn render_wrapped(markdown: &str, width: usize) -> Vec<Line<'static>> {
    let text =
        tui_markdown::from_str_with_options(markdown, &tui_markdown::Options::new(VeritasMd));
    let mut out = Vec::new();
    for line in &text.lines {
        wrap_line(line, width, &mut out);
    }
    out
}

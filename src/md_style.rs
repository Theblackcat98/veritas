//! Markdown styling bridge: maps tui-markdown's StyleSheet hooks to Veritas
//! theme tokens (rule 4: no colors outside theme.rs) and wraps rendered
//! lines to the transcript width so follow-tail row math stays exact (D011).

use ratatui::style::{Modifier, Style};
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
    fn heading_marker(&self, level: u8) -> &str {
        // Show heading markers for visual distinction
        match level {
            1 => "# ",
            2 => "## ",
            3 => "### ",
            4 => "#### ",
            5 => "##### ",
            _ => "###### ",
        }
    }

    fn code_block_fence(&self) -> &str {
        "```" // Show code block fences
    }

    // Style for heading markers to match heading level colors
    fn heading_meta(&self) -> Style {
        Theme::md_h3() // Use H3 style for heading markers
    }

    // Enhanced styling for different heading levels
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => Theme::md_h1(),
            2 => Theme::md_h2(),
            _ => Theme::md_h3(),
        }
    }

    // Table styling for better readability
    fn table_header(&self) -> Style {
        Theme::md_h2() // Use H2 style for table headers
    }

    fn table_cell(&self) -> Style {
        Theme::agent_text() // Use normal agent text for table cells
    }

    fn table_border(&self) -> Style {
        Theme::md_hr() // Use horizontal rule style for table borders
    }

    // HTML styling (if any HTML content appears)
    fn html(&self) -> Style {
        Theme::faint() // Dim HTML content
    }

    // Image alt text styling
    fn image_alt(&self) -> Style {
        Theme::faint() // Dim image alt text
    }
}

struct StyledToken<'a> {
    text: &'a str,
    style: Style,
    is_whitespace: bool,
    width: usize,
}

fn tokenize_line<'a>(line: &'a Line<'_>) -> Vec<StyledToken<'a>> {
    let mut tokens = Vec::new();
    for span in &line.spans {
        let content = span.content.as_ref();
        if content.is_empty() {
            continue;
        }
        let chars: Vec<(usize, char)> = content.char_indices().collect();
        let mut i = 0;
        while i < chars.len() {
            let is_ws = chars[i].1.is_whitespace();
            let chunk_start = chars[i].0;
            while i < chars.len() && chars[i].1.is_whitespace() == is_ws {
                i += 1;
            }
            let chunk_end = if i < chars.len() {
                chars[i].0
            } else {
                content.len()
            };
            let text = &content[chunk_start..chunk_end];
            tokens.push(StyledToken {
                text,
                style: span.style,
                is_whitespace: is_ws,
                width: unicode_width::UnicodeWidthStr::width(text),
            });
        }
    }
    tokens
}

/// Wrap one styled ratatui line to `width` display columns using word-aware wrapping,
/// preserving the style of every span. Never breaks words unless an individual word
/// exceeds the column width. Unicode-width aware so CJK content doesn't overflow.
fn wrap_line(line: &Line<'_>, width: usize, out: &mut Vec<Line<'static>>) {
    let width = width.max(1);
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

    let tokens = tokenize_line(line);
    let mut current_row: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;
    let initial_line = out.is_empty();

    for token in tokens {
        if token.is_whitespace {
            // Drop leading whitespace at the start of wrapped lines, but keep on initial line for indentation
            if current_row.is_empty() && !initial_line {
                continue;
            }
            if current_width + token.width <= width {
                current_row.push(Span::styled(token.text.to_string(), token.style));
                current_width += token.width;
            } else {
                if !current_row.is_empty() {
                    out.push(Line::from(std::mem::take(&mut current_row)));
                    current_width = 0;
                }
            }
        } else {
            // Word token
            if current_width + token.width <= width {
                current_row.push(Span::styled(token.text.to_string(), token.style));
                current_width += token.width;
            } else {
                // Word doesn't fit on current row: break to next line
                if !current_row.is_empty() {
                    out.push(Line::from(std::mem::take(&mut current_row)));
                    current_width = 0;
                }
                if token.width <= width {
                    current_row.push(Span::styled(token.text.to_string(), token.style));
                    current_width += token.width;
                } else {
                    // Single word exceeds full column width: break by character
                    for ch in token.text.chars() {
                        let cw = ch.width().unwrap_or(0);
                        if current_width + cw > width && !current_row.is_empty() {
                            out.push(Line::from(std::mem::take(&mut current_row)));
                            current_width = 0;
                        }
                        current_row.push(Span::styled(ch.to_string(), token.style));
                        current_width += cw;
                    }
                }
            }
        }
    }
    if !current_row.is_empty() {
        out.push(Line::from(current_row));
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
        let final_line = style_line(line, width);
        wrap_line(&final_line, width, &mut out);
    }
    out
}

/// Process a single line: enhance bold/italic and handle horizontal rules
fn style_line(line: &Line<'_>, width: usize) -> Line<'static> {
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

    // Check if this line looks like a horizontal rule
    let is_hr = text
        .trim()
        .chars()
        .all(|c| c == '-' || c == '_' || c == '*')
        && text.trim().len() >= 3;

    // Check if this line is a code block fence
    let is_fence = text.trim().starts_with("```");

    if is_hr {
        let hr_len = 40.min(width).max(8);
        Line::from(Span::styled("─".repeat(hr_len), Theme::md_hr()))
    } else if is_fence {
        Line::from(Span::styled(text, Theme::md_code()))
    } else {
        enhance_bold_italic(line)
    }
}

/// Enhance bold and italic spans with colors from the theme.
/// tui-markdown handles the modifiers but we add colors for better visibility.
fn enhance_bold_italic(line: &Line<'_>) -> Line<'static> {
    let enhanced_spans: Vec<Span<'static>> = line
        .spans
        .iter()
        .map(|span| {
            let style = span.style;
            let content: String = span.content.to_string();

            // Add colors to bold and italic text
            let is_bold = style.add_modifier.contains(Modifier::BOLD);
            let is_italic = style.add_modifier.contains(Modifier::ITALIC);

            let enhanced_style = if is_bold {
                Theme::md_bold()
            } else if is_italic {
                Theme::md_italic()
            } else {
                style
            };

            Span::styled(content, enhanced_style)
        })
        .collect();

    Line::from(enhanced_spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_wrapped_handles_headings() {
        let md = "# Title 1\n## Subtitle 2\nBody text";
        let lines = render_wrapped(md, 50);
        assert!(!lines.is_empty());
        let full_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(full_text.contains("Title 1"));
        assert!(full_text.contains("Subtitle 2"));
        assert!(full_text.contains("Body text"));
    }

    #[test]
    fn render_wrapped_horizontal_rule_scales_properly() {
        let md = "Above\n\n---\n\nBelow";
        let lines = render_wrapped(md, 50);
        let hr_lines: Vec<_> = lines
            .iter()
            .filter(|l| {
                let s: String = l.spans.iter().map(|sp| sp.content.as_ref()).collect();
                s.starts_with('─')
            })
            .collect();
        assert_eq!(hr_lines.len(), 1, "must render one horizontal rule");
        let hr_str: String = hr_lines[0]
            .spans
            .iter()
            .map(|sp| sp.content.as_ref())
            .collect();
        assert_eq!(
            hr_str.chars().count(),
            40,
            "HR must be 40 chars wide in 50-col view"
        );
    }

    #[test]
    fn render_wrapped_respects_narrow_column_width() {
        let long_line = "word ".repeat(30);
        let lines = render_wrapped(&long_line, 20);
        assert!(lines.len() > 1);
        for l in &lines {
            let width: usize = l.spans.iter().map(|s| s.width()).sum();
            assert!(width <= 20, "line width {width} must not exceed 20");
        }
    }

    #[test]
    fn render_wrapped_code_block_fences() {
        let md = "```rust\nlet x = 42;\n```";
        let lines = render_wrapped(md, 50);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(text.contains("let x = 42;"));
    }

    #[test]
    fn render_wrapped_never_splits_words() {
        // "This is a solid answer" at width 12:
        // "This is a " (10) fits, "solid" (5) would make 15 > 12,
        // so "solid" must move to the next line intact and NOT become "s\nolid"
        let md = "This is a solid answer";
        let lines = render_wrapped(md, 12);
        assert_eq!(lines.len(), 2);
        let line1: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        let line2: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(line1.trim(), "This is a");
        assert_eq!(line2.trim(), "solid answer");
        assert!(!line1.ends_with('s') || line1.trim().ends_with("This is a"));
        assert!(line2.starts_with("solid"));
    }
}

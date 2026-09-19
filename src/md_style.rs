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
        Theme::picker_dim() // Dim HTML content
    }

    // Image alt text styling
    fn image_alt(&self) -> Style {
        Theme::picker_dim() // Dim image alt text
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
        let final_line = style_line(line);
        wrap_line(&final_line, width, &mut out);
    }
    out
}

/// Process a single line: enhance bold/italic and handle horizontal rules
fn style_line(line: &Line<'_>) -> Line<'static> {
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    
    // Check if this line looks like a horizontal rule
    let is_hr = text.trim().chars().all(|c| c == '-' || c == '_' || c == '*') && 
                text.trim().len() >= 3;
    
    // Check if this line is a code block fence
    let is_fence = text.trim().starts_with("```");
    
    if is_hr {
        Line::from(Span::styled("─".repeat(text.trim().len().min(50)), Theme::md_hr()))
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

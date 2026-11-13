use crate::app::ArticleMatch;
use crate::ui::style_analyzer::{StyleAnalyzer, TextStyle};
use crate::ui::ColorScheme;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::borrow::Cow;

/// Renders article text into styled `ratatui` lines according to a style strategy.
pub trait Renderer {
    fn render(&self, text: &str, colors: &ColorScheme) -> Vec<Line<'static>>;
}

impl TextStyle {
    /// Return the concrete [`Renderer`] corresponding to this text style.
    pub fn renderer(&self) -> Box<dyn Renderer> {
        select_renderer(*self)
    }
}

/// Select the concrete [`Renderer`] for the given [`TextStyle`].
pub fn select_renderer(style: TextStyle) -> Box<dyn Renderer> {
    match style {
        TextStyle::Plain => Box::new(PlainRenderer::new()),
        TextStyle::Markdown => Box::new(MarkdownRenderer::new()),
    }
}

/// Analyze `text` with [`StyleAnalyzer`] and render it using the matching [`Renderer`].
pub fn render_article(text: &str, colors: &ColorScheme) -> Vec<Line<'static>> {
    let style = StyleAnalyzer::analyze(text);
    style.renderer().render(text, colors)
}

/// Renders plain text verbatim as unstyled lines.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlainRenderer;

impl PlainRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Renderer for PlainRenderer {
    fn render(&self, text: &str, _colors: &ColorScheme) -> Vec<Line<'static>> {
        text.split('\n')
            .map(|line| Line::from(line.to_string()))
            .collect()
    }
}

/// Renders Markdown-formatted article text into styled `ratatui` lines.
#[derive(Clone, Copy, Debug, Default)]
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Renderer for MarkdownRenderer {
    fn render(&self, text: &str, colors: &ColorScheme) -> Vec<Line<'static>> {
        let mut out = Vec::new();
        let mut in_fence: Option<char> = None;
        let code_line_style = Style::default().fg(colors.accent).bg(colors.selected_bg);

        for line in text.split('\n') {
            let trimmed_start = line.trim_start();

            if let Some(fence_ch) = in_fence {
                if is_closing_fence(trimmed_start, fence_ch) {
                    in_fence = None;
                } else {
                    out.push(Line::from(Span::styled(
                        format!("    {}", line),
                        code_line_style,
                    )));
                }
                continue;
            }

            if let Some(fence_ch) = parse_opening_fence(trimmed_start) {
                in_fence = Some(fence_ch);
                continue;
            }

            if line.trim().is_empty() {
                out.push(Line::from(""));
                continue;
            }

            if is_horizontal_rule(trimmed_start) {
                out.push(Line::from(Span::styled(
                    "─".repeat(40),
                    Style::default().fg(colors.border),
                )));
                continue;
            }

            if let Some((level, heading_text)) = parse_atx_heading(trimmed_start) {
                let heading_style = match level {
                    1 => Style::default()
                        .fg(colors.primary)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    2 => Style::default()
                        .fg(colors.secondary)
                        .add_modifier(Modifier::BOLD),
                    3 => Style::default()
                        .fg(colors.accent)
                        .add_modifier(Modifier::BOLD),
                    _ => Style::default()
                        .fg(colors.text_secondary)
                        .add_modifier(Modifier::BOLD),
                };
                out.push(Line::from(parse_inline_spans(
                    heading_text,
                    heading_style,
                    colors,
                )));
                continue;
            }

            if let Some((depth, quote_text)) = parse_blockquote(trimmed_start) {
                let prefix =
                    Span::styled("  │ ".repeat(depth), Style::default().fg(colors.secondary));
                let quote_style = Style::default()
                    .fg(colors.text_secondary)
                    .add_modifier(Modifier::ITALIC);
                let mut spans = vec![prefix];
                if !quote_text.is_empty() {
                    spans.extend(parse_inline_spans(quote_text, quote_style, colors));
                }
                out.push(Line::from(spans));
                continue;
            }

            if let Some((indent, item_text)) = parse_unordered_list(line) {
                let bullet = Span::styled(
                    format!("{}  {} ", "  ".repeat(indent), colors.get_list_bullet()),
                    Style::default().fg(colors.highlight),
                );
                let mut spans = vec![bullet];
                spans.extend(parse_inline_spans(
                    item_text,
                    Style::default().fg(colors.text),
                    colors,
                ));
                out.push(Line::from(spans));
                continue;
            }

            if let Some((indent, num, item_text)) = parse_ordered_list(line) {
                let number_span = Span::styled(
                    format!("{}  {}. ", "  ".repeat(indent), num),
                    Style::default()
                        .fg(colors.highlight)
                        .add_modifier(Modifier::BOLD),
                );
                let mut spans = vec![number_span];
                spans.extend(parse_inline_spans(
                    item_text,
                    Style::default().fg(colors.text),
                    colors,
                ));
                out.push(Line::from(spans));
                continue;
            }

            out.push(Line::from(parse_inline_spans(
                line,
                Style::default().fg(colors.text),
                colors,
            )));
        }

        out
    }
}

/// Join the visible text of rendered `Line`s with `'\n'`.
pub fn lines_to_plain_text(lines: &[Line<'_>]) -> String {
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        for span in &line.spans {
            out.push_str(span.content.as_ref());
        }
    }
    out
}

/// Overlay search match highlights onto already-styled `Line`s while
/// preserving original span styling on unmatched segments.
pub fn highlight_lines<'a>(
    lines: Vec<Line<'a>>,
    matches: &[ArticleMatch],
    current_idx: Option<usize>,
    colors: &ColorScheme,
) -> Vec<Line<'a>> {
    if matches.is_empty() {
        return lines;
    }

    let highlight_style = Style::default().fg(colors.highlight);
    let current_style = Style::default()
        .bg(colors.highlight)
        .fg(colors.surface)
        .add_modifier(Modifier::BOLD);

    let mut match_cursor = 0usize;

    lines
        .into_iter()
        .enumerate()
        .map(|(line_idx, line)| {
            while match_cursor < matches.len() && matches[match_cursor].line < line_idx {
                match_cursor += 1;
            }
            let line_start = match_cursor;
            while match_cursor < matches.len() && matches[match_cursor].line == line_idx {
                match_cursor += 1;
            }
            let line_matches = &matches[line_start..match_cursor];
            if line_matches.is_empty() {
                return line;
            }

            let mut new_spans: Vec<Span<'a>> =
                Vec::with_capacity(line.spans.len() + line_matches.len() * 2);
            let mut span_offset = 0usize;

            for span in line.spans {
                let text = span.content;
                let style = span.style;
                let span_len = text.len();
                let span_end = span_offset + span_len;

                if span_len == 0 {
                    new_spans.push(Span {
                        content: text,
                        style,
                    });
                    continue;
                }

                // Check if any match overlaps [span_offset, span_end).
                let has_overlap = line_matches
                    .iter()
                    .any(|m| m.end > span_offset && m.start < span_end);
                if !has_overlap {
                    new_spans.push(Span {
                        content: text,
                        style,
                    });
                    span_offset = span_end;
                    continue;
                }

                let mut local_pos = 0usize;
                for (offset, m) in line_matches.iter().enumerate() {
                    if m.end <= span_offset || m.start >= span_end {
                        continue;
                    }
                    let rel_start = m.start.saturating_sub(span_offset).max(local_pos);
                    let rel_end = (m.end - span_offset).min(span_len);
                    if rel_start > local_pos {
                        new_spans.push(slice_span(&text, local_pos, rel_start, style));
                    }
                    if rel_end > rel_start {
                        let m_style = if Some(line_start + offset) == current_idx {
                            current_style
                        } else {
                            highlight_style
                        };
                        new_spans.push(slice_span(&text, rel_start, rel_end, m_style));
                        local_pos = rel_end;
                    }
                }
                if local_pos < span_len {
                    new_spans.push(slice_span(&text, local_pos, span_len, style));
                }

                span_offset = span_end;
            }

            Line::from(new_spans)
        })
        .collect()
}

fn slice_span<'a>(text: &Cow<'a, str>, start: usize, end: usize, style: Style) -> Span<'a> {
    match text {
        Cow::Borrowed(s) => Span::styled(&s[start..end], style),
        Cow::Owned(s) => Span::styled(s[start..end].to_string(), style),
    }
}

fn parse_opening_fence(trimmed: &str) -> Option<char> {
    if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

fn is_closing_fence(trimmed: &str, fence_ch: char) -> bool {
    let count = trimmed.chars().take_while(|&c| c == fence_ch).count();
    count >= 3 && trimmed[count..].trim().is_empty()
}

fn is_horizontal_rule(trimmed: &str) -> bool {
    let stripped = trimmed.trim_end();
    let mut marker: Option<char> = None;
    let mut count = 0usize;
    for ch in stripped.chars() {
        if ch == ' ' || ch == '\t' {
            continue;
        }
        if !matches!(ch, '-' | '*' | '_') {
            return false;
        }
        match marker {
            None => {
                marker = Some(ch);
                count = 1;
            }
            Some(m) if m == ch => count += 1,
            Some(_) => return false,
        }
    }
    count >= 3
}

fn parse_atx_heading(trimmed: &str) -> Option<(usize, &str)> {
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !matches!(rest.as_bytes().first(), Some(b' ' | b'\t')) {
        return None;
    }
    let content = strip_trailing_hashes(rest.trim());
    if content.is_empty() {
        return None;
    }
    Some((hashes, content))
}

fn strip_trailing_hashes(s: &str) -> &str {
    let trimmed = s.trim_end_matches('#');
    if trimmed.len() == s.len() {
        return s;
    }
    if trimmed.is_empty() || trimmed.ends_with(|c: char| c.is_whitespace()) {
        trimmed.trim_end()
    } else {
        s
    }
}

fn parse_blockquote(trimmed: &str) -> Option<(usize, &str)> {
    let after = trimmed.trim_start_matches('>');
    let depth = trimmed.len() - after.len();
    if depth == 0 {
        return None;
    }
    if after.is_empty() {
        return Some((depth, ""));
    }
    if let Some(rest) = after.strip_prefix(' ') {
        return Some((depth, rest));
    }
    None
}

fn parse_unordered_list(line: &str) -> Option<(usize, &str)> {
    let leading_spaces = line.bytes().take_while(|&b| b == b' ').count();
    let trimmed = &line[leading_spaces..];
    let rest = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
        .or_else(|| trimmed.strip_prefix("• "))?;
    if rest.trim().is_empty() {
        return None;
    }
    Some((leading_spaces / 2, rest))
}

fn parse_ordered_list(line: &str) -> Option<(usize, &str, &str)> {
    let leading_spaces = line.bytes().take_while(|&b| b == b' ').count();
    let trimmed = &line[leading_spaces..];
    let digits = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
    if !(1..=9).contains(&digits) {
        return None;
    }
    let num = &trimmed[..digits];
    let rest = trimmed[digits..].strip_prefix(". ")?;
    if rest.trim().is_empty() {
        return None;
    }
    Some((leading_spaces / 2, num, rest))
}

fn parse_inline_spans(text: &str, base_style: Style, colors: &ColorScheme) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain_buf = String::new();
    let mut i = 0usize;

    let flush_plain = |buf: &mut String, out: &mut Vec<Span<'static>>| {
        if !buf.is_empty() {
            out.push(Span::styled(std::mem::take(buf), base_style));
        }
    };

    while i < text.len() {
        let rest = &text[i..];
        let b = rest.as_bytes()[0];

        // Backslash escape for ASCII punctuation.
        if b == b'\\' && rest.len() > 1 {
            let next_ch = rest[1..].chars().next().unwrap();
            if next_ch.is_ascii_punctuation() {
                plain_buf.push(next_ch);
                i += 1 + next_ch.len_utf8();
                continue;
            }
        }

        // Inline code: `code`
        if b == b'`' {
            if let Some(rel_end) = rest[1..].find('`') {
                let inner = &rest[1..1 + rel_end];
                if !inner.is_empty() {
                    flush_plain(&mut plain_buf, &mut spans);
                    spans.push(Span::styled(
                        inner.to_string(),
                        base_style.fg(colors.accent).bg(colors.selected_bg),
                    ));
                    i += 2 + rel_end;
                    continue;
                }
            }
        }

        // Image: ![alt](url)
        if rest.starts_with("![") {
            if let Some((alt, _url, consumed)) = parse_bracket_paren(&rest[1..]) {
                flush_plain(&mut plain_buf, &mut spans);
                let label = if alt.is_empty() { "image" } else { alt };
                spans.push(Span::styled(
                    format!("[{}]", label),
                    base_style.fg(colors.muted).add_modifier(Modifier::ITALIC),
                ));
                i += 1 + consumed;
                continue;
            }
        }

        // Link: [label](url)
        if b == b'[' {
            if let Some((label, url, consumed)) = parse_bracket_paren(rest) {
                flush_plain(&mut plain_buf, &mut spans);
                let link_style = base_style
                    .fg(colors.primary)
                    .add_modifier(Modifier::UNDERLINED);
                let visible = if label.is_empty() { url } else { label };
                spans.extend(parse_inline_spans(visible, link_style, colors));
                i += consumed;
                continue;
            }
        }

        // Bold + Italic: ***text*** or ___text___
        if rest.starts_with("***") || rest.starts_with("___") {
            let delim = &rest[..3];
            let boundary_ok = delim == "***" || prev_is_non_alnum(text, i);
            if boundary_ok {
                if let Some(rel_end) = find_closing_delim(rest, 3, delim, delim == "___") {
                    let inner = &rest[3..3 + rel_end];
                    if is_valid_inner(inner) {
                        flush_plain(&mut plain_buf, &mut spans);
                        spans.extend(parse_inline_spans(
                            inner,
                            base_style.add_modifier(Modifier::BOLD | Modifier::ITALIC),
                            colors,
                        ));
                        i += 3 + rel_end + 3;
                        continue;
                    }
                }
            }
        }

        // Bold: **text** or __text__
        if rest.starts_with("**") || rest.starts_with("__") {
            let delim = &rest[..2];
            let boundary_ok = delim == "**" || prev_is_non_alnum(text, i);
            if boundary_ok {
                if let Some(rel_end) = find_closing_delim(rest, 2, delim, delim == "__") {
                    let inner = &rest[2..2 + rel_end];
                    if is_valid_inner(inner) {
                        flush_plain(&mut plain_buf, &mut spans);
                        spans.extend(parse_inline_spans(
                            inner,
                            base_style.add_modifier(Modifier::BOLD),
                            colors,
                        ));
                        i += 2 + rel_end + 2;
                        continue;
                    }
                }
            }
        }

        // Strikethrough: ~~text~~
        if rest.starts_with("~~") {
            if let Some(rel_end) = find_closing_delim(rest, 2, "~~", false) {
                let inner = &rest[2..2 + rel_end];
                if is_valid_inner(inner) {
                    flush_plain(&mut plain_buf, &mut spans);
                    spans.extend(parse_inline_spans(
                        inner,
                        base_style.add_modifier(Modifier::CROSSED_OUT),
                        colors,
                    ));
                    i += 2 + rel_end + 2;
                    continue;
                }
            }
        }

        // Italic: *text* or _text_
        if b == b'*' || b == b'_' {
            let marker = b as char;
            let delim = if b == b'*' { "*" } else { "_" };
            let boundary_ok = marker == '*' || prev_is_non_alnum(text, i);
            if boundary_ok && rest.as_bytes().get(1) != Some(&b) {
                if let Some(rel_end) = find_closing_single(rest, 1, b, marker == '_') {
                    let inner = &rest[1..1 + rel_end];
                    if is_valid_inner(inner) {
                        flush_plain(&mut plain_buf, &mut spans);
                        spans.extend(parse_inline_spans(
                            inner,
                            base_style.add_modifier(Modifier::ITALIC),
                            colors,
                        ));
                        i += 1 + rel_end + 1;
                        continue;
                    }
                }
            }
            // Literal marker if not matched
            let _ = delim;
        }

        let ch = rest.chars().next().unwrap();
        plain_buf.push(ch);
        i += ch.len_utf8();
    }

    flush_plain(&mut plain_buf, &mut spans);
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base_style));
    }
    spans
}

fn parse_bracket_paren(s: &str) -> Option<(&str, &str, usize)> {
    if !s.starts_with('[') {
        return None;
    }
    let close_bracket = s[1..].find("](")? + 1;
    let label = &s[1..close_bracket];
    let url_start = close_bracket + 2;
    let rel_paren = s[url_start..].find(')')?;
    let url = &s[url_start..url_start + rel_paren];
    if url.is_empty() || url.chars().any(char::is_whitespace) {
        return None;
    }
    Some((label, url, url_start + rel_paren + 1))
}

fn find_closing_delim(
    s: &str,
    start: usize,
    delim: &str,
    check_word_boundary: bool,
) -> Option<usize> {
    let dlen = delim.len();
    let mut i = start;
    while i + dlen <= s.len() {
        if s.as_bytes()[i] == b'\\' {
            i += 2;
            continue;
        }
        if s[i..].starts_with(delim) && (!check_word_boundary || next_is_non_alnum(s, i + dlen)) {
            return Some(i - start);
        }
        let ch_len = s[i..].chars().next().map_or(1, char::len_utf8);
        i += ch_len;
    }
    None
}

fn find_closing_single(
    s: &str,
    start: usize,
    marker: u8,
    check_word_boundary: bool,
) -> Option<usize> {
    let mut i = start;
    while i < s.len() {
        if s.as_bytes()[i] == b'\\' {
            i += 2;
            continue;
        }
        if s.as_bytes()[i] == marker {
            if s.as_bytes().get(i + 1) == Some(&marker) {
                while i < s.len() && s.as_bytes()[i] == marker {
                    i += 1;
                }
                continue;
            }
            if !check_word_boundary || next_is_non_alnum(s, i + 1) {
                return Some(i - start);
            }
        }
        let ch_len = s[i..].chars().next().map_or(1, char::len_utf8);
        i += ch_len;
    }
    None
}

fn is_valid_inner(inner: &str) -> bool {
    if inner.is_empty() {
        return false;
    }
    let first = inner.chars().next().unwrap();
    let last = inner.chars().next_back().unwrap();
    !first.is_whitespace() && !last.is_whitespace()
}

fn prev_is_non_alnum(s: &str, idx: usize) -> bool {
    match s[..idx].chars().next_back() {
        None => true,
        Some(c) => !c.is_alphanumeric(),
    }
}

fn next_is_non_alnum(s: &str, idx: usize) -> bool {
    match s[idx..].chars().next() {
        None => true,
        Some(c) => !c.is_alphanumeric(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::find_article_matches;

    #[test]
    fn plain_renderer_preserves_lines_verbatim() {
        let colors = ColorScheme::dark();
        let renderer = PlainRenderer::new();
        let lines = renderer.render("alpha\nbeta\ngamma", &colors);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines_to_plain_text(&lines), "alpha\nbeta\ngamma");
    }

    #[test]
    fn markdown_renderer_styles_headings_and_strips_hashes() {
        let colors = ColorScheme::dark();
        let renderer = MarkdownRenderer::new();
        let lines = renderer.render("# Title\n## Sub\n### Third\n#### Fourth", &colors);
        assert_eq!(lines_to_plain_text(&lines), "Title\nSub\nThird\nFourth");
        assert_eq!(
            lines[0].spans[0].style,
            Style::default()
                .fg(colors.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        );
        assert_eq!(
            lines[1].spans[0].style,
            Style::default()
                .fg(colors.secondary)
                .add_modifier(Modifier::BOLD)
        );
        assert_eq!(
            lines[2].spans[0].style,
            Style::default()
                .fg(colors.accent)
                .add_modifier(Modifier::BOLD)
        );
        assert_eq!(
            lines[3].spans[0].style,
            Style::default()
                .fg(colors.text_secondary)
                .add_modifier(Modifier::BOLD)
        );
    }

    #[test]
    fn markdown_renderer_handles_inline_formatting() {
        let colors = ColorScheme::dark();
        let renderer = MarkdownRenderer::new();
        let lines = renderer.render(
            "Hello **bold** and *italic* and `code` and [link](https://example.com) and ~~old~~",
            &colors,
        );
        assert_eq!(
            lines_to_plain_text(&lines),
            "Hello bold and italic and code and link and old"
        );
        let spans = &lines[0].spans;
        assert!(spans.iter().any(|s| {
            s.content.as_ref() == "bold" && s.style.add_modifier.contains(Modifier::BOLD)
        }));
        assert!(spans.iter().any(|s| {
            s.content.as_ref() == "italic" && s.style.add_modifier.contains(Modifier::ITALIC)
        }));
        assert!(spans.iter().any(|s| {
            s.content.as_ref() == "code"
                && s.style.fg == Some(colors.accent)
                && s.style.bg == Some(colors.selected_bg)
        }));
        assert!(spans.iter().any(|s| {
            s.content.as_ref() == "link"
                && s.style.fg == Some(colors.primary)
                && s.style.add_modifier.contains(Modifier::UNDERLINED)
        }));
        assert!(spans.iter().any(|s| {
            s.content.as_ref() == "old" && s.style.add_modifier.contains(Modifier::CROSSED_OUT)
        }));
    }

    #[test]
    fn markdown_renderer_handles_code_blocks_lists_quotes_and_hr() {
        let colors = ColorScheme::dark();
        let renderer = MarkdownRenderer::new();
        let input = "> Quote\n\n- Bullet\n1. Numbered\n\n---\n\n```rust\nlet x = 1;\n```";
        let lines = renderer.render(input, &colors);
        let plain = lines_to_plain_text(&lines);
        assert!(plain.contains("  │ Quote"));
        assert!(plain.contains(&format!("  {} Bullet", colors.get_list_bullet())));
        assert!(plain.contains("  1. Numbered"));
        assert!(plain.contains(&"─".repeat(40)));
        assert!(plain.contains("    let x = 1;"));
        // Fence delimiters are omitted
        assert!(!plain.contains("```"));
    }

    #[test]
    fn markdown_renderer_handles_images_and_escapes_and_unclosed_delimiters() {
        let colors = ColorScheme::dark();
        let renderer = MarkdownRenderer::new();
        let lines = renderer.render(
            "See ![chart](https://example.com/a.png) and \\*literal\\* and *unclosed",
            &colors,
        );
        let plain = lines_to_plain_text(&lines);
        assert_eq!(plain, "See [chart] and *literal* and *unclosed");
    }

    #[test]
    fn render_article_dispatches_via_style_analyzer() {
        let colors = ColorScheme::dark();
        let md_lines = render_article("# Header\n\nBody **text**", &colors);
        assert_eq!(lines_to_plain_text(&md_lines), "Header\n\nBody text");

        let plain_lines = render_article("Just plain text\nsecond line", &colors);
        assert_eq!(
            lines_to_plain_text(&plain_lines),
            "Just plain text\nsecond line"
        );
        assert_eq!(plain_lines[0].spans.len(), 1);
    }

    #[test]
    fn highlight_lines_preserves_markdown_styles_and_highlights_matches() {
        let colors = ColorScheme::dark();
        let rendered = render_article("# Hello **world**\n\nAnother world line", &colors);
        let visible = lines_to_plain_text(&rendered);
        let matches = find_article_matches(&visible, "world");
        assert_eq!(matches.len(), 2);

        let highlighted = highlight_lines(rendered, &matches, Some(1), &colors);
        assert_eq!(lines_to_plain_text(&highlighted), visible);

        let quiet_style = Style::default().fg(colors.highlight);
        let current_style = Style::default()
            .bg(colors.highlight)
            .fg(colors.surface)
            .add_modifier(Modifier::BOLD);

        assert!(highlighted[0]
            .spans
            .iter()
            .any(|s| s.content.as_ref() == "world" && s.style == quiet_style));
        assert!(highlighted[2]
            .spans
            .iter()
            .any(|s| s.content.as_ref() == "world" && s.style == current_style));
        // Unmatched heading word "Hello " still carries heading style
        assert_eq!(
            highlighted[0].spans[0].style,
            Style::default()
                .fg(colors.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        );
    }
}

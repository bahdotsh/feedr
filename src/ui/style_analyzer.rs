/// Detected stylistic format of article body text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextStyle {
    #[default]
    Plain,
    Markdown,
}

/// Alias matching the article rendering domain terminology.
pub type ArticleStyle = TextStyle;

/// Analyzes article text to detect its formatting style (e.g. Markdown vs plain text).
#[derive(Clone, Copy, Debug, Default)]
pub struct StyleAnalyzer;

impl StyleAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Detect the style of `text`.
    pub fn analyze(text: &str) -> TextStyle {
        if Self::is_markdown(text) {
            TextStyle::Markdown
        } else {
            TextStyle::Plain
        }
    }

    /// Instance-method wrapper around [`StyleAnalyzer::analyze`].
    pub fn detect(&self, text: &str) -> TextStyle {
        Self::analyze(text)
    }

    /// Return `true` when `text` contains recognizable Markdown syntax.
    pub fn is_markdown(text: &str) -> bool {
        if text.trim().is_empty() {
            return false;
        }

        for line in text.lines() {
            let trimmed = line.trim_start();

            if parse_fence_open(trimmed).is_some()
                || is_atx_heading(trimmed)
                || is_blockquote(trimmed)
                || is_horizontal_rule(trimmed)
                || is_unordered_list_item(trimmed)
                || is_ordered_list_item(trimmed)
                || has_inline_markdown(line)
            {
                return true;
            }
        }

        false
    }
}

fn parse_fence_open(trimmed: &str) -> Option<char> {
    if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

fn is_atx_heading(trimmed: &str) -> bool {
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return false;
    }
    let rest = &trimmed[hashes..];
    matches!(rest.as_bytes().first(), Some(b' ' | b'\t')) && !rest.trim().is_empty()
}

fn is_blockquote(trimmed: &str) -> bool {
    let after = trimmed.trim_start_matches('>');
    let depth = trimmed.len() - after.len();
    depth >= 1 && after.starts_with(' ') && !after.trim().is_empty()
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

fn is_unordered_list_item(trimmed: &str) -> bool {
    let Some(first) = trimmed.as_bytes().first().copied() else {
        return false;
    };
    if !matches!(first, b'-' | b'*' | b'+') {
        return false;
    }
    let rest = &trimmed[1..];
    rest.starts_with(' ') && !rest.trim().is_empty()
}

fn is_ordered_list_item(trimmed: &str) -> bool {
    let digits = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
    if !(1..=9).contains(&digits) {
        return false;
    }
    let rest = &trimmed[digits..];
    if let Some(after_dot) = rest.strip_prefix(". ") {
        !after_dot.trim().is_empty()
    } else {
        false
    }
}

fn has_inline_markdown(line: &str) -> bool {
    has_inline_code(line)
        || has_delimited_span(line, "**", false)
        || has_delimited_span(line, "__", true)
        || has_delimited_span(line, "~~", false)
        || has_single_italic(line, '*')
        || has_single_italic(line, '_')
        || has_markdown_link(line)
}

fn has_inline_code(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'`' && bytes.get(i + 1) != Some(&b'`') {
            if let Some(rel_end) = line[i + 1..].find('`') {
                let inner = &line[i + 1..i + 1 + rel_end];
                if !inner.is_empty() && !inner.trim().is_empty() {
                    return true;
                }
                i += rel_end + 2;
                continue;
            }
        }
        i += 1;
    }
    false
}

fn has_delimited_span(line: &str, delim: &str, require_word_boundary: bool) -> bool {
    let dlen = delim.len();
    let mut i = 0;
    while i + dlen <= line.len() {
        if line.as_bytes()[i] == b'\\' {
            i += 2;
            continue;
        }
        if line[i..].starts_with(delim) {
            let before_ok = !require_word_boundary || prev_char_is_non_alnum(line, i);
            if before_ok {
                let content_start = i + dlen;
                if let Some(rel_end) = find_closing_double(line, content_start, delim) {
                    let end_idx = content_start + rel_end;
                    let after_ok =
                        !require_word_boundary || next_char_is_non_alnum(line, end_idx + dlen);
                    let inner = &line[content_start..end_idx];
                    if after_ok && is_valid_emphasis_inner(inner) {
                        return true;
                    }
                }
            }
            i += dlen;
            continue;
        }
        // Advance to next UTF-8 char boundary
        let ch_len = line[i..].chars().next().map_or(1, char::len_utf8);
        i += ch_len;
    }
    false
}

fn find_closing_double(line: &str, start: usize, delim: &str) -> Option<usize> {
    let dlen = delim.len();
    let mut i = start;
    while i + dlen <= line.len() {
        if line.as_bytes()[i] == b'\\' {
            i += 2;
            continue;
        }
        if line[i..].starts_with(delim) {
            return Some(i - start);
        }
        let ch_len = line[i..].chars().next().map_or(1, char::len_utf8);
        i += ch_len;
    }
    None
}

fn has_single_italic(line: &str, marker: char) -> bool {
    let m_byte = marker as u8;
    let mut i = 0;
    while i < line.len() {
        if line.as_bytes()[i] == b'\\' {
            i += 2;
            continue;
        }
        if line.as_bytes()[i] == m_byte {
            // Skip runs of 2+ identical markers (handled by double-delimiter checks).
            if line.as_bytes().get(i + 1) == Some(&m_byte) {
                while i < line.len() && line.as_bytes()[i] == m_byte {
                    i += 1;
                }
                continue;
            }
            let open_ok = if marker == '_' {
                prev_char_is_non_alnum(line, i)
            } else {
                prev_char_allows_opening_star(line, i)
            };
            if open_ok {
                let start = i + 1;
                let mut j = start;
                while j < line.len() {
                    if line.as_bytes()[j] == b'\\' {
                        j += 2;
                        continue;
                    }
                    if line.as_bytes()[j] == m_byte {
                        if line.as_bytes().get(j + 1) == Some(&m_byte) {
                            while j < line.len() && line.as_bytes()[j] == m_byte {
                                j += 1;
                            }
                            continue;
                        }
                        let close_ok = if marker == '_' {
                            next_char_is_non_alnum(line, j + 1)
                        } else {
                            next_char_allows_closing_star(line, j + 1)
                        };
                        let inner = &line[start..j];
                        if close_ok && is_valid_emphasis_inner(inner) {
                            return true;
                        }
                        break;
                    }
                    let ch_len = line[j..].chars().next().map_or(1, char::len_utf8);
                    j += ch_len;
                }
            }
        }
        let ch_len = line[i..].chars().next().map_or(1, char::len_utf8);
        i += ch_len;
    }
    false
}

fn has_markdown_link(line: &str) -> bool {
    let mut search_from = 0;
    while let Some(rel_open) = line[search_from..].find('[') {
        let open = search_from + rel_open;
        if open > 0 && line.as_bytes()[open - 1] == b'\\' {
            search_from = open + 1;
            continue;
        }
        if let Some(rel_close) = line[open + 1..].find("](") {
            let close_bracket = open + 1 + rel_close;
            let url_start = close_bracket + 2;
            if let Some(rel_paren) = line[url_start..].find(')') {
                let url = &line[url_start..url_start + rel_paren];
                if !url.is_empty() && !url.chars().any(char::is_whitespace) {
                    return true;
                }
            }
        }
        search_from = open + 1;
    }
    false
}

fn is_valid_emphasis_inner(inner: &str) -> bool {
    if inner.is_empty() {
        return false;
    }
    let first = inner.chars().next().unwrap();
    let last = inner.chars().next_back().unwrap();
    !first.is_whitespace() && !last.is_whitespace()
}

fn prev_char_is_non_alnum(s: &str, idx: usize) -> bool {
    match s[..idx].chars().next_back() {
        None => true,
        Some(c) => !c.is_alphanumeric(),
    }
}

fn next_char_is_non_alnum(s: &str, idx: usize) -> bool {
    match s[idx..].chars().next() {
        None => true,
        Some(c) => !c.is_alphanumeric(),
    }
}

fn prev_char_allows_opening_star(s: &str, idx: usize) -> bool {
    match s[..idx].chars().next_back() {
        None => true,
        Some(c) => c.is_whitespace() || matches!(c, '(' | '[' | '{' | '"' | '\'' | '/' | '—' | '-'),
    }
}

fn next_char_allows_closing_star(s: &str, idx: usize) -> bool {
    match s[idx..].chars().next() {
        None => true,
        Some(c) => c.is_whitespace() || c.is_ascii_punctuation(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_whitespace_are_plain() {
        assert_eq!(StyleAnalyzer::analyze(""), TextStyle::Plain);
        assert_eq!(StyleAnalyzer::analyze("   \n\t\n  "), TextStyle::Plain);
    }

    #[test]
    fn plain_prose_is_plain() {
        let prose = "This is a regular article paragraph.\n\nIt has multiple sentences, numbers like 42, and punctuation!";
        assert_eq!(StyleAnalyzer::analyze(prose), TextStyle::Plain);
    }

    #[test]
    fn hashtags_and_issue_numbers_are_plain() {
        assert_eq!(
            StyleAnalyzer::analyze("Trending #rust and issue #123 today."),
            TextStyle::Plain
        );
    }

    #[test]
    fn snake_case_and_urls_with_underscores_are_plain() {
        let text =
            "Check snake_case_identifier and https://example.com/a_b_c/my_page_name for details.";
        assert_eq!(StyleAnalyzer::analyze(text), TextStyle::Plain);
        assert_eq!(
            StyleAnalyzer::analyze("Also foo__bar__baz is not bold."),
            TextStyle::Plain
        );
    }

    #[test]
    fn math_expressions_with_asterisks_are_plain() {
        assert_eq!(
            StyleAnalyzer::analyze("Compute 2 * 3 * 4 = 24 in your head."),
            TextStyle::Plain
        );
    }

    #[test]
    fn status_banners_in_brackets_are_plain() {
        let banner = "[Fetching full text… — showing summary]\n\nRegular paragraph text.";
        assert_eq!(StyleAnalyzer::analyze(banner), TextStyle::Plain);
    }

    #[test]
    fn detects_atx_headings() {
        assert_eq!(StyleAnalyzer::analyze("# Title"), TextStyle::Markdown);
        assert_eq!(
            StyleAnalyzer::analyze("Intro\n\n### Subsection"),
            TextStyle::Markdown
        );
        assert_eq!(
            StyleAnalyzer::analyze("###### Level 6"),
            TextStyle::Markdown
        );
        assert_eq!(
            StyleAnalyzer::analyze("####### Too many hashes"),
            TextStyle::Plain
        );
    }

    #[test]
    fn detects_fenced_code_blocks() {
        let md = "Here is an example:\n```rust\nfn main() {}\n```";
        assert_eq!(StyleAnalyzer::analyze(md), TextStyle::Markdown);
        let tilde = "~~~\necho hello\n~~~";
        assert_eq!(StyleAnalyzer::analyze(tilde), TextStyle::Markdown);
    }

    #[test]
    fn detects_blockquotes() {
        assert_eq!(
            StyleAnalyzer::analyze("> A wise quote"),
            TextStyle::Markdown
        );
        assert_eq!(
            StyleAnalyzer::analyze(">> Nested quote"),
            TextStyle::Markdown
        );
    }

    #[test]
    fn detects_horizontal_rules() {
        assert_eq!(StyleAnalyzer::analyze("---"), TextStyle::Markdown);
        assert_eq!(StyleAnalyzer::analyze("***"), TextStyle::Markdown);
        assert_eq!(StyleAnalyzer::analyze("___"), TextStyle::Markdown);
        assert_eq!(StyleAnalyzer::analyze("- - -"), TextStyle::Markdown);
    }

    #[test]
    fn detects_lists() {
        assert_eq!(
            StyleAnalyzer::analyze("- First item\n- Second item"),
            TextStyle::Markdown
        );
        assert_eq!(StyleAnalyzer::analyze("* Bullet item"), TextStyle::Markdown);
        assert_eq!(StyleAnalyzer::analyze("+ Plus item"), TextStyle::Markdown);
        assert_eq!(
            StyleAnalyzer::analyze("1. Step one\n2. Step two"),
            TextStyle::Markdown
        );
    }

    #[test]
    fn detects_inline_formatting() {
        assert_eq!(
            StyleAnalyzer::analyze("Use `cargo test` now."),
            TextStyle::Markdown
        );
        assert_eq!(
            StyleAnalyzer::analyze("This is **bold** text."),
            TextStyle::Markdown
        );
        assert_eq!(
            StyleAnalyzer::analyze("This is __bold__ text."),
            TextStyle::Markdown
        );
        assert_eq!(
            StyleAnalyzer::analyze("This is ~~old~~ text."),
            TextStyle::Markdown
        );
        assert_eq!(
            StyleAnalyzer::analyze("This is *italic* text."),
            TextStyle::Markdown
        );
        assert_eq!(
            StyleAnalyzer::analyze("This is _italic_ text."),
            TextStyle::Markdown
        );
    }

    #[test]
    fn detects_links_and_images() {
        assert_eq!(
            StyleAnalyzer::analyze("Read [the docs](https://example.com) here."),
            TextStyle::Markdown
        );
        assert_eq!(
            StyleAnalyzer::analyze("Look ![diagram](https://example.com/pic.png)"),
            TextStyle::Markdown
        );
    }

    #[test]
    fn instance_detect_matches_analyze() {
        let analyzer = StyleAnalyzer::new();
        assert_eq!(analyzer.detect("# Heading"), TextStyle::Markdown);
        assert_eq!(analyzer.detect("Just plain text."), TextStyle::Plain);
    }
}

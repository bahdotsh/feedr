use ratatui::layout::Rect;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// Helper function to create a centered rect with minimum dimensions
pub(crate) fn centered_rect_with_min(
    percent_x: u16,
    percent_y: u16,
    min_w: u16,
    min_h: u16,
    r: Rect,
) -> Rect {
    let pct_w = r.width * percent_x / 100;
    let pct_h = r.height * percent_y / 100;
    let w = pct_w.max(min_w).min(r.width);
    let h = pct_h.max(min_h).min(r.height);
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

// Helper function to truncate a URL for display
pub(crate) fn truncate_url(url: &str, max_length: usize) -> String {
    // Remove common prefixes for cleaner display
    let clean_url = url
        .replace("https://", "")
        .replace("http://", "")
        .replace("www.", "");

    truncate_str(&clean_url, max_length)
}

// Helper function to truncate a string with unicode awareness
pub(crate) fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.width() <= max_chars {
        s.to_string()
    } else {
        // Find position to truncate while respecting unicode boundaries
        let mut total_width = 0;
        let mut truncate_idx = 0;

        for (idx, c) in s.char_indices() {
            let char_width = c.width_cjk().unwrap_or(1);
            if total_width + char_width > max_chars.saturating_sub(3) {
                truncate_idx = idx;
                break;
            }
            total_width += char_width;
        }

        if truncate_idx > 0 {
            format!("{}...", &s[..truncate_idx])
        } else {
            // Fallback if we couldn't properly calculate (shouldn't happen often)
            format!("{}...", &s[..max_chars.saturating_sub(3)])
        }
    }
}

fn is_atx_heading_line(trimmed: &str) -> bool {
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return false;
    }
    let rest = &trimmed[hashes..];
    matches!(rest.as_bytes().first(), Some(b' ' | b'\t')) && !rest.trim().is_empty()
}

fn is_horizontal_rule_line(trimmed: &str) -> bool {
    let mut marker: Option<char> = None;
    let mut count = 0usize;
    for ch in trimmed.chars() {
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

fn is_list_item_line(trimmed: &str) -> bool {
    if trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || trimmed.starts_with("• ")
    {
        return !trimmed[1..].trim().is_empty();
    }
    let digits = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
    if (1..=9).contains(&digits) {
        if let Some(after) = trimmed[digits..].strip_prefix(". ") {
            return !after.trim().is_empty();
        }
    }
    false
}

fn parse_blockquote_line(trimmed: &str) -> Option<(usize, &str)> {
    let after = trimmed.trim_start_matches('>');
    let depth = trimmed.len() - after.len();
    if depth == 0 {
        return None;
    }
    if after.is_empty() {
        Some((depth, ""))
    } else if let Some(rest) = after.strip_prefix(' ') {
        Some((depth, rest.trim()))
    } else {
        None
    }
}

fn fence_open_char(trimmed: &str) -> Option<char> {
    if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

fn is_fence_close(trimmed: &str, ch: char) -> bool {
    let count = trimmed.chars().take_while(|&c| c == ch).count();
    count >= 3 && trimmed[count..].trim().is_empty()
}

// Helper function to format content for better reading experience
pub(crate) fn format_content_for_reading(text: &str) -> String {
    let mut formatted_lines: Vec<String> = Vec::new();
    let mut current_paragraph: Vec<String> = Vec::new();
    let mut current_blockquote: Option<(usize, Vec<String>)> = None;
    let mut in_list = false;
    let mut in_fence: Option<char> = None;

    let flush_paragraph = |para: &mut Vec<String>, out: &mut Vec<String>| {
        if !para.is_empty() {
            out.push(para.join(" "));
            para.clear();
        }
    };

    let flush_blockquote = |bq: &mut Option<(usize, Vec<String>)>, out: &mut Vec<String>| {
        if let Some((depth, parts)) = bq.take() {
            if !parts.is_empty() {
                out.push(format!("{} {}", ">".repeat(depth), parts.join(" ")));
            }
        }
    };

    for line in text.lines() {
        let trimmed = line.trim();

        if let Some(fence_ch) = in_fence {
            formatted_lines.push(line.to_string());
            if is_fence_close(trimmed, fence_ch) {
                in_fence = None;
                formatted_lines.push(String::new());
            }
            continue;
        }

        if let Some(fence_ch) = fence_open_char(trimmed) {
            flush_paragraph(&mut current_paragraph, &mut formatted_lines);
            flush_blockquote(&mut current_blockquote, &mut formatted_lines);
            in_list = false;
            in_fence = Some(fence_ch);
            formatted_lines.push(trimmed.to_string());
            continue;
        }

        if trimmed.is_empty() {
            flush_paragraph(&mut current_paragraph, &mut formatted_lines);
            flush_blockquote(&mut current_blockquote, &mut formatted_lines);
            formatted_lines.push(String::new());
            in_list = false;
            continue;
        }

        if is_atx_heading_line(trimmed) || is_horizontal_rule_line(trimmed) {
            flush_paragraph(&mut current_paragraph, &mut formatted_lines);
            flush_blockquote(&mut current_blockquote, &mut formatted_lines);
            in_list = false;
            formatted_lines.push(trimmed.to_string());
            formatted_lines.push(String::new());
            continue;
        }

        if let Some((depth, bq_text)) = parse_blockquote_line(trimmed) {
            flush_paragraph(&mut current_paragraph, &mut formatted_lines);
            in_list = false;
            if bq_text.is_empty() {
                flush_blockquote(&mut current_blockquote, &mut formatted_lines);
                formatted_lines.push(String::new());
            } else {
                match &mut current_blockquote {
                    Some((cur_depth, parts)) if *cur_depth == depth => {
                        parts.push(bq_text.to_string());
                    }
                    _ => {
                        flush_blockquote(&mut current_blockquote, &mut formatted_lines);
                        current_blockquote = Some((depth, vec![bq_text.to_string()]));
                    }
                }
            }
            continue;
        }

        flush_blockquote(&mut current_blockquote, &mut formatted_lines);

        if is_list_item_line(trimmed) {
            flush_paragraph(&mut current_paragraph, &mut formatted_lines);
            formatted_lines.push(line.trim_end().to_string());
            in_list = true;
        } else if in_list && line.starts_with("  ") {
            // Append indented continuation to the current list item
            if let Some(last) = formatted_lines.last_mut() {
                last.push(' ');
                last.push_str(trimmed);
            }
        } else {
            if in_list {
                formatted_lines.push(String::new());
                in_list = false;
            }
            current_paragraph.push(trimmed.to_string());
        }
    }

    flush_paragraph(&mut current_paragraph, &mut formatted_lines);
    flush_blockquote(&mut current_blockquote, &mut formatted_lines);

    // Clean up excessive empty lines (outside fenced code blocks, max 1 empty line in a row)
    let mut result = Vec::new();
    let mut empty_count = 0usize;
    let mut cleanup_fence: Option<char> = None;

    for line in formatted_lines {
        if let Some(fence_ch) = cleanup_fence {
            let closes = is_fence_close(line.trim(), fence_ch);
            result.push(line);
            if closes {
                cleanup_fence = None;
                empty_count = 0;
            }
            continue;
        }
        if let Some(fence_ch) = fence_open_char(line.trim()) {
            empty_count = 0;
            cleanup_fence = Some(fence_ch);
            result.push(line);
            continue;
        }

        if line.is_empty() {
            empty_count += 1;
            if empty_count <= 1 && !result.is_empty() {
                result.push(line);
            }
        } else {
            empty_count = 0;
            result.push(line);
        }
    }

    while result.last().is_some_and(|l| l.is_empty()) {
        result.pop();
    }

    result.join("\n")
}

// Helper function to count the number of lines when text is wrapped
pub(crate) fn count_wrapped_lines(text: &str, width: usize) -> u16 {
    if width == 0 {
        return 0;
    }

    let mut line_count = 0u16;

    for line in text.lines() {
        if line.is_empty() {
            // Empty lines still count as one line
            line_count = line_count.saturating_add(1);
        } else {
            // Calculate how many wrapped lines this line will take
            let line_width = line.width();
            if line_width == 0 {
                line_count = line_count.saturating_add(1);
            } else {
                let wrapped_lines = line_width.div_ceil(width).max(1);
                line_count = line_count.saturating_add(wrapped_lines as u16);
            }
        }
    }

    // If text doesn't end with newline, we still have the lines we counted
    // If text is empty, return at least 1 line
    line_count.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_content_preserves_markdown_blocks_and_fences() {
        let input = "# Heading\nParagraph one\ncontinues here.\n\n*italic* word\n\n- item 1\n  continued\n10. tenth\n\n> quote one\n> quote two\n>\n> second quote\n\n```rust\n  let a = 1;\n\n  let b = 2;\n```";
        let out = format_content_for_reading(input);
        assert!(out.contains("# Heading\n\nParagraph one continues here."));
        assert!(out.contains("*italic* word"));
        assert!(out.contains("- item 1 continued\n10. tenth"));
        assert!(out.contains("> quote one quote two\n\n> second quote"));
        assert!(out.contains("```rust\n  let a = 1;\n\n  let b = 2;\n```"));
    }
}

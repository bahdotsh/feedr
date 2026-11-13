use crate::app::{find_article_matches, App, ExtractionState, InputMode};
use crate::keybindings::{key_display, KeyAction};
use crate::ui::renderer::{highlight_lines, lines_to_plain_text};
use crate::ui::style_analyzer::StyleAnalyzer;
use crate::ui::utils::{count_wrapped_lines, format_content_for_reading, truncate_url};
use crate::ui::ColorScheme;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap},
    Frame,
};

/// Which body the detail view is currently showing. Owns its strings so
/// the immutable borrow on `app.extracted` is released before we call any
/// `&mut App` method later in the render (e.g. `update_detail_max_scroll`).
enum BodySource {
    Summary,
    Extracted(String),
    Extracting,
    /// Failed extraction — body falls back to the summary, and `reason` is
    /// shown as a small dim hint above it.
    Failed(String),
}

pub(super) fn render_item_detail<B: Backend>(
    f: &mut Frame<B>,
    app: &mut App,
    area: Rect,
    colors: &ColorScheme,
) {
    // Reset the `show_extracted` toggle on article focus change. Done at
    // the top so we hold no immutable borrow on `app` (the `current_item`
    // call below would conflict). Logic lives on `App` so it's
    // unit-testable without spinning up a TestBackend.
    app.sync_detail_focus_anchor();
    // The search footer is visible whenever the user is actively typing a
    // query OR a committed query has highlights still pinned. Computing
    // this once up front lets the layout split, footer render, and title
    // suffix all agree on visibility.
    let search_footer_visible =
        matches!(app.input_mode, InputMode::ArticleSearch) || !app.article_search_query.is_empty();
    // ── Image strip setup (hoisted above the `current_item` borrow) ─────
    // We extract the thumbnail URL with a short-lived immutable borrow,
    // then release it so we can mutate `app.image_cache` and write to
    // `app.pending_image_render` below. The actual layout decision needs
    // the URL string, which we own here.
    let image_url: Option<String> = app.current_item().and_then(|i| i.thumbnail.clone());
    // 10 rows is a reasonable thumbnail size on a typical terminal: tall
    // enough to show the image content, short enough to leave room for the
    // body below.
    const IMAGE_STRIP_ROWS: u16 = 10;
    // Minimum detail-area height to reserve the strip. The header is a
    // fixed 9 rows and the strip 10, both higher layout priority than the
    // `Min(0)` body — on anything shorter the body (which loses another 6
    // rows to borders/padding) collapses to near-zero and the article
    // becomes unreadable. 30 leaves at least ~5 visible text lines.
    const MIN_AREA_HEIGHT_FOR_IMAGE: u16 = 30;
    let image_strip_rows: u16 = match &image_url {
        Some(url)
            if !app.compact
                && area.height >= MIN_AREA_HEIGHT_FOR_IMAGE
                && app.image_cache.has_image(url)
                && app.image_cache.protocol() == crate::image::ImageProtocol::Kitty =>
        {
            IMAGE_STRIP_ROWS
        }
        _ => 0,
    };
    // Queue a background fetch on first display of an item with a
    // thumbnail. `start_fetch` is idempotent for already-attempted URLs,
    // so re-rendering the same detail view is cheap.
    if let Some(url) = &image_url {
        app.image_cache.start_fetch(url);
    }
    // Split the area into header, optional image strip, content, and
    // (optionally) a 1-row search footer. The footer is omitted when
    // in-article search is inactive so quiet reading is unchanged. The
    // split happens before the `item` borrow below so the *actual* strip
    // rect can be stored in `pending_image_render`.
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(9)]; // Header
    if image_strip_rows > 0 {
        constraints.push(Constraint::Length(image_strip_rows));
    }
    constraints.push(Constraint::Min(0)); // Content
    if search_footer_visible {
        constraints.push(Constraint::Length(1));
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    // Resolve chunk indices: header is always [0]; image strip (if
    // present) is [1] and shifts content/footer down by one.
    let content_chunk_idx: usize = if image_strip_rows > 0 { 2 } else { 1 };
    let footer_chunk_idx: Option<usize> = if search_footer_visible {
        Some(if image_strip_rows > 0 { 3 } else { 2 })
    } else {
        None
    };

    // Compute the pending image rect from the strip chunk the layout
    // solver actually produced — on short terminals it can be squeezed
    // below the requested rows, and the image must not spill past it
    // onto the content or help bar. The image is horizontally centered
    // within the strip. None when no strip is shown or it collapsed.
    app.pending_image_render = match (&image_url, image_strip_rows > 0) {
        (Some(url), true) => {
            let strip = chunks[1];
            if strip.width == 0 || strip.height == 0 {
                None
            } else {
                app.image_cache
                    .display_cells(url, strip.width as usize, strip.height as usize)
                    .map(|(cols, rows)| crate::app::PendingImage {
                        url: url.clone(),
                        col: strip.x + strip.width.saturating_sub(cols) / 2,
                        row: strip.y,
                        cols,
                        rows,
                    })
            }
        }
        _ => None,
    };

    if let Some(item) = app.current_item() {
        // Create header with enhanced typography
        let mut header_lines = vec![
            // Title with better emphasis
            Line::from(vec![Span::styled(
                &item.title,
                Style::default()
                    .fg(colors.text)
                    .add_modifier(Modifier::BOLD),
            )]),
            // Add spacing after title
            Line::from(""),
        ];

        // Build metadata line with enhanced formatting
        let mut metadata_parts = Vec::new();

        // Add read status with icon
        if let (Some(feed_idx), Some(item_idx)) = (app.selected_feed, app.selected_item) {
            let is_read = app.is_item_read(feed_idx, item_idx);
            metadata_parts.push(Span::styled(
                if is_read { "✓ Read" } else { "○ Unread" },
                Style::default().fg(if is_read {
                    colors.success
                } else {
                    colors.highlight
                }),
            ));
        }

        // Add author with emphasis
        if let Some(author) = &item.author {
            if !metadata_parts.is_empty() {
                metadata_parts.push(Span::styled(" · ", Style::default().fg(colors.muted)));
            }
            metadata_parts.push(Span::styled(
                author,
                Style::default()
                    .fg(colors.secondary)
                    .add_modifier(Modifier::ITALIC),
            ));
        }

        // Add date
        if let Some(date) = &item.formatted_date {
            if !metadata_parts.is_empty() {
                metadata_parts.push(Span::styled(" · ", Style::default().fg(colors.muted)));
            }
            metadata_parts.push(Span::styled(
                date,
                Style::default().fg(colors.text_secondary),
            ));
        }

        if !metadata_parts.is_empty() {
            header_lines.push(Line::from(metadata_parts));
        }

        // Add subtle separator before link
        header_lines.push(Line::from(""));

        // Add link if available with better styling
        if let Some(link) = &item.link {
            header_lines.push(Line::from(vec![
                Span::styled("🔗 ", Style::default().fg(colors.muted)),
                Span::styled(
                    truncate_url(link, 70),
                    Style::default()
                        .fg(colors.primary)
                        .add_modifier(Modifier::UNDERLINED),
                ),
            ]));
        }

        let article_icon = colors.get_icon_article();
        let header = Paragraph::new(header_lines)
            .block(
                Block::default()
                    .title(format!(" {} Article ", article_icon))
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(colors.border_normal)
                    .border_style(Style::default().fg(colors.border))
                    .style(Style::default().bg(colors.surface))
                    .padding(Padding::new(3, 3, 1, 1)), // Increased horizontal padding
            )
            .style(Style::default().fg(colors.text))
            .alignment(Alignment::Left);

        f.render_widget(header, chunks[0]);
        // The image strip (chunks[1], if present) is intentionally NOT
        // rendered into — the cells stay blank so the Kitty escapes
        // emitted after `terminal.draw()` returns paint the image on
        // top, and ratatui's diff sees no changes there on subsequent
        // frames (image persists). The target rect was already stored in
        // `app.pending_image_render` before the item-borrow began.

        // Decide which body to render: the original summary, the extracted
        // full-text, or a placeholder for in-flight / failed extractions.
        // Uses `current_article_indices()` (rather than the raw selection
        // fields) so the lookup matches the resolution logic that
        // `toggle_or_request_fulltext` already uses — keeps the two sides
        // in lockstep if detail rendering ever leaks out of FeedItemDetail.
        let body_source: BodySource = if app.show_extracted {
            match app
                .current_article_indices()
                .and_then(|(fi, ii)| app.extraction_state_for(fi, ii))
            {
                Some(ExtractionState::Ready(article)) => {
                    BodySource::Extracted(article.plain_text.clone())
                }
                Some(ExtractionState::Pending { .. }) => BodySource::Extracting,
                Some(ExtractionState::Failed(reason)) => BodySource::Failed(reason.clone()),
                None => BodySource::Summary,
            }
        } else {
            BodySource::Summary
        };

        // Computed once and shared by the Summary / Failed branches below.
        // Lifted out of the match so the description-formatting logic isn't
        // duplicated when extraction fails and we fall back to the summary.
        let summary_text = || -> String {
            if let Some(desc) = &item.description {
                let raw_text = crate::feed::render_clean_html(desc, 100);
                format_content_for_reading(&raw_text)
            } else {
                "No description available".to_string()
            }
        };

        // The retry / toggle hints embed the *current* binding for
        // FetchFullText so they stay correct if the user remaps it.
        let fulltext_key = key_display(&KeyAction::FetchFullText, &app.keybindings);

        let description = match &body_source {
            BodySource::Summary => summary_text(),
            BodySource::Failed(reason) => {
                // Surface the failure reason at the top, then fall back to
                // the original summary so the user still has something to
                // read. Press the FetchFullText key again to retry.
                format!(
                    "[Full-text extraction failed: {}]\n[Press {} to retry — showing summary]\n\n{}",
                    reason,
                    fulltext_key,
                    summary_text()
                )
            }
            BodySource::Extracted(text) => format_content_for_reading(text),
            BodySource::Extracting => {
                // Keep the summary visible while the worker is in flight —
                // a slow site can take several seconds, and replacing the
                // body with "Fetching…" leaves the user with nothing to
                // read AND no way to revert (the FetchFullText key is a
                // no-op on Pending). The scroll-indicator title (see
                // body_label below) carries the "Extracting…" status.
                format!(
                    "[Fetching full text… — showing summary]\n\n{}",
                    summary_text()
                )
            }
        };

        // Calculate the viewport height (accounting for borders and padding)
        let viewport_height = chunks[content_chunk_idx]
            .height
            .saturating_sub(2) // borders (top and bottom)
            .saturating_sub(4); // increased padding (top and bottom)

        // Calculate the content width (accounting for borders and padding)
        let content_width = chunks[content_chunk_idx]
            .width
            .saturating_sub(2) // borders (left and right)
            .saturating_sub(8) // increased padding for better reading width
            as usize;

        // Detect the body's style and render into styled lines first so
        // line-count / scroll calculations and search match offsets align
        // with what is actually displayed (e.g. stripped `#` / `**` / fences).
        let text_style = StyleAnalyzer::analyze(&description);
        let rendered_lines = text_style.renderer().render(&description, colors);
        let rendered_text = lines_to_plain_text(&rendered_lines);

        // Calculate the number of lines the wrapped content will take
        let content_lines = count_wrapped_lines(&rendered_text, content_width);

        // Update the max scroll value
        app.update_detail_max_scroll(content_lines, viewport_height);
        app.clamp_detail_scroll();

        // Search work is skipped entirely when the user isn't typing a
        // query and has no committed query pinned — keeps the steady-state
        // detail view allocation-free. Without this gate the renderer
        // would clone the full body string into `article_body_cache` every
        // frame (default tick rate 100ms ⇒ ~500KB/s churn on a 50KB
        // full-text article) just to back a feature that wasn't engaged.
        let search_active = search_footer_visible;

        let (matches, current_idx) = if search_active {
            // Cache the rendered body and content width so `n` / `N`
            // (handled in event dispatch) can resolve a match line back to
            // a wrapped-row scroll target without re-deriving the body
            // source. Re-derivation here is the source of truth — these
            // writes happen once per frame the search is live.
            app.article_body_cache.clear();
            app.article_body_cache.push_str(&rendered_text);
            app.article_body_width_cache = content_width;

            // Recompute matches against the freshly rendered body. Clamp
            // the current-match cursor so it stays valid as the query
            // shrinks or the body changes (e.g. after `Shift+F` toggles
            // full-text). The clamp lands the cursor on *some* match, not
            // necessarily the "same" one — the i-th match before a body
            // change is rarely the i-th match after — but landing on a
            // valid match beats resetting to 0 every keystroke.
            let matches = find_article_matches(&rendered_text, &app.article_search_query);
            if matches.is_empty() {
                app.article_search_current = None;
            } else {
                let cur = app
                    .article_search_current
                    .unwrap_or(0)
                    .min(matches.len() - 1);
                app.article_search_current = Some(cur);
            }
            let current_idx = app.article_search_current;
            app.article_search_matches = matches.clone();
            (matches, current_idx)
        } else {
            // Search inactive: ensure stale state can't satisfy a future
            // `n`/`N` press, then skip the work.
            app.article_search_matches.clear();
            app.article_search_current = None;
            (Vec::new(), None)
        };

        // Create theme-specific scroll indicator
        let scroll_arrows = if colors.border_normal == BorderType::Double {
            ("▼", "▲") // Dark: solid arrows
        } else {
            ("↓", "↑") // Light: simple arrows
        };

        let body_label = match &body_source {
            BodySource::Summary => "Summary",
            BodySource::Extracted(_) => "Full-text",
            BodySource::Extracting => "Extracting…",
            BodySource::Failed(_) => "Full-text failed",
        };

        let match_suffix = if app.article_search_query.is_empty() {
            String::new()
        } else if matches.is_empty() {
            " · no matches".to_string()
        } else {
            format!(
                " · {}/{} matches",
                current_idx.map(|i| i + 1).unwrap_or(0),
                matches.len()
            )
        };

        let scroll_indicator = if app.detail_max_scroll > 0 {
            let scroll_pct =
                (app.detail_vertical_scroll as f32 / app.detail_max_scroll as f32 * 100.0) as u16;
            if app.detail_vertical_scroll == 0 {
                format!(
                    " {} {} · Scroll {} for more{} ",
                    article_icon, body_label, scroll_arrows.0, match_suffix
                )
            } else if app.detail_vertical_scroll >= app.detail_max_scroll {
                format!(
                    " {} {} · End of article{} ",
                    article_icon, body_label, match_suffix
                )
            } else {
                format!(
                    " {} {} · {}%{} ",
                    article_icon, body_label, scroll_pct, match_suffix
                )
            }
        } else {
            format!(" {} {}{} ", article_icon, body_label, match_suffix)
        };

        // Overlay search highlights onto the styled body lines.
        let body_lines = highlight_lines(rendered_lines, &matches, current_idx, colors);

        // Create content paragraph with theme-specific styling
        let content = Paragraph::new(body_lines)
            .block(
                Block::default()
                    .title(scroll_indicator)
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(colors.border_normal)
                    .border_style(Style::default().fg(colors.border))
                    .style(Style::default().bg(colors.surface))
                    .padding(Padding::new(4, 4, 2, 2)), // Generous padding for reading comfort
            )
            .style(Style::default().fg(colors.text))
            .scroll((app.detail_vertical_scroll, 0))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left);

        f.render_widget(content, chunks[content_chunk_idx]);

        if let Some(idx) = footer_chunk_idx {
            render_search_footer(f, app, chunks[idx], colors);
        }
    }
}

/// Render the 1-row search footer at the bottom of the detail view.
/// Shows `/<query>` and (when not actively typing) the match indicator.
/// Sets the terminal cursor on the line when in `ArticleSearch` input
/// mode so the user sees where input is going.
fn render_search_footer<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect, colors: &ColorScheme) {
    let typing = matches!(app.input_mode, InputMode::ArticleSearch);
    let match_count = app.article_search_matches.len();
    let current = app.article_search_current;

    let mut spans = vec![
        Span::styled(
            "/",
            Style::default()
                .fg(colors.highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            app.article_search_query.clone(),
            Style::default().fg(colors.text),
        ),
    ];

    if !typing {
        let status = if app.article_search_query.is_empty() {
            String::new()
        } else if match_count == 0 {
            "  no matches".to_string()
        } else {
            format!(
                "  {}/{} (n/N to jump · ESC to clear)",
                current.map(|i| i + 1).unwrap_or(0),
                match_count
            )
        };
        if !status.is_empty() {
            spans.push(Span::styled(status, Style::default().fg(colors.muted)));
        }
    }

    let footer = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(colors.surface))
        .alignment(Alignment::Left);
    f.render_widget(footer, area);

    if typing {
        // Cursor sits just past the rendered query: 1 column for "/" plus
        // the display width of the query. Clamp to the visible row width.
        let q_width =
            unicode_width::UnicodeWidthStr::width(app.article_search_query.as_str()) as u16;
        let cursor_x = area.x + 1 + q_width;
        f.set_cursor(cursor_x.min(area.x + area.width.saturating_sub(1)), area.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{find_article_matches, ArticleMatch};

    /// Build the body as styled `Line`s using [`StyleAnalyzer`] and the matching
    /// [`Renderer`](crate::ui::renderer::Renderer), then apply highlight spans for
    /// each search match.
    fn build_styled_body<'a>(
        body: &'a str,
        matches: &[ArticleMatch],
        current_idx: Option<usize>,
        colors: &ColorScheme,
    ) -> Vec<Line<'a>> {
        let style = StyleAnalyzer::analyze(body);
        let rendered = style.renderer().render(body, colors);
        highlight_lines(rendered, matches, current_idx, colors)
    }

    /// Extract the printable text of a `Line` so tests can assert content
    /// independent of span styling. Joins all span text in order.
    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn build_styled_body_no_matches_returns_one_raw_line_per_source_line() {
        let colors = ColorScheme::dark();
        let body = "alpha\nbeta\ngamma";
        let lines = build_styled_body(body, &[], None, &colors);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].spans.len(), 1);
        assert_eq!(line_text(&lines[0]), "alpha");
        assert_eq!(line_text(&lines[1]), "beta");
        assert_eq!(line_text(&lines[2]), "gamma");
    }

    #[test]
    fn build_styled_body_highlights_match_and_preserves_surrounding_text() {
        let colors = ColorScheme::dark();
        let body = "before foo after";
        let matches = find_article_matches(body, "foo");
        assert_eq!(matches.len(), 1);

        let lines = build_styled_body(body, &matches, Some(0), &colors);
        assert_eq!(lines.len(), 1);

        // Expected span sequence: "before ", "foo" (styled), " after".
        let spans = &lines[0].spans;
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content.as_ref(), "before ");
        assert_eq!(spans[1].content.as_ref(), "foo");
        assert_eq!(spans[2].content.as_ref(), " after");

        // The "current" match must use the filled-background style, not
        // the foreground-only style — a regression here would mean `n`/`N`
        // gives no visual signal of which match is focused.
        let current_style = Style::default()
            .bg(colors.highlight)
            .fg(colors.surface)
            .add_modifier(Modifier::BOLD);
        assert_eq!(spans[1].style, current_style);
    }

    #[test]
    fn build_styled_body_non_current_match_uses_quiet_style() {
        let colors = ColorScheme::dark();
        let body = "foo and foo";
        let matches = find_article_matches(body, "foo");
        assert_eq!(matches.len(), 2);

        // Cursor on match 0; match 1 must render with the quiet style.
        let lines = build_styled_body(body, &matches, Some(0), &colors);
        let spans = &lines[0].spans;
        // Expect: "foo" (current), " and ", "foo" (quiet)
        assert_eq!(spans.len(), 3);
        let current_style = Style::default()
            .bg(colors.highlight)
            .fg(colors.surface)
            .add_modifier(Modifier::BOLD);
        let quiet_style = Style::default().fg(colors.highlight);
        assert_eq!(spans[0].style, current_style);
        assert_eq!(spans[2].style, quiet_style);
    }

    #[test]
    fn build_styled_body_handles_matches_across_multiple_lines() {
        let colors = ColorScheme::dark();
        let body = "first foo line\nsecond foo line";
        let matches = find_article_matches(body, "foo");
        assert_eq!(matches.len(), 2);

        let lines = build_styled_body(body, &matches, Some(1), &colors);
        assert_eq!(lines.len(), 2);
        // Each line has 3 spans: prefix raw, match, suffix raw.
        assert_eq!(lines[0].spans.len(), 3);
        assert_eq!(lines[1].spans.len(), 3);
        // The "current" highlight must be on line 1 (idx 1), not line 0.
        let current_style = Style::default()
            .bg(colors.highlight)
            .fg(colors.surface)
            .add_modifier(Modifier::BOLD);
        assert_eq!(lines[0].spans[1].style.bg, None);
        assert_eq!(lines[1].spans[1].style, current_style);
    }

    #[test]
    fn build_styled_body_match_at_line_start_emits_no_empty_prefix_span() {
        let colors = ColorScheme::dark();
        let body = "foo tail";
        let matches = find_article_matches(body, "foo");
        let lines = build_styled_body(body, &matches, Some(0), &colors);
        let spans = &lines[0].spans;
        // Two spans: styled "foo", raw " tail" — no leading empty Span::raw.
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content.as_ref(), "foo");
        assert_eq!(spans[1].content.as_ref(), " tail");
    }

    #[test]
    fn build_styled_body_match_at_line_end_emits_no_trailing_empty_span() {
        let colors = ColorScheme::dark();
        let body = "head foo";
        let matches = find_article_matches(body, "foo");
        let lines = build_styled_body(body, &matches, Some(0), &colors);
        let spans = &lines[0].spans;
        // Two spans: raw "head ", styled "foo" — no trailing empty Span::raw.
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content.as_ref(), "head ");
        assert_eq!(spans[1].content.as_ref(), "foo");
    }

    #[test]
    fn build_styled_body_renders_markdown_and_highlights_rendered_text() {
        let colors = ColorScheme::dark();
        let body = "# Heading foo\n\nParagraph with **foo** inside.";
        let rendered_only = build_styled_body(body, &[], None, &colors);
        assert_eq!(line_text(&rendered_only[0]), "Heading foo");
        assert_eq!(line_text(&rendered_only[2]), "Paragraph with foo inside.");

        let visible = lines_to_plain_text(&rendered_only);
        let matches = find_article_matches(&visible, "foo");
        assert_eq!(matches.len(), 2);

        let highlighted = build_styled_body(body, &matches, Some(1), &colors);
        let current_style = Style::default()
            .bg(colors.highlight)
            .fg(colors.surface)
            .add_modifier(Modifier::BOLD);
        let quiet_style = Style::default().fg(colors.highlight);

        assert!(highlighted[0]
            .spans
            .iter()
            .any(|s| s.content.as_ref() == "foo" && s.style == quiet_style));
        assert!(highlighted[2]
            .spans
            .iter()
            .any(|s| s.content.as_ref() == "foo" && s.style == current_style));
    }
}

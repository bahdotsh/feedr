use crate::app::App;
use crate::ui::utils::{count_wrapped_lines, format_content_for_reading};
use crate::ui::ColorScheme;
use html2text::from_read;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

pub(super) fn render_dashboard<B: Backend>(
    f: &mut Frame<B>,
    app: &mut App,
    area: Rect,
    colors: &ColorScheme,
) {
    let search_icon = colors.get_icon_search();
    let mut title = if app.is_searching {
        let result_count = app.active_dashboard_items().len();
        format!(
            " {} Search: '{}' \u{2014} {} results across all feeds ",
            search_icon, app.search_query, result_count
        )
    } else {
        format!(" {} Latest Updates ", colors.get_icon_dashboard())
    };

    // Add filter indicators to title if any filters are active
    if app.filter_options.is_active() {
        title = format!("{} | {} Filtered", title, search_icon);
    }

    // Determine which item list to use — borrow as a slice to avoid cloning
    let items_to_display: &[(usize, usize)] = app.active_dashboard_items();
    // Copy the indices we need for the preview pane before borrowing app mutably
    let preview_indices: Vec<(usize, usize)> = if app.preview_pane {
        items_to_display.to_vec()
    } else {
        Vec::new()
    };

    if items_to_display.is_empty() {
        let message = if app.is_searching {
            let no_results = format!("No results found for '{}'", app.search_query);

            // Create a visually appealing empty search results screen
            let lines = [
                "",
                "       🔍       ",
                "",
                &no_results,
                "",
                "Try different keywords or add more feeds",
            ];

            lines.join("\n")
        } else if app.feeds.is_empty() {
            // Theme-specific ASCII art
            let ascii_art = colors.get_dashboard_art();
            ascii_art.join("\n")
        } else {
            let empty_msg = [
                "",
                "       📭       ",
                "",
                "No recent items",
                "",
                "Refresh with 'r' to update",
                "",
            ];
            empty_msg.join("\n")
        };

        // Rich text for empty dashboard with theme-specific styling
        let mut text = Text::default();

        if app.feeds.is_empty() && !app.is_searching {
            // For welcome screen with theme-specific styling
            for line in message.lines() {
                // Dark theme patterns
                if line.contains("███")
                    || line.contains("╔")
                    || line.contains("╗")
                    || line.contains("║")
                    || line.contains("╚")
                    || line.contains("╝")
                {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(colors.primary),
                    )]));
                } else if line.contains("◢◣") || line.contains("◤◥") || line.contains("▸")
                {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default()
                            .fg(colors.highlight)
                            .add_modifier(Modifier::BOLD),
                    )]));
                } else if line.contains("NEURAL")
                    || line.contains("INTERFACE")
                    || line.contains("CYBER")
                {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default()
                            .fg(colors.accent)
                            .add_modifier(Modifier::BOLD),
                    )]));
                } else if line.contains("═══") || line.contains("───") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(colors.border),
                    )]));
                } else if line.contains("INITIALIZE")
                    || line.contains("CONNECT")
                    || line.contains("NO_SIGNAL")
                    || line.contains("INIT_FEED")
                {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(colors.secondary),
                    )]));
                // Light theme patterns
                } else if line.contains("F  e  e  d  r") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default()
                            .fg(colors.primary)
                            .add_modifier(Modifier::BOLD),
                    )]));
                } else if line.contains("mindful") || line.contains("Begin") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(colors.text),
                    )]));
                } else if line.contains("Press 'a'") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(colors.highlight),
                    )]));
                } else if line.contains("🍃") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(colors.success),
                    )]));
                } else if line.contains("_")
                    || line.contains("( )")
                    || line.contains("|")
                    || line.contains("/ \\")
                    || line.contains("/   \\")
                    || line.contains("-------")
                {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(colors.secondary),
                    )]));
                } else {
                    text.lines.push(Line::from(line));
                }
            }
        } else {
            // For empty search or empty dashboard
            for line in message.lines() {
                if line.contains("🔍") || line.contains("📭") || line.contains(search_icon) {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(colors.secondary),
                    )]));
                } else if line.contains("No results") || line.contains("No recent") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default()
                            .fg(colors.text)
                            .add_modifier(Modifier::BOLD),
                    )]));
                } else {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(colors.muted),
                    )]));
                }
            }
        }

        let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(colors.border_normal)
                .border_style(Style::default().fg(colors.border))
                .style(Style::default().bg(colors.surface))
                .padding(Padding::new(2, 2, 2, 2)),
        );

        f.render_widget(paragraph, area);
        return;
    }

    if app.filter_options.is_active() && items_to_display.is_empty() {
        let mut text = Text::default();

        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            format!("       {}       ", search_icon),
            Style::default().fg(colors.secondary),
        )));
        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            "No items match your current filters",
            Style::default()
                .fg(colors.text)
                .add_modifier(Modifier::BOLD),
        )));
        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            app.get_filter_summary(),
            Style::default().fg(colors.secondary),
        )));
        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            "Press 'f' to adjust filters or 'r' to refresh feeds",
            Style::default().fg(colors.highlight),
        )));

        let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(colors.border_normal)
                .border_style(Style::default().fg(colors.border))
                .style(Style::default().bg(colors.surface))
                .padding(Padding::new(2, 2, 2, 2)),
        );

        f.render_widget(paragraph, area);
        return;
    }

    // For non-empty dashboard, create richly formatted items with theme-specific styling
    let arrow = colors.get_arrow_right();
    let success_icon = colors.get_icon_success();
    let is_compact = app.compact;
    let items: Vec<ListItem> = items_to_display
        .iter()
        .enumerate()
        .map(|(idx, &(feed_idx, item_idx))| {
            let (feed, item) = app.active_dashboard_item(idx).unwrap();

            let date_str = item.formatted_date.as_deref().unwrap_or("Unknown date");
            let is_selected = app.selected_item == Some(idx);
            let is_read = app.is_item_read(feed_idx, item_idx);
            let is_starred = app.is_item_starred(feed_idx, item_idx);

            if is_compact {
                // Compact: single line per item
                ListItem::new(Line::from(vec![
                    Span::styled(
                        if is_selected {
                            format!("{} ", arrow)
                        } else {
                            "  ".to_string()
                        },
                        Style::default().fg(colors.highlight),
                    ),
                    Span::styled(
                        format!("{} | ", feed.title),
                        Style::default()
                            .fg(colors.text_secondary)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        &item.title,
                        Style::default().fg(if is_read { colors.muted } else { colors.text }),
                    ),
                    Span::styled(
                        if is_starred { " \u{2605}" } else { "" },
                        Style::default().fg(Color::Rgb(255, 215, 0)),
                    ),
                    Span::styled(format!("  {}", date_str), Style::default().fg(colors.muted)),
                ]))
                .style(Style::default().fg(colors.text).bg(if is_selected {
                    colors.selected_bg
                } else {
                    colors.background
                }))
            } else {
                // Create clearer visual group with theme-specific hierarchy
                ListItem::new(vec![
                    // Feed source with theme-specific indicator
                    Line::from(vec![
                        Span::styled(
                            if is_selected {
                                format!("{} ", arrow)
                            } else {
                                "  ".to_string()
                            },
                            Style::default().fg(colors.highlight),
                        ),
                        Span::styled(
                            feed.title.to_string(),
                            Style::default()
                                .fg(if is_selected {
                                    colors.secondary
                                } else {
                                    colors.text_secondary
                                })
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            if is_starred { " \u{2605}" } else { "" },
                            Style::default().fg(Color::Rgb(255, 215, 0)),
                        ),
                        Span::styled(
                            if is_read {
                                format!(" {}", success_icon)
                            } else {
                                "".to_string()
                            },
                            Style::default().fg(colors.success),
                        ),
                    ]),
                    // Item title - cleaner layout
                    Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            &item.title,
                            Style::default()
                                .fg(if is_selected {
                                    colors.text
                                } else if is_read {
                                    colors.text_secondary
                                } else {
                                    colors.text
                                })
                                .add_modifier(if is_selected {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ),
                    ]),
                    // Publication date with subtle styling
                    Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(date_str, Style::default().fg(colors.muted)),
                    ]),
                    // Spacing between items
                    Line::from(""),
                ])
                .style(Style::default().fg(colors.text).bg(if is_selected {
                    colors.selected_bg
                } else {
                    colors.background
                }))
            }
        })
        .collect();

    // Split area for preview pane if active (disabled in compact mode)
    let (list_area, preview_area) = if app.preview_pane && !app.compact {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let dashboard_list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(colors.border_normal)
                .border_style(Style::default().fg(colors.border))
                .style(Style::default().bg(colors.surface))
                .padding(Padding::new(2, 1, 1, 1)),
        )
        .highlight_style(
            Style::default()
                .bg(colors.selected_bg)
                .fg(colors.highlight)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("");

    let mut state = ratatui::widgets::ListState::default();
    state.select(app.selected_item);

    f.render_stateful_widget(dashboard_list, list_area, &mut state);

    // Render preview pane
    if let Some(preview_area) = preview_area {
        render_preview_pane(f, app, preview_area, &preview_indices, colors);
    }
}

fn render_preview_pane<B: Backend>(
    f: &mut Frame<B>,
    app: &mut App,
    area: Rect,
    items_to_display: &[(usize, usize)],
    colors: &ColorScheme,
) {
    let selected = app.selected_item.unwrap_or(0);
    let item_data = items_to_display
        .get(selected)
        .and_then(|&(feed_idx, item_idx)| {
            app.feeds
                .get(feed_idx)
                .and_then(|feed| feed.items.get(item_idx).map(|item| (feed, item)))
        });

    let article_icon = colors.get_icon_article();

    let Some((feed, item)) = item_data else {
        let empty = Paragraph::new("No item selected")
            .block(
                Block::default()
                    .title(format!(" {} Preview ", article_icon))
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(colors.border_normal)
                    .border_style(Style::default().fg(colors.border))
                    .style(Style::default().bg(colors.surface))
                    .padding(Padding::new(2, 2, 1, 1)),
            )
            .style(Style::default().fg(colors.muted))
            .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    };

    // Build preview content
    let mut lines: Vec<Line> = Vec::new();

    // Title
    lines.push(Line::from(vec![Span::styled(
        &item.title,
        Style::default()
            .fg(colors.text)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(""));

    // Feed source
    lines.push(Line::from(vec![Span::styled(
        &feed.title,
        Style::default()
            .fg(colors.secondary)
            .add_modifier(Modifier::BOLD),
    )]));

    // Author and date
    let mut meta_parts = Vec::new();
    if let Some(author) = &item.author {
        meta_parts.push(Span::styled(
            author.as_str(),
            Style::default()
                .fg(colors.text_secondary)
                .add_modifier(Modifier::ITALIC),
        ));
    }
    if let Some(date) = &item.formatted_date {
        if !meta_parts.is_empty() {
            meta_parts.push(Span::styled(" · ", Style::default().fg(colors.muted)));
        }
        meta_parts.push(Span::styled(
            date.as_str(),
            Style::default().fg(colors.muted),
        ));
    }
    if !meta_parts.is_empty() {
        lines.push(Line::from(meta_parts));
    }

    // Separator
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "─".repeat(area.width.saturating_sub(8) as usize),
        Style::default().fg(colors.border),
    )]));
    lines.push(Line::from(""));

    // Content
    if let Some(desc) = &item.description {
        let raw_text = from_read(desc.as_bytes(), area.width.saturating_sub(10) as usize);
        let formatted = format_content_for_reading(&raw_text);
        for line in formatted.lines() {
            lines.push(Line::from(vec![Span::styled(
                line.to_string(),
                Style::default().fg(colors.text),
            )]));
        }
    } else {
        lines.push(Line::from(vec![Span::styled(
            "No content available",
            Style::default().fg(colors.muted),
        )]));
    }

    let viewport_height = area
        .height
        .saturating_sub(2) // borders
        .saturating_sub(2); // padding

    // Account for line wrapping when calculating content height
    let content_width = area
        .width
        .saturating_sub(2) // borders
        .saturating_sub(4) // padding (2 left + 2 right)
        as usize;
    let content_text: String = lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let content_lines = count_wrapped_lines(&content_text, content_width);

    // Compute scroll locally to avoid mutable borrow while lines borrows app
    let preview_max_scroll = content_lines.saturating_sub(viewport_height);
    let preview_scroll = app.preview_scroll.min(preview_max_scroll);

    let preview = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" {} Preview ", article_icon))
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(colors.border_normal)
                .border_style(Style::default().fg(colors.border_focus))
                .style(Style::default().bg(colors.surface))
                .padding(Padding::new(2, 2, 1, 1)),
        )
        .scroll((preview_scroll, 0))
        .wrap(Wrap { trim: true });

    // Render consumes the Paragraph, releasing borrows into app
    f.render_widget(preview, area);

    // Now safe to update app scroll state
    app.preview_max_scroll = preview_max_scroll;
    app.preview_scroll = preview_scroll;
}

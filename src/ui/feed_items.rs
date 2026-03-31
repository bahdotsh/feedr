use crate::app::App;
use crate::ui::utils::truncate_str;
use crate::ui::ColorScheme;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph},
    Frame,
};

pub(super) fn render_feed_items<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    area: Rect,
    colors: &ColorScheme,
) {
    if let Some(feed) = app.current_feed() {
        let feed_icon = colors.get_icon_feed();
        let title = format!(" {} {} ", feed_icon, feed.title);

        if feed.items.is_empty() {
            // Empty feed visualization
            let mut text = Text::default();
            let empty_icon = if colors.border_normal == BorderType::Double {
                "◇" // Dark: hollow diamond
            } else {
                "📭" // Light: mailbox
            };

            text.lines.push(Line::from(""));
            text.lines.push(Line::from(Span::styled(
                format!("       {}       ", empty_icon),
                Style::default().fg(colors.secondary),
            )));
            text.lines.push(Line::from(""));
            text.lines.push(Line::from(Span::styled(
                "No items in this feed",
                Style::default()
                    .fg(colors.text)
                    .add_modifier(Modifier::BOLD),
            )));
            text.lines.push(Line::from(""));
            text.lines.push(Line::from(Span::styled(
                "This feed might be empty or need refreshing",
                Style::default().fg(colors.muted),
            )));
            text.lines.push(Line::from(""));
            text.lines.push(Line::from(Span::styled(
                "Press 'r' to refresh feeds",
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

        // Enhanced feed items with theme-specific styling
        let arrow = colors.get_arrow_right();
        let success_icon = colors.get_icon_success();
        let items: Vec<ListItem> = feed
            .items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let date_str = item.formatted_date.as_deref().unwrap_or("");
                let author = item.author.as_deref().unwrap_or("");
                let is_selected = app.selected_item == Some(idx);
                let is_read = app
                    .selected_feed
                    .is_some_and(|feed_idx| app.is_item_read(feed_idx, idx));
                let is_starred = app
                    .selected_feed
                    .is_some_and(|feed_idx| app.is_item_starred(feed_idx, idx));

                // Use cached plain_text to avoid HTML parsing per frame
                let snippet = if let Some(plain_text) = &item.plain_text {
                    // Remove excess whitespace for cleaner display
                    let clean_text = plain_text
                        .replace('\n', " ")
                        .replace("  ", " ")
                        .trim()
                        .to_string();
                    truncate_str(&clean_text, 100)
                } else {
                    "".to_string()
                };

                // Create compact but readable item layout with theme-specific indicators
                let mut lines = vec![
                    // Title with read indicator
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
                ];

                // Add content preview with subtle styling
                if !snippet.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            snippet,
                            Style::default().fg(if is_selected {
                                colors.text_secondary
                            } else {
                                colors.muted
                            }),
                        ),
                    ]));
                }

                // Add metadata on one line
                let mut metadata_parts = Vec::new();
                metadata_parts.push(Span::styled("  ", Style::default()));

                if !author.is_empty() {
                    metadata_parts.push(Span::styled(author, Style::default().fg(colors.muted)));
                    if !date_str.is_empty() {
                        metadata_parts.push(Span::styled(" · ", Style::default().fg(colors.muted)));
                    }
                }

                if !date_str.is_empty() {
                    metadata_parts.push(Span::styled(date_str, Style::default().fg(colors.muted)));
                }

                if !metadata_parts.is_empty() {
                    lines.push(Line::from(metadata_parts));
                }

                // Add spacing between items
                lines.push(Line::from(""));

                ListItem::new(lines).style(Style::default().fg(colors.text).bg(if is_selected {
                    colors.selected_bg
                } else {
                    colors.background
                }))
            })
            .collect();

        let items_list = List::new(items)
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

        f.render_stateful_widget(items_list, area, &mut state);
    }
}

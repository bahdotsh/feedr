use crate::app::{App, TreeItem};
use crate::ui::ColorScheme;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

pub(super) fn render_feed_list<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    area: Rect,
    colors: &ColorScheme,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let title_text = vec![
        Line::from(Span::styled(
            "  Add your RSS/Atom feeds to get started  ",
            Style::default().fg(colors.muted),
        )),
        Line::from(Span::styled(
            "  Press 'a' to add a feed                  ",
            Style::default().fg(colors.highlight),
        )),
    ];

    let title_para = Paragraph::new(Text::from(title_text))
        .block(Block::default().borders(Borders::NONE))
        .alignment(Alignment::Left);

    f.render_widget(title_para, chunks[0]);

    if app.feeds.is_empty() && app.categories.is_empty() {
        // Theme-specific empty feed ASCII art
        let mut text = Text::default();
        let art_lines = colors.get_empty_feed_art();

        for &line in art_lines {
            // Style based on theme
            if colors.border_normal == BorderType::Double {
                // Dark theme styling
                if line.contains("╔")
                    || line.contains("╗")
                    || line.contains("║")
                    || line.contains("╚")
                    || line.contains("╝")
                {
                    text.lines.push(Line::from(Span::styled(
                        line,
                        Style::default().fg(colors.primary),
                    )));
                } else if line.contains("◢◣") || line.contains("◤◥") {
                    text.lines.push(Line::from(Span::styled(
                        line,
                        Style::default().fg(colors.accent),
                    )));
                } else if line.contains("CYBER")
                    || line.contains("NO_SIGNAL")
                    || line.contains("INIT_FEED")
                {
                    text.lines.push(Line::from(Span::styled(
                        line,
                        Style::default().fg(colors.secondary),
                    )));
                } else if line.contains("═══") {
                    text.lines.push(Line::from(Span::styled(
                        line,
                        Style::default().fg(colors.highlight),
                    )));
                } else {
                    text.lines.push(Line::from(line));
                }
            } else {
                // Light theme styling
                if line.contains("_")
                    || line.contains("( )")
                    || line.contains("|")
                    || line.contains("/")
                    || line.contains("\\")
                    || line.contains("-")
                {
                    text.lines.push(Line::from(Span::styled(
                        line,
                        Style::default().fg(colors.primary),
                    )));
                } else {
                    text.lines.push(Line::from(line));
                }
            }
        }

        // Help message
        text.lines.push(Line::from(Span::styled(
            "  No feeds added yet!                      ",
            Style::default()
                .fg(colors.text)
                .add_modifier(Modifier::BOLD),
        )));
        text.lines.push(Line::from(Span::styled(
            "                                           ",
            Style::default().fg(colors.muted),
        )));
        text.lines.push(Line::from(Span::styled(
            "  Press 'a' to add a feed                  ",
            Style::default().fg(colors.highlight),
        )));

        let feed_icon = colors.get_icon_feed();
        let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .title(format!(" {} Feeds ", feed_icon))
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(colors.border_normal)
                .border_style(Style::default().fg(colors.border))
                .style(Style::default().bg(colors.surface))
                .padding(Padding::new(2, 2, 2, 2)),
        );

        f.render_widget(paragraph, chunks[1]);
        return;
    }

    // Build tree view items
    let bullet = colors.get_list_bullet();
    let items: Vec<ListItem> = app
        .feed_tree
        .iter()
        .enumerate()
        .map(|(i, tree_item)| {
            let is_selected = app.selected_tree_item == Some(i);
            match tree_item {
                TreeItem::Category(cat_idx) => {
                    let category = &app.categories[*cat_idx];
                    let expand_icon = if category.expanded {
                        "\u{25be}" // ▾
                    } else {
                        "\u{25b8}" // ▸
                    };
                    let feed_count = category.feeds.len();
                    let name_style = Style::default()
                        .fg(if is_selected {
                            colors.highlight
                        } else {
                            colors.primary
                        })
                        .add_modifier(Modifier::BOLD);
                    let count_style = Style::default().fg(colors.muted);

                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{} ", expand_icon),
                            Style::default().fg(if is_selected {
                                colors.highlight
                            } else {
                                colors.secondary
                            }),
                        ),
                        Span::styled(category.name.clone(), name_style),
                        Span::styled(
                            format!(
                                " ({} feed{})",
                                feed_count,
                                if feed_count == 1 { "" } else { "s" }
                            ),
                            count_style,
                        ),
                    ]))
                }
                TreeItem::Feed(feed_idx, parent) => {
                    let feed = &app.feeds[*feed_idx];
                    let indent = if parent.is_some() { "    " } else { "  " };
                    let item_count = feed.items.len();
                    let domain = extract_domain(&feed.url);

                    let title_style = Style::default()
                        .fg(if is_selected {
                            colors.text
                        } else {
                            colors.text_secondary
                        })
                        .add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        });

                    ListItem::new(Line::from(vec![
                        Span::styled(indent, Style::default()),
                        Span::styled(
                            format!("{} ", bullet),
                            Style::default().fg(if is_selected {
                                colors.highlight
                            } else {
                                colors.accent
                            }),
                        ),
                        Span::styled(feed.title.clone(), title_style),
                        Span::styled(
                            format!(" ({})", item_count),
                            Style::default().fg(colors.muted),
                        ),
                        Span::styled(
                            format!(" \u{00b7} {}", domain),
                            Style::default().fg(colors.muted),
                        ),
                    ]))
                }
            }
        })
        .collect();

    let feed_icon = colors.get_icon_feed();
    let arrow = colors.get_arrow_right();
    let highlight_symbol = format!("{} ", arrow);
    let feeds = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(colors.border_normal)
                .title(format!(" {} Feeds ", feed_icon))
                .title_alignment(Alignment::Center)
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
        .highlight_symbol(&highlight_symbol);

    // Create a mutable ListState to track selection
    let mut list_state = ListState::default();
    list_state.select(app.selected_tree_item);

    f.render_stateful_widget(feeds, chunks[1], &mut list_state);
}

// Add this helper function to extract domain from URL
pub fn extract_domain(url: &str) -> String {
    let clean_url = url
        .replace("https://", "")
        .replace("http://", "")
        .replace("www.", "");

    if let Some(slash_pos) = clean_url.find('/') {
        clean_url[..slash_pos].to_string()
    } else {
        clean_url
    }
}

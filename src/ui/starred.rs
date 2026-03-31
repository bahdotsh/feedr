use crate::app::App;
use crate::ui::ColorScheme;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

pub(super) fn render_starred<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    area: Rect,
    colors: &ColorScheme,
) {
    let starred_items = app.get_starred_dashboard_items();
    let title = format!(" \u{2605} Starred Articles ({}) ", starred_items.len());

    if starred_items.is_empty() {
        let star_icon = if colors.border_normal == BorderType::Double {
            "\u{2606}" // Dark: hollow star
        } else {
            "\u{2605}" // Light: filled star
        };

        let mut text = Text::default();
        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            format!("       {}       ", star_icon),
            Style::default().fg(Color::Rgb(255, 215, 0)),
        )));
        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            "No starred articles yet",
            Style::default()
                .fg(colors.text)
                .add_modifier(Modifier::BOLD),
        )));
        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            "Press s on any article to star it",
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

    let arrow = colors.get_arrow_right();
    let success_icon = colors.get_icon_success();
    let items: Vec<ListItem> = starred_items
        .iter()
        .enumerate()
        .map(|(idx, &(feed_idx, item_idx))| {
            let feed = &app.feeds[feed_idx];
            let item = &feed.items[item_idx];
            let date_str = item.formatted_date.as_deref().unwrap_or("Unknown date");
            let is_selected = app.selected_item == Some(idx);
            let is_read = app.is_item_read(feed_idx, item_idx);

            ListItem::new(vec![
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
                    Span::styled(" \u{2605}", Style::default().fg(Color::Rgb(255, 215, 0))),
                    Span::styled(
                        if is_read {
                            format!(" {}", success_icon)
                        } else {
                            "".to_string()
                        },
                        Style::default().fg(colors.success),
                    ),
                ]),
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
                Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(date_str, Style::default().fg(colors.muted)),
                ]),
                Line::from(""),
            ])
            .style(Style::default().fg(colors.text).bg(if is_selected {
                colors.selected_bg
            } else {
                colors.background
            }))
        })
        .collect();

    let starred_list = List::new(items)
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

    let mut state = ListState::default();
    state.select(app.selected_item);

    f.render_stateful_widget(starred_list, area, &mut state);
}

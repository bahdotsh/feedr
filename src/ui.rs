use crate::app::{App, InputMode, View};
use html2text::from_read;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Tabs, Wrap},
    Frame,
};

// Define a consistent color palette
const PRIMARY_COLOR: Color = Color::Rgb(0, 135, 175); // Cyan-blue
const SECONDARY_COLOR: Color = Color::Rgb(247, 140, 108); // Coral
const HIGHLIGHT_COLOR: Color = Color::Rgb(195, 232, 141); // Light green
const BACKGROUND_COLOR: Color = Color::Rgb(28, 28, 28); // Dark gray
const TEXT_COLOR: Color = Color::Rgb(220, 220, 220); // Light gray
const MUTED_COLOR: Color = Color::Rgb(130, 130, 130); // Medium gray

pub fn render<B: Backend>(f: &mut Frame<B>, app: &App) {
    // Set background color
    let bg_block = Block::default().style(Style::default().bg(BACKGROUND_COLOR));
    f.render_widget(bg_block, f.size());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title bar
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Help bar
        ])
        .split(f.size());

    render_title_bar(f, app, chunks[0]);

    match app.view {
        View::Dashboard => render_dashboard(f, app, chunks[1]),
        View::FeedList => render_feed_list(f, app, chunks[1]),
        View::FeedItems => render_feed_items(f, app, chunks[1]),
        View::FeedItemDetail => render_item_detail(f, app, chunks[1]),
    }

    render_help_bar(f, app, chunks[2]);

    // Show error if present
    if let Some(error) = &app.error {
        render_error_modal(f, error);
    }

    // Show input modal when in input modes
    if matches!(app.input_mode, InputMode::InsertUrl | InputMode::SearchMode) {
        render_input_modal(f, app);
    }
}

fn render_title_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    // Create tabs for navigation
    let titles = vec!["Dashboard", "Feeds", "Items", "Detail"];
    let selected_tab = match app.view {
        View::Dashboard => 0,
        View::FeedList => 1,
        View::FeedItems => 2,
        View::FeedItemDetail => 3,
    };

    let tabs = Tabs::new(titles.iter().map(|t| Line::from(*t)).collect())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(PRIMARY_COLOR))
                .padding(Padding::new(1, 0, 0, 0)),
        )
        .style(Style::default().fg(MUTED_COLOR))
        .select(selected_tab)
        .highlight_style(
            Style::default()
                .fg(HIGHLIGHT_COLOR)
                .add_modifier(Modifier::BOLD),
        )
        .divider(symbols::line::VERTICAL);

    f.render_widget(tabs, area);
}

fn render_dashboard<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let title = if app.is_searching {
        format!(" Search Results: '{}' ", app.search_query)
    } else {
        " Latest Updates ".to_string()
    };

    let items_to_display = if app.is_searching {
        &app.filtered_items
    } else {
        &app.dashboard_items
    };

    if items_to_display.is_empty() {
        let message = if app.is_searching {
            format!("No results found for '{}'", app.search_query)
        } else {
            "No recent items. Add feeds with 'a' or refresh with 'r'".to_string()
        };

        let paragraph = Paragraph::new(message)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .title(title)
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .padding(Padding::new(1, 1, 1, 1)),
            )
            .style(Style::default().fg(MUTED_COLOR));

        f.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = items_to_display
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let (feed, item) = if app.is_searching {
                app.search_item(idx).unwrap()
            } else {
                app.dashboard_item(idx).unwrap()
            };

            let date_str = item.formatted_date.as_deref().unwrap_or("Unknown date");

            // Create a formatted list item
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!("● {}: ", feed.title),
                        Style::default()
                            .fg(SECONDARY_COLOR)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&item.title, Style::default().fg(TEXT_COLOR)),
                ]),
                Line::from(vec![Span::styled(
                    format!("  {}", date_str),
                    Style::default().fg(MUTED_COLOR),
                )]),
            ])
            .style(Style::default().fg(TEXT_COLOR))
        })
        .collect();

    let dashboard_list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(PRIMARY_COLOR))
                .padding(Padding::new(1, 0, 0, 0)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(50, 50, 50))
                .fg(HIGHLIGHT_COLOR)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" → ");

    let mut state = ratatui::widgets::ListState::default();
    state.select(app.selected_item);

    f.render_stateful_widget(dashboard_list, area, &mut state);
}

fn render_feed_list<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    if app.feeds.is_empty() {
        let paragraph = Paragraph::new("No feeds added. Press 'a' to add a feed.")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .title(" Feeds ")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .padding(Padding::new(1, 1, 1, 1)),
            )
            .style(Style::default().fg(MUTED_COLOR));

        f.render_widget(paragraph, area);
        return;
    }

    let feeds: Vec<ListItem> = app
        .feeds
        .iter()
        .map(|feed| {
            ListItem::new(vec![
                Line::from(vec![Span::styled(
                    format!("● {}", feed.title),
                    Style::default()
                        .fg(SECONDARY_COLOR)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    format!("  {} items", feed.items.len()),
                    Style::default().fg(MUTED_COLOR),
                )]),
            ])
            .style(Style::default().fg(TEXT_COLOR))
        })
        .collect();

    let feeds = List::new(feeds)
        .block(
            Block::default()
                .title(" Feeds ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(PRIMARY_COLOR))
                .padding(Padding::new(1, 0, 0, 0)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(50, 50, 50))
                .fg(HIGHLIGHT_COLOR)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" → ");

    let mut state = ratatui::widgets::ListState::default();
    state.select(app.selected_feed);

    f.render_stateful_widget(feeds, area, &mut state);
}

fn render_feed_items<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    if let Some(feed) = app.current_feed() {
        let title = format!(" {} ", feed.title);

        if feed.items.is_empty() {
            let paragraph = Paragraph::new("No items in this feed.")
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .title(title)
                        .title_alignment(Alignment::Center)
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(PRIMARY_COLOR))
                        .padding(Padding::new(1, 1, 1, 1)),
                )
                .style(Style::default().fg(MUTED_COLOR));

            f.render_widget(paragraph, area);
            return;
        }

        let items: Vec<ListItem> = feed
            .items
            .iter()
            .map(|item| {
                let date_str = item.formatted_date.as_deref().unwrap_or("");
                let author = item.author.as_deref().unwrap_or("");

                ListItem::new(vec![
                    Line::from(vec![Span::styled(
                        format!("● {}", item.title),
                        Style::default()
                            .fg(HIGHLIGHT_COLOR)
                            .add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(vec![if !author.is_empty() {
                        Span::styled(
                            format!("  By: {} • {}", author, date_str),
                            Style::default().fg(MUTED_COLOR),
                        )
                    } else {
                        Span::styled(format!("  {}", date_str), Style::default().fg(MUTED_COLOR))
                    }]),
                ])
                .style(Style::default().fg(TEXT_COLOR))
            })
            .collect();

        let items_list = List::new(items)
            .block(
                Block::default()
                    .title(title)
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .padding(Padding::new(1, 0, 0, 0)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(50, 50, 50))
                    .fg(HIGHLIGHT_COLOR)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(" → ");

        let mut state = ratatui::widgets::ListState::default();
        state.select(app.selected_item);

        f.render_stateful_widget(items_list, area, &mut state);
    }
}

fn render_item_detail<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    if let Some(item) = app.current_item() {
        // Split the area into header and content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Header
                Constraint::Min(0),    // Content
            ])
            .split(area);

        // Create header with title, date, and author
        let mut header_lines = vec![Line::from(vec![
            Span::styled(
                "Title: ",
                Style::default()
                    .fg(SECONDARY_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &item.title,
                Style::default()
                    .fg(HIGHLIGHT_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
        ])];

        // Add publication date if available
        if let Some(date) = &item.formatted_date {
            header_lines.push(Line::from(vec![
                Span::styled(
                    "Date: ",
                    Style::default()
                        .fg(SECONDARY_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(date, Style::default().fg(TEXT_COLOR)),
            ]));
        }

        // Add author if available
        if let Some(author) = &item.author {
            header_lines.push(Line::from(vec![
                Span::styled(
                    "Author: ",
                    Style::default()
                        .fg(SECONDARY_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(author, Style::default().fg(TEXT_COLOR)),
            ]));
        }

        // Add link
        if let Some(link) = &item.link {
            header_lines.push(Line::from(vec![
                Span::styled(
                    "Link: ",
                    Style::default()
                        .fg(SECONDARY_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    link,
                    Style::default()
                        .fg(PRIMARY_COLOR)
                        .add_modifier(Modifier::UNDERLINED),
                ),
            ]));
            header_lines.push(Line::from(vec![Span::styled(
                "Press 'o' to open in browser",
                Style::default().fg(MUTED_COLOR),
            )]));
        }

        let header = Paragraph::new(header_lines)
            .block(
                Block::default()
                    .title(" Item Details ")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .padding(Padding::new(1, 1, 0, 0)),
            )
            .style(Style::default().fg(TEXT_COLOR));

        f.render_widget(header, chunks[0]);

        // Render content with HTML converted to plain text
        let description = if let Some(desc) = &item.description {
            from_read(desc.as_bytes(), 80)
        } else {
            "No description available".to_string()
        };

        let content = Paragraph::new(description)
            .block(
                Block::default()
                    .title(" Content ")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .padding(Padding::new(1, 1, 1, 1)),
            )
            .style(Style::default().fg(TEXT_COLOR))
            .wrap(Wrap { trim: true });

        f.render_widget(content, chunks[1]);
    }
}

fn render_help_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let (msg, style) = match app.input_mode {
        InputMode::Normal => {
            let help_text = match app.view {
                View::Dashboard => "f: feeds | a: add feed | r: refresh | enter: view item | o: open link | /: search | q: quit",
                View::FeedList => "h/esc: home | a: add feed | d: delete feed | enter: view feed | r: refresh | /: search | q: quit",
                View::FeedItems => "h/esc: back to feeds | home: dashboard | enter: view detail | o: open link | /: search | q: quit",
                View::FeedItemDetail => "h/esc: back | home: dashboard | o: open in browser | q: quit",
            };
            (help_text, Style::default().fg(TEXT_COLOR))
        }
        InputMode::InsertUrl => ("", Style::default().fg(MUTED_COLOR)),
        InputMode::SearchMode => ("", Style::default().fg(MUTED_COLOR)),
    };

    // Only show help bar in normal mode
    if matches!(app.input_mode, InputMode::Normal) {
        let help = Paragraph::new(msg)
            .style(style)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .title(" Help ")
                    .title_alignment(Alignment::Center),
            );
        f.render_widget(help, area);
    }
}

fn render_error_modal<B: Backend>(f: &mut Frame<B>, error: &str) {
    let area = centered_rect(60, 20, f.size());

    // Clear the background
    f.render_widget(Clear, area);

    let error_text = Paragraph::new(Text::from(error.to_string()))
        .style(Style::default().fg(Color::Red))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Red))
                .title(" Error ")
                .title_alignment(Alignment::Center)
                .padding(Padding::new(1, 1, 1, 1)),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(error_text, area);
}

fn render_input_modal<B: Backend>(f: &mut Frame<B>, app: &App) {
    let area = centered_rect(60, 20, f.size());

    // Clear the background
    f.render_widget(Clear, area);

    let title = if matches!(app.input_mode, InputMode::InsertUrl) {
        " Add Feed URL "
    } else {
        " Search "
    };

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(HIGHLIGHT_COLOR))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(PRIMARY_COLOR))
                .title(title)
                .title_alignment(Alignment::Center)
                .padding(Padding::new(1, 1, 1, 1)),
        );

    f.render_widget(input, area);
}

// Helper function to create a centered rect using up certain percentage of the available rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

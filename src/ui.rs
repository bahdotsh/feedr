use crate::app::{App, InputMode, View};
use html2text::from_read;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};

pub fn render<B: Backend>(f: &mut Frame<B>, app: &App) {
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
        .block(Block::default().borders(Borders::ALL))
        .select(selected_tab)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

fn render_dashboard<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let title = if app.is_searching {
        format!("Search Results: '{}'", app.search_query)
    } else {
        "Latest Updates".to_string()
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
            .block(Block::default().title(title).borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray));

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

            let content = Line::from(vec![
                Span::styled(
                    format!("{}: ", feed.title),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&item.title),
                Span::styled(format!(" ({})", date_str), Style::default().fg(Color::Gray)),
            ]);

            ListItem::new(content)
        })
        .collect();

    let dashboard_list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("» ");

    let mut state = ratatui::widgets::ListState::default();
    state.select(app.selected_item);

    f.render_stateful_widget(dashboard_list, area, &mut state);
}

fn render_feed_list<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    if app.feeds.is_empty() {
        let paragraph = Paragraph::new("No feeds added. Press 'a' to add a feed.")
            .alignment(Alignment::Center)
            .block(Block::default().title("Feeds").borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray));

        f.render_widget(paragraph, area);
        return;
    }

    let feeds: Vec<ListItem> = app
        .feeds
        .iter()
        .map(|feed| {
            let content = Line::from(vec![
                Span::styled(
                    &feed.title,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({} items)", feed.items.len()),
                    Style::default().fg(Color::Gray),
                ),
            ]);

            ListItem::new(content)
        })
        .collect();

    let feeds = List::new(feeds)
        .block(Block::default().title("Feeds").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("» ");

    let mut state = ratatui::widgets::ListState::default();
    state.select(app.selected_feed);

    f.render_stateful_widget(feeds, area, &mut state);
}

fn render_feed_items<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    if let Some(feed) = app.current_feed() {
        if feed.items.is_empty() {
            let paragraph = Paragraph::new("No items in this feed.")
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .title(feed.title.as_str())
                        .borders(Borders::ALL),
                )
                .style(Style::default().fg(Color::Gray));

            f.render_widget(paragraph, area);
            return;
        }

        let items: Vec<ListItem> = feed
            .items
            .iter()
            .map(|item| {
                let date_str = item.formatted_date.as_deref().unwrap_or("");

                let content = Line::from(vec![
                    Span::styled(
                        &item.title,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(format!("({})", date_str), Style::default().fg(Color::Gray)),
                ]);

                ListItem::new(vec![
                    content,
                    // Add a subtitle with author if available
                    if let Some(author) = &item.author {
                        Line::from(vec![Span::styled(
                            format!("By: {}", author),
                            Style::default().fg(Color::Blue),
                        )])
                    } else {
                        Line::from("")
                    },
                ])
            })
            .collect();

        let feed_title = format!("{} ({})", feed.title, feed.items.len());
        let items_list = List::new(items)
            .block(Block::default().title(feed_title).borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("» ");

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
                Constraint::Length(4), // Header
                Constraint::Min(0),    // Content
            ])
            .split(area);

        // Create header with title, date, and author
        let mut header_lines = vec![Line::from(vec![
            Span::styled("Title: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                &item.title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ])];

        // Add publication date if available
        if let Some(date) = &item.formatted_date {
            header_lines.push(Line::from(vec![
                Span::styled("Date: ", Style::default().fg(Color::Yellow)),
                Span::raw(date),
            ]));
        }

        // Add author if available
        if let Some(author) = &item.author {
            header_lines.push(Line::from(vec![
                Span::styled("Author: ", Style::default().fg(Color::Yellow)),
                Span::raw(author),
            ]));
        }

        // Add link
        if let Some(link) = &item.link {
            header_lines.push(Line::from(vec![
                Span::styled("Link: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    link,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::UNDERLINED),
                ),
                Span::raw(" (Press 'o' to open in browser)"),
            ]));
        }

        let header = Paragraph::new(header_lines)
            .block(Block::default().title("Item Details").borders(Borders::ALL))
            .style(Style::default());

        f.render_widget(header, chunks[0]);

        // Render content with HTML converted to plain text
        let description = if let Some(desc) = &item.description {
            from_read(desc.as_bytes(), 80)
        } else {
            "No description available".to_string()
        };

        let content = Paragraph::new(description)
            .block(Block::default().title("Content").borders(Borders::ALL))
            .wrap(Wrap { trim: true })
            .scroll((0, 0)); // TODO: Add scrolling functionality

        f.render_widget(content, chunks[1]);
    }
}

fn render_help_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let (msg, style) = match app.input_mode {
        InputMode::Normal => {
            let help_text = match app.view {
                View::Dashboard => "h: home | f: feeds | a: add feed | r: refresh | o: open link | /: search | q: quit",
                View::FeedList => "h: home | a: add feed | d: delete feed | enter: view feed | r: refresh | /: search | q: quit",
                View::FeedItems => "h: back | enter: view detail | o: open link | /: search | q: quit",
                View::FeedItemDetail => "h: back | o: open in browser | q: quit",
            };
            (help_text, Style::default().fg(Color::Gray))
        }
        InputMode::InsertUrl => (
            "Enter feed URL (ESC to cancel, Enter to submit)",
            Style::default().fg(Color::Yellow),
        ),
        InputMode::SearchMode => (
            "Enter search query (ESC to cancel, Enter to search)",
            Style::default().fg(Color::Yellow),
        ),
    };

    // Show input box when in insert or search mode
    if matches!(app.input_mode, InputMode::InsertUrl | InputMode::SearchMode) {
        let title = if matches!(app.input_mode, InputMode::InsertUrl) {
            "Add Feed URL"
        } else {
            "Search"
        };

        let input = Paragraph::new(app.input.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title(title));
        f.render_widget(input, area);
    } else {
        // Show help text in normal mode
        let help = Paragraph::new(msg)
            .style(style)
            .block(Block::default().borders(Borders::ALL).title("Help"));
        f.render_widget(help, area);
    }

    // Show error if present
    if let Some(error) = &app.error {
        let error_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3)])
            .margin(1)
            .split(f.size())[0];

        let error_msg = Paragraph::new(error.clone())
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL).title("Error"));

        f.render_widget(error_msg, error_area);
    }
}

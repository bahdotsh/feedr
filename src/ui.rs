use crate::app::{App, InputMode, View};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.size());

    render_title(f, chunks[0]);

    match app.view {
        View::FeedList => render_feed_list(f, app, chunks[1]),
        View::FeedItems => render_feed_items(f, app, chunks[1]),
        View::FeedItemDetail => render_item_detail(f, app, chunks[1]),
    }

    render_input_help(f, app, chunks[2]);
}

fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new("FEEDR - Minimalistic RSS Reader")
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

fn render_feed_list(f: &mut Frame, app: &App, area: Rect) {
    let feeds: Vec<ListItem> = app
        .feeds
        .iter()
        .map(|feed| ListItem::new(feed.title.clone()))
        .collect();

    let feeds = List::new(feeds)
        .block(Block::default().title("Feeds").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ratatui::widgets::ListState::default();
    state.select(app.selected_feed);

    f.render_stateful_widget(feeds, area, &mut state);
}

fn render_feed_items(f: &mut Frame, app: &App, area: Rect) {
    if let Some(feed) = app.current_feed() {
        let items: Vec<ListItem> = feed
            .items
            .iter()
            .map(|item| ListItem::new(item.title.clone()))
            .collect();

        let items = List::new(items)
            .block(
                Block::default()
                    .title(format!("Items: {}", feed.title))
                    .borders(Borders::ALL),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        let mut state = ratatui::widgets::ListState::default();
        state.select(app.selected_item);

        f.render_stateful_widget(items, area, &mut state);
    }
}

fn render_item_detail(f: &mut Frame, app: &App, area: Rect) {
    if let Some(item) = app.current_item() {
        // Create header with title and date
        let mut header = vec![Line::from(vec![
            Span::styled("Title: ", Style::default().fg(Color::Yellow)),
            Span::raw(&item.title),
        ])];

        // Add publication date if available
        if let Some(date) = &item.pub_date {
            header.push(Line::from(vec![
                Span::styled("Date: ", Style::default().fg(Color::Yellow)),
                Span::raw(date),
            ]));
        }

        // Add link if available
        if let Some(link) = &item.link {
            header.push(Line::from(vec![
                Span::styled("Link: ", Style::default().fg(Color::Yellow)),
                Span::raw(link),
            ]));
        }

        // Add a separator
        header.push(Line::from("-".repeat(area.width as usize - 4)));

        // Add description or a placeholder
        let description = if let Some(desc) = &item.description {
            desc
        } else {
            "No description available"
        };

        // Create the text with header and description
        let mut text = Text::from(header);
        text.extend(Text::from(description.to_string()));

        let content = Paragraph::new(text)
            .block(Block::default().title("Item Detail").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        f.render_widget(content, area);
    }
}

fn render_input_help(f: &mut Frame, app: &App, area: Rect) {
    let (msg, style) = match app.input_mode {
        InputMode::Normal => {
            let help_text = match app.view {
                View::FeedList => "a: add feed, d: delete feed, enter: view feed, q: quit",
                View::FeedItems => "esc: back to feeds, enter: view detail, q: quit",
                View::FeedItemDetail => "esc: back to items, q: quit",
            };
            (help_text, Style::default().fg(Color::Gray))
        }
        InputMode::InsertUrl => (
            "Enter feed URL (ESC to cancel, Enter to submit)",
            Style::default().fg(Color::Yellow),
        ),
    };

    // Show input box when in insert mode
    if let InputMode::InsertUrl = app.input_mode {
        let input = Paragraph::new(app.input.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("URL"));
        f.render_widget(input, area);
    } else {
        // Show help text in normal mode
        let help = Paragraph::new(msg)
            .style(style)
            .block(Block::default().borders(Borders::ALL));
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

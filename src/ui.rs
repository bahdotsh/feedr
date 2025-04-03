use crate::app::{App, InputMode, View};
use html2text::from_read;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::{self},
    text::{Line, Span, Text},
    widgets::{
        canvas::{Canvas, Rectangle},
        Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Tabs, Wrap,
    },
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// Define a refined color palette inspired by modern terminal themes
const PRIMARY_COLOR: Color = Color::Cyan; // blue
const SECONDARY_COLOR: Color = Color::Magenta; // purple
const HIGHLIGHT_COLOR: Color = Color::Green; // mint
const BACKGROUND_COLOR: Color = Color::Black; // charcoal
const TEXT_COLOR: Color = Color::White; // off-white
const MUTED_COLOR: Color = Color::DarkGray; // steel gray
const ACCENT_COLOR: Color = Color::Yellow; // gold
const ERROR_COLOR: Color = Color::Red; // rose
const BORDER_COLOR: Color = Color::DarkGray; // dark gray

const NORMAL_BORDER: BorderType = BorderType::Rounded;
const ACTIVE_BORDER: BorderType = BorderType::Thick;

pub fn render<B: Backend>(f: &mut Frame<B>, app: &App) {
    // Set background color for the entire terminal
    let bg_block = Block::default().style(Style::default().bg(BACKGROUND_COLOR));
    f.render_widget(bg_block, f.size());

    // Main layout division
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title/tab bar
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

    // Loading animation characters - smoother spinner
    let loading_symbols = ["◐", "◓", "◑", "◒"];

    // Create title with loading indicator if loading
    let title = if app.is_loading {
        format!(
            " {} Loading... ",
            loading_symbols[app.loading_indicator % 4]
        )
    } else {
        " 📰 Feedr ".to_string()
    };

    // Create tab highlight effect
    let tabs = Tabs::new(
        titles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let prefix = if i == selected_tab { "▪ " } else { "  " };
                Line::from(vec![Span::styled(
                    format!("{}{}", prefix, t),
                    if i == selected_tab {
                        Style::default()
                            .fg(HIGHLIGHT_COLOR)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(MUTED_COLOR)
                    },
                )])
            })
            .collect(),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(if app.is_loading {
                ACTIVE_BORDER
            } else {
                NORMAL_BORDER
            })
            .border_style(Style::default().fg(PRIMARY_COLOR))
            .title(title)
            .title_alignment(Alignment::Center)
            .padding(Padding::new(1, 0, 0, 0)),
    )
    .style(Style::default().fg(MUTED_COLOR))
    .select(selected_tab)
    .divider(symbols::line::VERTICAL);

    f.render_widget(tabs, area);
}

fn render_dashboard<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let title = if app.is_searching {
        format!(" 🔍 Search Results: '{}' ", app.search_query)
    } else {
        " 🔔 Latest Updates ".to_string()
    };

    let items_to_display = if app.is_searching {
        &app.filtered_items
    } else {
        &app.dashboard_items
    };

    if items_to_display.is_empty() {
        let message = if app.is_searching {
            let no_results = format!("No results found for '{}'", app.search_query);

            // Create a visually appealing empty search results screen
            let mut lines = Vec::new();
            lines.push("");
            lines.push("       🔍       ");
            lines.push("");
            lines.push(&no_results);
            lines.push("");
            lines.push("Try different keywords or add more feeds");

            lines.join("\n")
        } else if app.feeds.is_empty() {
            // Enhanced ASCII art with color coding and interactive suggestions
            let ascii_art = vec![
                "                                                ",
                "  ███████╗███████╗███████╗██████╗ ██████╗      ",
                "  ██╔════╝██╔════╝██╔════╝██╔══██╗██╔══██╗     ",
                "  █████╗  █████╗  █████╗  ██║  ██║██████╔╝     ",
                "  ██╔══╝  ██╔══╝  ██╔══╝  ██║  ██║██╔══██╗     ",
                "  ██║     ███████╗███████╗██████╔╝██║  ██║     ",
                "  ╚═╝     ╚══════╝╚══════╝╚═════╝ ╚═╝  ╚═╝     ",
                "                                                ",
                "  Welcome to Feedr - Your Terminal RSS Reader   ",
                " ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ ",
                "                                                ",
                "  📚 Get started by adding your favorite RSS feeds",
                "  🔶 Press 'a' to add a feed URL                 ",
                "                                                ",
                "  Quick-add suggestions: (press the number key) ",
                "    1️⃣ news.ycombinator.com/rss                  ",
                "    2️⃣ feeds.feedburner.com/TechCrunch           ",
            ];
            ascii_art.join("\n")
        } else {
            let empty_msg = vec![
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

        // Rich text for empty dashboard
        let mut text = Text::default();

        if app.feeds.is_empty() && !app.is_searching {
            // For welcome screen
            for line in message.lines() {
                if line.contains("Welcome") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default()
                            .fg(ACCENT_COLOR)
                            .add_modifier(Modifier::BOLD),
                    )]));
                } else if line.contains("Press") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(HIGHLIGHT_COLOR),
                    )]));
                } else if line.contains("Some suggestions") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default()
                            .fg(SECONDARY_COLOR)
                            .add_modifier(Modifier::BOLD),
                    )]));
                } else if line.contains("•") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(PRIMARY_COLOR),
                    )]));
                } else if line.contains("Get started") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(TEXT_COLOR),
                    )]));
                } else if line.contains("━") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(BORDER_COLOR),
                    )]));
                } else if line.contains("███") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(PRIMARY_COLOR),
                    )]));
                } else {
                    text.lines.push(Line::from(line));
                }
            }
        } else {
            // For empty search or empty dashboard
            for line in message.lines() {
                if line.contains("🔍") || line.contains("📭") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(SECONDARY_COLOR),
                    )]));
                } else if line.contains("No results") || line.contains("No recent") {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
                    )]));
                } else {
                    text.lines.push(Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(MUTED_COLOR),
                    )]));
                }
            }
        }

        let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(NORMAL_BORDER)
                .border_style(Style::default().fg(PRIMARY_COLOR))
                .padding(Padding::new(1, 1, 1, 1)),
        );

        f.render_widget(paragraph, area);
        return;
    }

    // For non-empty dashboard, create richly formatted items
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
            let is_selected = app.selected_item.map_or(false, |selected| selected == idx);

            // Create clearer visual group with feed name as header
            ListItem::new(vec![
                // Feed source with icon - more prominent
                Line::from(vec![
                    Span::styled(
                        if is_selected { "► " } else { "● " },
                        Style::default().fg(if is_selected {
                            HIGHLIGHT_COLOR
                        } else {
                            PRIMARY_COLOR
                        }),
                    ),
                    Span::styled(
                        format!("[{}]", feed.title),
                        Style::default()
                            .fg(SECONDARY_COLOR)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                // Item title - more prominent
                Line::from(vec![
                    Span::styled("  │ ", Style::default().fg(BORDER_COLOR)),
                    Span::styled(
                        &item.title,
                        Style::default()
                            .fg(if is_selected {
                                HIGHLIGHT_COLOR
                            } else {
                                TEXT_COLOR
                            })
                            .add_modifier(if is_selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                ]),
                // Publication date with icon
                Line::from(vec![
                    Span::styled("  └─ ", Style::default().fg(BORDER_COLOR)),
                    Span::styled("🕒 ", Style::default().fg(PRIMARY_COLOR)),
                    Span::styled(date_str, Style::default().fg(MUTED_COLOR)),
                ]),
                // Empty line for spacing between items
                Line::from(""),
            ])
            .style(Style::default().fg(TEXT_COLOR).bg(if is_selected {
                Color::Rgb(59, 66, 82)
            } else {
                BACKGROUND_COLOR
            }))
        })
        .collect();

    let dashboard_list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(NORMAL_BORDER)
                .border_style(Style::default().fg(PRIMARY_COLOR))
                .padding(Padding::new(1, 0, 0, 0)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(59, 66, 82)) // Slightly lighter than background
                .fg(HIGHLIGHT_COLOR)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ► ");

    let mut state = ratatui::widgets::ListState::default();
    state.select(app.selected_item);

    f.render_stateful_widget(dashboard_list, area, &mut state);
}

fn render_feed_list<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    if app.feeds.is_empty() {
        // Enhanced ASCII art for empty feeds
        let mut text = Text::default();

        // Stylized ASCII robot
        text.lines.push(Line::from(Span::styled(
            "                                           ",
            Style::default().fg(MUTED_COLOR),
        )));
        text.lines.push(Line::from(Span::styled(
            "       .---.                               ",
            Style::default().fg(PRIMARY_COLOR),
        )));
        text.lines.push(Line::from(vec![
            Span::styled("      |", Style::default().fg(PRIMARY_COLOR)),
            Span::styled("o_o", Style::default().fg(ACCENT_COLOR)),
            Span::styled(
                " |                              ",
                Style::default().fg(PRIMARY_COLOR),
            ),
        ]));
        text.lines.push(Line::from(vec![
            Span::styled("      |", Style::default().fg(PRIMARY_COLOR)),
            Span::styled(":_/", Style::default().fg(SECONDARY_COLOR)),
            Span::styled(
                " |                              ",
                Style::default().fg(PRIMARY_COLOR),
            ),
        ]));
        text.lines.push(Line::from(Span::styled(
            "     //   \\ \\                             ",
            Style::default().fg(PRIMARY_COLOR),
        )));
        text.lines.push(Line::from(Span::styled(
            "    (|     | )                            ",
            Style::default().fg(PRIMARY_COLOR),
        )));
        text.lines.push(Line::from(Span::styled(
            "   /'\\_   _/`\\                           ",
            Style::default().fg(PRIMARY_COLOR),
        )));
        text.lines.push(Line::from(Span::styled(
            "   \\___)=(___/                           ",
            Style::default().fg(PRIMARY_COLOR),
        )));
        text.lines.push(Line::from(Span::styled(
            "                                           ",
            Style::default().fg(MUTED_COLOR),
        )));

        // Help message
        text.lines.push(Line::from(Span::styled(
            "  No feeds added yet!                      ",
            Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
        )));
        text.lines.push(Line::from(Span::styled(
            "                                           ",
            Style::default().fg(MUTED_COLOR),
        )));
        text.lines.push(Line::from(Span::styled(
            "  Press 'a' to add a feed                  ",
            Style::default().fg(HIGHLIGHT_COLOR),
        )));

        let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .title(" 📋 Feeds ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(NORMAL_BORDER)
                .border_style(Style::default().fg(PRIMARY_COLOR))
                .padding(Padding::new(1, 1, 1, 1)),
        );

        f.render_widget(paragraph, area);
        return;
    }

    // Improved feed list visualization for better readability
    let feeds: Vec<ListItem> = app
        .feeds
        .iter()
        .map(|feed| {
            // Create more visually distinct items with clearer hierarchy
            let item_count = feed.items.len();

            // Create a categorized badge based on feed size
            let (count_style, category) = match item_count {
                0..=5 => (
                    Style::default().fg(Color::Red).bg(Color::Rgb(40, 40, 40)),
                    "small",
                ),
                6..=20 => (
                    Style::default()
                        .fg(Color::Yellow)
                        .bg(Color::Rgb(40, 40, 40)),
                    "medium",
                ),
                21..=50 => (
                    Style::default().fg(ACCENT_COLOR).bg(Color::Rgb(40, 40, 40)),
                    "large",
                ),
                _ => (
                    Style::default()
                        .fg(HIGHLIGHT_COLOR)
                        .bg(Color::Rgb(40, 40, 40)),
                    "huge",
                ),
            };

            let count_badge = format!(" {} items ", item_count);

            // Extract domain for cleaner display
            let domain = extract_domain(&feed.url);

            ListItem::new(vec![
                // Title with clearer visual hierarchy
                Line::from(vec![
                    Span::styled("● ", Style::default().fg(PRIMARY_COLOR)),
                    Span::styled(
                        &feed.title,
                        Style::default()
                            .fg(SECONDARY_COLOR)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                // More informative metadata with visual separation
                Line::from(vec![
                    Span::styled("  ├─ ", Style::default().fg(BORDER_COLOR)),
                    Span::styled(count_badge, count_style.add_modifier(Modifier::BOLD)),
                    Span::styled(" ", Style::default().fg(MUTED_COLOR)),
                    Span::styled(format!("({})", category), Style::default().fg(MUTED_COLOR)),
                ]),
                // Source domain with icon
                Line::from(vec![
                    Span::styled("  └─ ", Style::default().fg(BORDER_COLOR)),
                    Span::styled("🌐 ", Style::default().fg(PRIMARY_COLOR)),
                    Span::styled(domain, Style::default().fg(TEXT_COLOR)),
                ]),
            ])
            .style(Style::default().fg(TEXT_COLOR))
        })
        .collect();

    let feeds = List::new(feeds)
        .block(
            Block::default()
                .title(" 📋 Your Feeds ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(NORMAL_BORDER)
                .border_style(Style::default().fg(PRIMARY_COLOR))
                .padding(Padding::new(1, 0, 0, 0)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(59, 66, 82)) // Slightly lighter background for selected item
                .fg(HIGHLIGHT_COLOR)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ► ");

    let mut state = ratatui::widgets::ListState::default();
    state.select(app.selected_feed);

    f.render_stateful_widget(feeds, area, &mut state);
}

// Add this helper function to extract domain from URL
fn extract_domain(url: &str) -> String {
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

fn render_feed_items<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    if let Some(feed) = app.current_feed() {
        let title = format!(" 📰 {} ", feed.title);

        if feed.items.is_empty() {
            // Empty feed visualization
            let mut text = Text::default();

            text.lines.push(Line::from(""));
            text.lines.push(Line::from(Span::styled(
                "       📭       ",
                Style::default().fg(SECONDARY_COLOR),
            )));
            text.lines.push(Line::from(""));
            text.lines.push(Line::from(Span::styled(
                "No items in this feed",
                Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
            )));
            text.lines.push(Line::from(""));
            text.lines.push(Line::from(Span::styled(
                "This feed might be empty or need refreshing",
                Style::default().fg(MUTED_COLOR),
            )));
            text.lines.push(Line::from(""));
            text.lines.push(Line::from(Span::styled(
                "Press 'r' to refresh feeds",
                Style::default().fg(HIGHLIGHT_COLOR),
            )));

            let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(
                Block::default()
                    .title(title)
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(NORMAL_BORDER)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .padding(Padding::new(1, 1, 1, 1)),
            );

            f.render_widget(paragraph, area);
            return;
        }

        // Enhanced feed items for better readability
        let items: Vec<ListItem> = feed
            .items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let date_str = item.formatted_date.as_deref().unwrap_or("");
                let author = item.author.as_deref().unwrap_or("");
                let is_selected = app.selected_item.map_or(false, |selected| selected == idx);

                // More distinct selection indicators
                let (title_color, bullet_icon, bullet_color) = if is_selected {
                    (HIGHLIGHT_COLOR, "►", PRIMARY_COLOR)
                } else {
                    (SECONDARY_COLOR, "•", MUTED_COLOR)
                };

                // Better formatted snippet with HTML cleanup
                let snippet = if let Some(desc) = &item.description {
                    let plain_text = html2text::from_read(desc.as_bytes(), 50);
                    // Remove excess whitespace for cleaner display
                    let clean_text = plain_text
                        .replace('\n', " ")
                        .replace("  ", " ")
                        .trim()
                        .to_string();
                    let snippet = truncate_str(&clean_text, 80);
                    snippet
                } else {
                    "".to_string()
                };

                // Create visually separated blocks for each item
                let mut lines = vec![
                    // Title with more distinct selection indicator
                    Line::from(vec![
                        Span::styled(
                            format!("{} ", bullet_icon),
                            Style::default().fg(bullet_color),
                        ),
                        Span::styled(
                            &item.title,
                            Style::default()
                                .fg(title_color)
                                .add_modifier(if is_selected {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ),
                    ]),
                ];

                // Add content preview with better formatting
                if !snippet.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  │ ", Style::default().fg(BORDER_COLOR)),
                        Span::styled(
                            snippet,
                            Style::default().fg(if is_selected { TEXT_COLOR } else { MUTED_COLOR }),
                        ),
                    ]));
                }

                // Add better formatted metadata with icons
                if !author.is_empty() && !date_str.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  └─ ", Style::default().fg(BORDER_COLOR)),
                        Span::styled("👤 ", Style::default().fg(PRIMARY_COLOR)),
                        Span::styled(author, Style::default().fg(TEXT_COLOR)),
                        Span::styled(" • ", Style::default().fg(BORDER_COLOR)),
                        Span::styled("🕒 ", Style::default().fg(PRIMARY_COLOR)),
                        Span::styled(date_str, Style::default().fg(TEXT_COLOR)),
                    ]));
                } else if !author.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  └─ ", Style::default().fg(BORDER_COLOR)),
                        Span::styled("👤 ", Style::default().fg(PRIMARY_COLOR)),
                        Span::styled(author, Style::default().fg(TEXT_COLOR)),
                    ]));
                } else if !date_str.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  └─ ", Style::default().fg(BORDER_COLOR)),
                        Span::styled("🕒 ", Style::default().fg(PRIMARY_COLOR)),
                        Span::styled(date_str, Style::default().fg(TEXT_COLOR)),
                    ]));
                }

                // Add a subtle separator line between items for better visual grouping
                lines.push(Line::from(Span::styled(
                    "  ",
                    Style::default().fg(MUTED_COLOR),
                )));

                ListItem::new(lines).style(Style::default().fg(TEXT_COLOR).bg(if is_selected {
                    Color::Rgb(59, 66, 82)
                } else {
                    BACKGROUND_COLOR
                }))
            })
            .collect();

        let items_list = List::new(items)
            .block(
                Block::default()
                    .title(title)
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(NORMAL_BORDER)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .padding(Padding::new(1, 0, 0, 0)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(59, 66, 82))
                    .fg(HIGHLIGHT_COLOR)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(""); // We're handling the symbol manually now

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
                Constraint::Length(6), // Header (increased for richer metadata)
                Constraint::Min(0),    // Content
            ])
            .split(area);

        // Create header with rich styling and icons
        let mut header_lines = vec![
            // Title with icon
            Line::from(vec![
                Span::styled("  ", Style::default().fg(MUTED_COLOR)),
                Span::styled(
                    &item.title,
                    Style::default()
                        .fg(HIGHLIGHT_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            // Separator line
            Line::from(vec![
                Span::styled("  ", Style::default().fg(MUTED_COLOR)),
                Span::styled(
                    "─".repeat(
                        (chunks[0].width as usize)
                            .saturating_sub(4)
                            .min(item.title.width()),
                    ),
                    Style::default().fg(BORDER_COLOR),
                ),
            ]),
        ];

        // Add publication date with icon
        if let Some(date) = &item.formatted_date {
            header_lines.push(Line::from(vec![
                Span::styled("  ", Style::default().fg(MUTED_COLOR)),
                Span::styled(
                    "🕒 ",
                    Style::default()
                        .fg(PRIMARY_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Published: ", Style::default().fg(SECONDARY_COLOR)),
                Span::styled(date, Style::default().fg(TEXT_COLOR)),
            ]));
        }

        // Add author with icon
        if let Some(author) = &item.author {
            header_lines.push(Line::from(vec![
                Span::styled("  ", Style::default().fg(MUTED_COLOR)),
                Span::styled(
                    "👤 ",
                    Style::default()
                        .fg(PRIMARY_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Author: ", Style::default().fg(SECONDARY_COLOR)),
                Span::styled(author, Style::default().fg(TEXT_COLOR)),
            ]));
        }

        // Add link with visual cue
        if let Some(link) = &item.link {
            header_lines.push(Line::from(vec![
                Span::styled("  ", Style::default().fg(MUTED_COLOR)),
                Span::styled(
                    "🔗 ",
                    Style::default()
                        .fg(PRIMARY_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Link: ", Style::default().fg(SECONDARY_COLOR)),
                Span::styled(
                    truncate_url(link, 40),
                    Style::default()
                        .fg(ACCENT_COLOR)
                        .add_modifier(Modifier::UNDERLINED),
                ),
            ]));

            // Add guidance for opening in browser
            header_lines.push(Line::from(vec![
                Span::styled("  ", Style::default().fg(MUTED_COLOR)),
                Span::styled("  └─ Press ", Style::default().fg(BORDER_COLOR)),
                Span::styled(
                    "'o'",
                    Style::default()
                        .fg(HIGHLIGHT_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to open in browser", Style::default().fg(BORDER_COLOR)),
            ]));
        }

        let header = Paragraph::new(header_lines)
            .block(
                Block::default()
                    .title(" 📄 Article Details ")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(NORMAL_BORDER)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .padding(Padding::new(1, 1, 0, 0)),
            )
            .style(Style::default().fg(TEXT_COLOR));

        f.render_widget(header, chunks[0]);

        // Render content with HTML converted to plain text and formatted
        let description = if let Some(desc) = &item.description {
            from_read(desc.as_bytes(), 80)
        } else {
            "No description available".to_string()
        };

        // Create content paragraph with enhanced formatting
        let content = Paragraph::new(description)
            .block(
                Block::default()
                    .title(" 📝 Content ")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(NORMAL_BORDER)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .padding(Padding::new(1, 1, 1, 1)),
            )
            .style(Style::default().fg(TEXT_COLOR))
            .wrap(Wrap { trim: true });

        f.render_widget(content, chunks[1]);
    }
}

fn render_help_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let (msg, _) = match app.input_mode {
        InputMode::Normal => {
            let help_text = match app.view {
                View::Dashboard => "  f: feeds  |  a: add feed  |  r: refresh  |  enter: view item  |  o: open link  |  /: search  |  q: quit  ",
                View::FeedList => "  h/esc: home  |  a: add feed  |  d: delete feed  |  enter: view feed  |  r: refresh  |  /: search  |  q: quit  ",
                View::FeedItems => "  h/esc: back to feeds  |  home: dashboard  |  enter: view detail  |  o: open link  |  /: search  |  q: quit  ",
                View::FeedItemDetail => "  h/esc: back  |  home: dashboard  |  o: open in browser  |  q: quit  ",
            };
            (help_text, Style::default().fg(TEXT_COLOR))
        }
        InputMode::InsertUrl => ("", Style::default().fg(MUTED_COLOR)),
        InputMode::SearchMode => ("", Style::default().fg(MUTED_COLOR)),
    };

    // Only show help bar in normal mode
    if matches!(app.input_mode, InputMode::Normal) {
        // Create a stylized help bar with visually separated commands
        let parts: Vec<&str> = msg.split('|').collect();
        let mut spans = Vec::new();

        for (idx, part) in parts.iter().enumerate() {
            let trimmed = part.trim();

            // Extract the command key and description
            if let Some(pos) = trimmed.find(':') {
                let (key, desc) = trimmed.split_at(pos + 1);

                // Add the key in highlight color
                spans.push(Span::styled(
                    key,
                    Style::default()
                        .fg(HIGHLIGHT_COLOR)
                        .add_modifier(Modifier::BOLD),
                ));

                // Add the description in normal text color
                spans.push(Span::styled(desc, Style::default().fg(TEXT_COLOR)));
            } else {
                spans.push(Span::styled(trimmed, Style::default().fg(TEXT_COLOR)));
            }

            // Add separator unless this is the last item
            if idx < parts.len() - 1 {
                spans.push(Span::styled(" | ", Style::default().fg(BORDER_COLOR)));
            }
        }

        let help = Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(NORMAL_BORDER)
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .title(" 💡 Commands ")
                    .title_alignment(Alignment::Center),
            );
        f.render_widget(help, area);
    }
}

fn render_error_modal<B: Backend>(f: &mut Frame<B>, error: &str) {
    let area = centered_rect(60, 25, f.size());

    // Clear the background
    f.render_widget(Clear, area);

    // Create a visually appealing error box
    let error_lines = vec![
        Line::from(Span::styled(
            "  ⚠️  ERROR  ⚠️  ",
            Style::default()
                .fg(ERROR_COLOR)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(error, Style::default().fg(TEXT_COLOR))),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to dismiss",
            Style::default().fg(MUTED_COLOR),
        )),
    ];

    let error_text = Paragraph::new(error_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ACTIVE_BORDER)
                .border_style(Style::default().fg(ERROR_COLOR))
                .padding(Padding::new(2, 2, 1, 1)),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(error_text, area);
}

fn render_input_modal<B: Backend>(f: &mut Frame<B>, app: &App) {
    let area = centered_rect(70, 20, f.size());

    // Clear the background with semi-transparent effect
    f.render_widget(Clear, area);

    // Create modal title and help text based on mode
    let (title, help_text, icon) = if matches!(app.input_mode, InputMode::InsertUrl) {
        (
            " Add Feed URL ",
            "Enter the RSS feed URL and press Enter",
            "🔗",
        )
    } else {
        (" Search ", "Enter search terms and press Enter", "🔍")
    };

    // Create an attractive input box
    let mut lines = Vec::new();

    // Add icon and title
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {}  ", icon),
            Style::default().fg(SECONDARY_COLOR),
        ),
        Span::styled(
            title,
            Style::default()
                .fg(HIGHLIGHT_COLOR)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Add help text
    lines.push(Line::from(vec![Span::styled(
        format!("  {}  ", help_text),
        Style::default().fg(MUTED_COLOR),
    )]));

    // Add spacer
    lines.push(Line::from(""));

    // Add input field with cursor
    let input_display = format!("{}█", app.input);
    lines.push(Line::from(vec![
        Span::styled("  > ", Style::default().fg(PRIMARY_COLOR)),
        Span::styled(
            input_display,
            Style::default()
                .fg(ACCENT_COLOR)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Add controls help
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  Press Enter to submit or Esc to cancel  ",
        Style::default().fg(TEXT_COLOR),
    )]));

    let input_paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(ACTIVE_BORDER)
            .border_style(Style::default().fg(PRIMARY_COLOR))
            .padding(Padding::new(2, 2, 1, 1)),
    );

    f.render_widget(input_paragraph, area);
}

// Helper function to create a centered rect using up certain percentage of the available rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Create padding effect with backdrop
    let _ = Canvas::default()
        .paint(|ctx| {
            ctx.draw(&Rectangle {
                x: 0.0,
                y: 0.0,
                width: r.width as f64,
                height: r.height as f64,
                color: Color::Rgb(0, 0, 0),
            });
        })
        .x_bounds([0.0, r.width as f64])
        .y_bounds([0.0, r.height as f64]);

    // Calculate popup dimensions
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

// Helper function to truncate a URL for display
fn truncate_url(url: &str, max_length: usize) -> String {
    // Remove common prefixes for cleaner display
    let clean_url = url
        .replace("https://", "")
        .replace("http://", "")
        .replace("www.", "");

    truncate_str(&clean_url, max_length)
}

// Helper function to truncate a string with unicode awareness
fn truncate_str(s: &str, max_chars: usize) -> String {
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

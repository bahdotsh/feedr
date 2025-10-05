use crate::app::{App, CategoryAction, InputMode, TimeFilter, View};
use html2text::from_read;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::{self},
    text::{Line, Span, Text},
    widgets::{
        canvas::{Canvas, Rectangle},
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Tabs,
        Wrap,
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

pub fn render<B: Backend>(f: &mut Frame<B>, app: &mut App) {
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
        View::CategoryManagement => render_category_management(f, app, chunks[1]),
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

    // Show filter modal when in filter mode
    if app.filter_mode {
        render_filter_modal(f, app);
    }

    // Show category input modal when in category name input mode
    if app.input_mode == InputMode::CategoryNameInput {
        render_category_input_modal(f, app);
    }
}

fn render_title_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    // Create tabs for navigation
    let titles = ["Dashboard", "Feeds", "Items", "Detail", "Categories"];
    let selected_tab = match app.view {
        View::Dashboard => 0,
        View::FeedList => 1,
        View::FeedItems => 2,
        View::FeedItemDetail => 3,
        View::CategoryManagement => 4,
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
    let mut title = if app.is_searching {
        format!(" 🔍 Search Results: '{}' ", app.search_query)
    } else {
        " 🔔 Latest Updates ".to_string()
    };

    // Add filter indicators to title if any filters are active
    if app.filter_options.is_active() {
        title = format!("{} | 🔍 Filtered", title);
    }

    // Use the filtered items when filters are active
    let items_to_display = if app.is_searching {
        &app.filtered_items
    } else if app.filter_options.is_active() {
        &app.filtered_dashboard_items
    } else {
        &app.dashboard_items
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

    if app.filter_options.is_active() && items_to_display.is_empty() {
        let mut text = Text::default();

        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            "       🔍       ",
            Style::default().fg(SECONDARY_COLOR),
        )));
        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            "No items match your current filters",
            Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
        )));
        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            app.get_filter_summary(),
            Style::default().fg(SECONDARY_COLOR),
        )));
        text.lines.push(Line::from(""));
        text.lines.push(Line::from(Span::styled(
            "Press 'f' to adjust filters or 'r' to refresh feeds",
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

    // For non-empty dashboard, create richly formatted items
    let items: Vec<ListItem> = items_to_display
        .iter()
        .enumerate()
        .map(|(idx, &(feed_idx, item_idx))| {
            let (feed, item) = if app.is_searching {
                app.search_item(idx).unwrap()
            } else {
                app.dashboard_item(idx).unwrap()
            };

            let date_str = item.formatted_date.as_deref().unwrap_or("Unknown date");
            let is_selected = app.selected_item == Some(idx);
            let is_read = app.is_item_read(feed_idx, item_idx);

            // Create clearer visual group with feed name as header
            ListItem::new(vec![
                // Feed source with icon - more prominent
                Line::from(vec![
                    Span::styled(
                        if is_selected { "► " } else { "● " },
                        Style::default().fg(if is_selected {
                            HIGHLIGHT_COLOR
                        } else if is_read {
                            MUTED_COLOR // Use muted color for read items
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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let title_text = vec![
        Line::from(Span::styled(
            "  Add your RSS/Atom feeds to get started  ",
            Style::default().fg(MUTED_COLOR),
        )),
        Line::from(Span::styled(
            "  Press 'a' to add a feed                  ",
            Style::default().fg(HIGHLIGHT_COLOR),
        )),
    ];

    let title_para = Paragraph::new(Text::from(title_text))
        .block(Block::default().borders(Borders::NONE))
        .alignment(Alignment::Left);

    f.render_widget(title_para, chunks[0]);

    if app.feeds.is_empty() {
        // Restore the stylized ASCII robot penguin
        let mut text = Text::default();

        // Stylized ASCII robot penguin
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

        f.render_widget(paragraph, chunks[1]);
        return;
    }

    // Modify to show category indicators next to feeds
    let items: Vec<ListItem> = app
        .feeds
        .iter()
        .enumerate()
        .map(|(i, feed)| {
            let mut content = vec![];

            // Add category indicator if the feed is in a category
            let category = app.get_category_for_feed(&feed.url);
            let category_tag = if let Some(cat_idx) = category {
                if cat_idx < app.categories.len() {
                    format!(" [{}]", app.categories[cat_idx].name)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Feed title with category tag
            let feed_title = format!("{}{}", feed.title, category_tag);

            if Some(i) == app.selected_feed {
                content.push(Span::styled(
                    format!("▶ {}", feed_title),
                    Style::default()
                        .fg(HIGHLIGHT_COLOR)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                content.push(Span::styled(
                    format!("  {}", feed_title),
                    Style::default().fg(TEXT_COLOR),
                ));
            }

            let domain = extract_domain(&feed.url);
            content.push(Span::styled(
                format!(" ({})", domain),
                Style::default().fg(MUTED_COLOR),
            ));

            ListItem::new(Line::from(content))
        })
        .collect();

    let feeds = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(NORMAL_BORDER)
                .title(" 📋 Feeds ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(PRIMARY_COLOR)),
        )
        .highlight_style(
            Style::default()
                .bg(PRIMARY_COLOR)
                .fg(TEXT_COLOR)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ► ");

    // Create a mutable ListState to track selection
    let mut list_state = ListState::default();
    list_state.select(app.selected_feed);

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
                let is_selected = app.selected_item == Some(idx);
                let is_read = app
                    .selected_feed
                    .is_some_and(|feed_idx| app.is_item_read(feed_idx, idx));

                // More distinct selection indicators
                let (title_color, bullet_icon, bullet_color) = if is_selected {
                    (HIGHLIGHT_COLOR, "►", PRIMARY_COLOR)
                } else if is_read {
                    (MUTED_COLOR, "•", MUTED_COLOR) // Muted for read items
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
                    truncate_str(&clean_text, 80)
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

fn render_item_detail<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
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

        // Add read status with icon
        if let (Some(feed_idx), Some(item_idx)) = (app.selected_feed, app.selected_item) {
            let is_read = app.is_item_read(feed_idx, item_idx);
            header_lines.push(Line::from(vec![
                Span::styled("  ", Style::default().fg(MUTED_COLOR)),
                Span::styled(
                    if is_read { "✓ " } else { "✗ " },
                    Style::default().fg(if is_read {
                        HIGHLIGHT_COLOR
                    } else {
                        MUTED_COLOR
                    }),
                ),
                Span::styled("Status: ", Style::default().fg(SECONDARY_COLOR)),
                Span::styled(
                    if is_read { "Read" } else { "Unread" },
                    Style::default().fg(if is_read {
                        HIGHLIGHT_COLOR
                    } else {
                        MUTED_COLOR
                    }),
                ),
            ]));
        }
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

        // Calculate the viewport height (accounting for borders and padding)
        let viewport_height = chunks[1]
            .height
            .saturating_sub(2) // borders (top and bottom)
            .saturating_sub(2); // padding (top and bottom)

        // Calculate the content width (accounting for borders and padding)
        let content_width = chunks[1]
            .width
            .saturating_sub(2) // borders (left and right)
            .saturating_sub(2) // padding (left and right)
            as usize;

        // Calculate the number of lines the wrapped content will take
        let content_lines = count_wrapped_lines(&description, content_width);

        // Update the max scroll value
        app.update_detail_max_scroll(content_lines, viewport_height);
        app.clamp_detail_scroll();

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
            .scroll((app.detail_vertical_scroll, 0))
            .wrap(Wrap { trim: true });

        f.render_widget(content, chunks[1]);
    }
}

fn render_help_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    // Match on the input mode and view to determine the help text and style
    let (help_text, _style) = match app.input_mode {
        InputMode::Normal => {
            let help_text = match app.view {
                View::Dashboard => {
                    if app.feeds.is_empty() {
                        "a: Add feed | q: Quit | CTRL+C: Manage categories"
                    } else {
                        "↑/↓: Navigate | ENTER: View feed | a: Add feed | r: Refresh | f: Filter | /: Search | q: Quit"
                    }
                }
                View::FeedList => {
                    if app.feeds.is_empty() {
                        "a: Add feed | q: Quit | TAB: Dashboard | CTRL+C: Categories"
                    } else {
                        "a: Add feed | c: Assign to category | DEL: Remove feed | ENTER: Open | q: Quit | CTRL+C: Categories"
                    }
                }
                View::CategoryManagement => {
                    "n: New category | e: Edit | d: Delete | SPACE: Toggle feeds | c: Add selected feed | ESC/q: Back"
                }
                View::FeedItems => {
                    "h/esc: back to feeds | home: dashboard | enter: view detail | o: open link | /: search | q: quit"
                }
                View::FeedItemDetail => {
                    "h/esc: back | home: dashboard | o: open in browser | ↑/↓: scroll | PgUp/PgDn: fast scroll | q: quit"
                }
            };
            (help_text, Style::default().fg(TEXT_COLOR))
        }
        InputMode::InsertUrl => (
            "Enter feed URL (e.g., https://news.ycombinator.com/rss)",
            Style::default().fg(HIGHLIGHT_COLOR),
        ),
        InputMode::SearchMode => (
            "Enter search term (press ENTER to search)",
            Style::default().fg(HIGHLIGHT_COLOR),
        ),
        InputMode::FilterMode => ("", Style::default().fg(MUTED_COLOR)),
        InputMode::CategoryNameInput => ("", Style::default().fg(MUTED_COLOR)),
    };

    // Only show help bar in normal mode
    if matches!(app.input_mode, InputMode::Normal) {
        // Create a stylized help bar with visually separated commands
        let parts: Vec<&str> = help_text.split('|').collect();
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

fn render_filter_modal<B: Backend>(f: &mut Frame<B>, app: &App) {
    let area = centered_rect(70, 60, f.size());

    // Clear the area
    f.render_widget(Clear, area);

    // Create filter selection UI
    let mut text = vec![
        // Header
        Line::from(vec![
            Span::styled("  🔍  ", Style::default().fg(PRIMARY_COLOR)),
            Span::styled(
                "Feed Filters",
                Style::default()
                    .fg(HIGHLIGHT_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("  Select filters to apply to your feed items:"),
        Line::from(""),
    ];

    // Category filter
    let available_categories = app.get_available_categories();
    let category_status = match &app.filter_options.category {
        Some(cat) => format!("[{}]", cat),
        None => "[Off]".to_string(),
    };

    text.push(Line::from(vec![
        Span::styled("  c - Category: ", Style::default().fg(TEXT_COLOR)),
        Span::styled(
            category_status,
            Style::default().fg(if app.filter_options.category.is_some() {
                HIGHLIGHT_COLOR
            } else {
                MUTED_COLOR
            }),
        ),
        Span::styled(
            if !available_categories.is_empty() {
                format!(" ({})", available_categories.join(", "))
            } else {
                "".to_string()
            },
            Style::default().fg(MUTED_COLOR),
        ),
    ]));

    // Age filter
    let age_status = match &app.filter_options.age {
        Some(age) => {
            let age_str = match age {
                TimeFilter::Today => "Today",
                TimeFilter::ThisWeek => "This Week",
                TimeFilter::ThisMonth => "This Month",
                TimeFilter::Older => "Older",
            };
            format!("[{}]", age_str)
        }
        None => "[Off]".to_string(),
    };

    text.push(Line::from(vec![
        Span::styled("  t - Time/Age: ", Style::default().fg(TEXT_COLOR)),
        Span::styled(
            age_status,
            Style::default().fg(if app.filter_options.age.is_some() {
                HIGHLIGHT_COLOR
            } else {
                MUTED_COLOR
            }),
        ),
    ]));

    // Author filter
    let author_status = match app.filter_options.has_author {
        Some(true) => "[With author]",
        Some(false) => "[No author]",
        None => "[Off]",
    };

    text.push(Line::from(vec![
        Span::styled("  a - Author: ", Style::default().fg(TEXT_COLOR)),
        Span::styled(
            author_status,
            Style::default().fg(if app.filter_options.has_author.is_some() {
                HIGHLIGHT_COLOR
            } else {
                MUTED_COLOR
            }),
        ),
    ]));

    // Read status filter
    let read_status = match app.filter_options.read_status {
        Some(true) => "[Read]",
        Some(false) => "[Unread]",
        None => "[Off]",
    };

    text.push(Line::from(vec![
        Span::styled("  r - Read status: ", Style::default().fg(TEXT_COLOR)),
        Span::styled(
            read_status,
            Style::default().fg(if app.filter_options.read_status.is_some() {
                HIGHLIGHT_COLOR
            } else {
                MUTED_COLOR
            }),
        ),
    ]));

    // Length filter
    let length_status = match app.filter_options.min_length {
        Some(100) => "[Short]",
        Some(500) => "[Medium]",
        Some(1000) => "[Long]",
        Some(n) => &format!("[{} chars]", n),
        None => "[Off]",
    };

    text.push(Line::from(vec![
        Span::styled("  l - Length: ", Style::default().fg(TEXT_COLOR)),
        Span::styled(
            length_status,
            Style::default().fg(if app.filter_options.min_length.is_some() {
                HIGHLIGHT_COLOR
            } else {
                MUTED_COLOR
            }),
        ),
    ]));

    // Clear filters option
    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled("  x - ", Style::default().fg(TEXT_COLOR)),
        Span::styled("Clear all filters", Style::default().fg(ERROR_COLOR)),
    ]));

    text.push(Line::from(""));
    text.push(Line::from(""));

    // Update the filter statistics
    let (active_count, filtered_count, total_count) = app.get_filter_stats();

    text.push(Line::from(vec![Span::styled(
        format!(
            "  Active Filters: {}/5  |  Showing: {}/{} items",
            active_count, filtered_count, total_count
        ),
        Style::default().fg(MUTED_COLOR),
    )]));

    // Add the filter summary
    if active_count > 0 {
        text.push(Line::from(""));
        text.push(Line::from(vec![Span::styled(
            format!("  Current filters: {}", app.get_filter_summary()),
            Style::default().fg(SECONDARY_COLOR),
        )]));
    }

    text.push(Line::from(""));
    text.push(Line::from(vec![Span::styled(
        "  Press Esc to close this dialog",
        Style::default().fg(TEXT_COLOR),
    )]));

    let filter_paragraph = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(ACTIVE_BORDER)
            .border_style(Style::default().fg(PRIMARY_COLOR))
            .title(" Filter Options ")
            .title_alignment(Alignment::Center)
            .padding(Padding::new(2, 2, 1, 1)),
    );

    f.render_widget(filter_paragraph, area);
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

// Helper function to count the number of lines when text is wrapped
fn count_wrapped_lines(text: &str, width: usize) -> u16 {
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
                let wrapped_lines = ((line_width + width - 1) / width).max(1);
                line_count = line_count.saturating_add(wrapped_lines as u16);
            }
        }
    }

    // If text doesn't end with newline, we still have the lines we counted
    // If text is empty, return at least 1 line
    line_count.max(1)
}

// Update the render_category_management function to show feeds when a category is expanded
fn render_category_management<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(3),    // Category list
            Constraint::Length(5), // Help text
        ])
        .split(area);

    // Add a title block
    let title = match &app.category_action {
        Some(CategoryAction::AddFeedToCategory(url)) => {
            // Show which feed is being assigned to a category
            let feed_idx = app.feeds.iter().position(|f| f.url == *url);
            let feed_title = feed_idx
                .and_then(|idx| app.feeds.get(idx))
                .map_or("Unknown Feed", |feed| feed.title.as_str());
            format!(" 📂 Add '{}' to Category ", truncate_str(feed_title, 30))
        }
        _ => " 📂 Category Management ".to_string(),
    };

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_type(NORMAL_BORDER)
        .title(title)
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(PRIMARY_COLOR));

    f.render_widget(title_block, chunks[0]);

    // Prepare list items for categories and their feeds
    let mut list_items = Vec::new();
    let mut list_indices = Vec::new(); // To map UI index to category index

    if app.categories.is_empty() {
        list_items.push(ListItem::new(Line::from(Span::styled(
            "No categories yet. Press 'n' to create a new category.",
            Style::default().fg(MUTED_COLOR),
        ))));
    } else {
        for (cat_idx, category) in app.categories.iter().enumerate() {
            // Add category to the list
            let icon = if category.expanded { "▼" } else { "▶" };
            let feed_count = category.feed_count();
            let count_text = if feed_count == 1 {
                "1 feed".to_string()
            } else {
                format!("{} feeds", feed_count)
            };

            let style = if Some(cat_idx) == app.selected_category {
                Style::default()
                    .fg(HIGHLIGHT_COLOR)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT_COLOR)
            };

            list_items.push(ListItem::new(Line::from(Span::styled(
                format!("{} {} ({})", icon, category.name, count_text),
                style,
            ))));
            list_indices.push(Some(cat_idx));

            // If category is expanded, show its feeds
            if category.expanded {
                let feeds_in_category = app
                    .feeds
                    .iter()
                    .enumerate()
                    .filter(|(_, feed)| category.contains_feed(&feed.url))
                    .collect::<Vec<_>>();

                for (feed_idx, feed) in &feeds_in_category {
                    let feed_style = if Some(*feed_idx) == app.selected_feed {
                        Style::default().fg(ACCENT_COLOR)
                    } else {
                        Style::default().fg(MUTED_COLOR)
                    };

                    list_items.push(ListItem::new(Line::from(Span::styled(
                        format!("   → {}", truncate_str(&feed.title, 40)),
                        feed_style,
                    ))));
                    list_indices.push(None); // None means this is a feed, not a category
                }

                // Show a message if the category is empty
                if feeds_in_category.is_empty() {
                    list_items.push(ListItem::new(Line::from(Span::styled(
                        "   (No feeds in this category)",
                        Style::default().fg(MUTED_COLOR),
                    ))));
                    list_indices.push(None);
                }
            }
        }
    }

    let categories_list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(NORMAL_BORDER)
                .title(" Categories ")
                .border_style(Style::default().fg(SECONDARY_COLOR)),
        )
        .highlight_style(
            Style::default()
                .bg(PRIMARY_COLOR)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );

    // Create a mutable ListState based on the selected category
    let mut list_state = ListState::default();
    if let Some(selected_idx) = app.selected_category {
        // Find the corresponding index in the UI list (may differ due to expanded feeds)
        if let Some(ui_idx) = list_indices
            .iter()
            .position(|&cat_idx| cat_idx == Some(selected_idx))
        {
            list_state.select(Some(ui_idx));
        }
    }

    f.render_stateful_widget(categories_list, chunks[1], &mut list_state);

    // Render help text
    let help_text = if let Some(CategoryAction::AddFeedToCategory(_)) = &app.category_action {
        "ENTER: Add to category | ESC/q: Cancel | UP/DOWN: Navigate"
    } else {
        "n: New category | e: Edit | d: Delete | SPACE: Toggle feeds | c: Add selected feed | ESC/q: Back"
    };

    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_type(NORMAL_BORDER)
        .title(" Controls ")
        .border_style(Style::default().fg(MUTED_COLOR));

    let help_para = Paragraph::new(help_text)
        .block(help_block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(help_para, chunks[2]);
}

// Add a new function to render the category name input modal
fn render_category_input_modal<B: Backend>(f: &mut Frame<B>, app: &App) {
    let area = centered_rect(60, 20, f.size());

    // Clear the area behind the modal
    f.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Input field
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Help text
        ])
        .split(area);

    // Determine title based on the current action
    let title = match &app.category_action {
        Some(CategoryAction::Create) => " Create New Category ",
        Some(CategoryAction::Rename(_)) => " Rename Category ",
        _ => " Category Name ",
    };

    // Create title block
    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(PRIMARY_COLOR));

    f.render_widget(title_block, chunks[0]);

    // Create input field
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(" Name ")
        .border_style(Style::default().fg(SECONDARY_COLOR));

    let input_text = Paragraph::new(app.input.as_str())
        .block(input_block)
        .style(Style::default().fg(HIGHLIGHT_COLOR));

    f.render_widget(input_text, chunks[1]);

    // Position cursor at the end of input
    let cursor_x = app.input.width() as u16 + chunks[1].x + 1; // +1 for border
    let cursor_y = chunks[1].y + 1;
    f.set_cursor(cursor_x, cursor_y);

    // Help text
    let help_block = Block::default()
        .borders(Borders::ALL)
        .title(" Controls ")
        .border_style(Style::default().fg(MUTED_COLOR));

    let help_text = "ENTER: Confirm | ESC: Cancel";
    let help_para = Paragraph::new(help_text)
        .block(help_block)
        .alignment(Alignment::Center);

    f.render_widget(help_para, chunks[3]);
}

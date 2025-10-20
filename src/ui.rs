use crate::app::{App, CategoryAction, InputMode, TimeFilter, View};
use crate::config::Theme;
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

/// Color scheme for the UI - supports both light and dark themes with distinct personalities
#[derive(Clone, Debug)]
pub struct ColorScheme {
    pub primary: Color,
    pub secondary: Color,
    pub highlight: Color,
    pub success: Color,
    pub background: Color,
    pub surface: Color,
    pub selected_bg: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub muted: Color,
    pub accent: Color,
    pub error: Color,
    pub border: Color,
    pub border_focus: Color,
    // Theme-specific border styles
    pub border_normal: BorderType,
    pub border_active: BorderType,
    pub border_focus_type: BorderType,
}

impl ColorScheme {
    /// Dark theme - Cyberpunk/Neon aesthetic with high contrast and futuristic vibes
    pub fn dark() -> Self {
        Self {
            primary: Color::Rgb(0, 217, 255),          // Electric cyan
            secondary: Color::Rgb(255, 0, 255),        // Vivid magenta
            highlight: Color::Rgb(0, 255, 157),        // Neon green
            success: Color::Rgb(57, 255, 20),          // Bright neon green
            background: Color::Rgb(10, 10, 10),        // Deep black
            surface: Color::Rgb(15, 20, 25),           // Very dark with blue tint
            selected_bg: Color::Rgb(30, 30, 50),       // Dark blue-purple
            text: Color::Rgb(255, 255, 255),           // Pure white
            text_secondary: Color::Rgb(150, 200, 255), // Light cyan
            muted: Color::Rgb(100, 100, 120),          // Muted blue-gray
            accent: Color::Rgb(255, 215, 0),           // Electric gold
            error: Color::Rgb(255, 20, 147),           // Hot pink
            border: Color::Rgb(80, 80, 120),           // Blue-tinted border
            border_focus: Color::Rgb(0, 217, 255),     // Electric cyan focus
            border_normal: BorderType::Double,
            border_active: BorderType::Double,
            border_focus_type: BorderType::Thick,
        }
    }

    /// Light theme - Minimal/Zen aesthetic with soft natural colors and organic simplicity
    pub fn light() -> Self {
        Self {
            primary: Color::Rgb(92, 138, 126),         // Soft sage green
            secondary: Color::Rgb(201, 112, 100),      // Warm terracotta
            highlight: Color::Rgb(218, 165, 32),       // Gentle amber gold
            success: Color::Rgb(106, 153, 85),         // Muted sage
            background: Color::Rgb(250, 248, 245),     // Warm off-white
            surface: Color::Rgb(255, 255, 252),        // Cream white
            selected_bg: Color::Rgb(237, 231, 220),    // Soft beige selection
            text: Color::Rgb(60, 50, 40),              // Warm dark brown
            text_secondary: Color::Rgb(120, 110, 100), // Medium warm gray
            muted: Color::Rgb(170, 165, 155),          // Muted stone gray
            accent: Color::Rgb(184, 134, 100),         // Natural wood brown
            error: Color::Rgb(180, 80, 70),            // Soft clay red
            border: Color::Rgb(200, 195, 185),         // Subtle warm border
            border_focus: Color::Rgb(92, 138, 126),    // Sage focus
            border_normal: BorderType::Rounded,
            border_active: BorderType::Rounded,
            border_focus_type: BorderType::Rounded,
        }
    }

    /// Get the color scheme for the given theme
    pub fn from_theme(theme: &Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }

    /// Get theme-specific list bullet symbol
    pub fn get_list_bullet(&self) -> &str {
        if self.border_normal == BorderType::Double {
            "◆" // Dark theme: tech diamond
        } else {
            "◦" // Light theme: minimal circle
        }
    }

    /// Get theme-specific arrow right symbol
    pub fn get_arrow_right(&self) -> &str {
        if self.border_normal == BorderType::Double {
            "▸" // Dark theme: futuristic arrow
        } else {
            "›" // Light theme: minimal arrow
        }
    }

    /// Get theme-specific selection indicator
    pub fn get_selection_indicator(&self) -> &str {
        if self.border_normal == BorderType::Double {
            "▶" // Dark theme: solid arrow
        } else {
            "•" // Light theme: simple bullet
        }
    }

    /// Get theme-specific loading animation frames
    pub fn get_loading_frames(&self) -> Vec<&str> {
        if self.border_normal == BorderType::Double {
            // Dark theme: Tech/cyber loading
            vec!["◢", "◣", "◤", "◥", "◢", "◣", "◤", "◥", "◢", "◣"]
        } else {
            // Light theme: Minimal loading
            vec!["⋯", "⋰", "⋱", "⋯", "⋰", "⋱", "⋯", "⋰", "⋱", "⋯"]
        }
    }

    /// Get theme-specific empty feed ASCII art
    pub fn get_empty_feed_art(&self) -> Vec<String> {
        if self.border_normal == BorderType::Double {
            // Dark theme: Cyberpunk terminal
            vec![
                "                                           ".to_string(),
                "       ╔═══════════════════╗               ".to_string(),
                "       ║  ◢◣  C Y B E R  ◤◥  ║               ".to_string(),
                "       ║   ═══════════════  ║               ".to_string(),
                "       ║   > NO_SIGNAL_    ║               ".to_string(),
                "       ║   > INIT_FEED...  ║               ".to_string(),
                "       ╚═══════════════════╝               ".to_string(),
                "                                           ".to_string(),
            ]
        } else {
            // Light theme: Zen garden with simple plant
            vec![
                "                                           ".to_string(),
                "              _                            ".to_string(),
                "             ( )                           ".to_string(),
                "              |                            ".to_string(),
                "             / \\                           ".to_string(),
                "            /   \\                          ".to_string(),
                "           -------                         ".to_string(),
                "                                           ".to_string(),
            ]
        }
    }

    /// Get theme-specific dashboard welcome art
    pub fn get_dashboard_art(&self) -> Vec<String> {
        if self.border_normal == BorderType::Double {
            // Dark theme: Cyberpunk glitch aesthetic
            vec![
                "                                                ".to_string(),
                "  ███████╗███████╗███████╗██████╗ ██████╗      ".to_string(),
                "  ██╔════╝██╔════╝██╔════╝██╔══██╗██╔══██╗     ".to_string(),
                "  █████╗  █████╗  █████╗  ██║  ██║██████╔╝     ".to_string(),
                "  ██╔══╝  ██╔══╝  ██╔══╝  ██║  ██║██╔══██╗     ".to_string(),
                "  ██║     ███████╗███████╗██████╔╝██║  ██║     ".to_string(),
                "  ╚═╝     ╚══════╝╚══════╝╚═════╝ ╚═╝  ╚═╝     ".to_string(),
                "  ═══════════════════════════════════════════  ".to_string(),
                "  ◢◣ NEURAL FEED INTERFACE v2.0 ◤◥             ".to_string(),
                "  ═══════════════════════════════════════════  ".to_string(),
                "                                                ".to_string(),
                "  ▸ INITIALIZE: Press 'a' to add feed URL       ".to_string(),
                "  ▸ CONNECT TO DATA STREAMS                     ".to_string(),
                "                                                ".to_string(),
            ]
        } else {
            // Light theme: Zen minimalist
            vec![
                "                                                ".to_string(),
                "                                                ".to_string(),
                "            F  e  e  d  r                      ".to_string(),
                "                                                ".to_string(),
                "         ─────────────────                     ".to_string(),
                "                                                ".to_string(),
                "         A mindful RSS reader                   ".to_string(),
                "                                                ".to_string(),
                "                                                ".to_string(),
                "  🍃  Begin by adding your first feed           ".to_string(),
                "       Press 'a' to add a feed URL              ".to_string(),
                "                                                ".to_string(),
                "                                                ".to_string(),
            ]
        }
    }

    /// Get theme-specific icon prefix
    pub fn get_icon_feed(&self) -> &str {
        if self.border_normal == BorderType::Double {
            "◈" // Dark: tech diamond
        } else {
            "🍃" // Light: leaf
        }
    }

    pub fn get_icon_article(&self) -> &str {
        if self.border_normal == BorderType::Double {
            "◇" // Dark: hollow diamond
        } else {
            "📄" // Light: paper
        }
    }

    pub fn get_icon_search(&self) -> &str {
        if self.border_normal == BorderType::Double {
            "◎" // Dark: target
        } else {
            "🔍" // Light: magnifying glass
        }
    }

    pub fn get_icon_dashboard(&self) -> &str {
        if self.border_normal == BorderType::Double {
            "◢◣" // Dark: tech brackets
        } else {
            "☀️" // Light: sun
        }
    }

    pub fn get_icon_error(&self) -> &str {
        "⚠" // Universal warning icon for both themes
    }

    pub fn get_icon_success(&self) -> &str {
        "✓" // Universal checkmark for both themes
    }
}

pub fn render<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    // Get the color scheme based on the current theme
    let colors = ColorScheme::from_theme(&app.config.ui.theme);

    // Set background color for the entire terminal
    let bg_block = Block::default().style(Style::default().bg(colors.background));
    f.render_widget(bg_block, f.size());

    // Main layout division with better spacing
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(4), // Title/tab bar (slightly taller)
            Constraint::Min(0),    // Main content
            Constraint::Length(4), // Help bar (slightly taller for breathing room)
        ])
        .split(f.size());

    render_title_bar(f, app, chunks[0], &colors);

    match app.view {
        View::Dashboard => render_dashboard(f, app, chunks[1], &colors),
        View::FeedList => render_feed_list(f, app, chunks[1], &colors),
        View::FeedItems => render_feed_items(f, app, chunks[1], &colors),
        View::FeedItemDetail => render_item_detail(f, app, chunks[1], &colors),
        View::CategoryManagement => render_category_management(f, app, chunks[1], &colors),
    }

    render_help_bar(f, app, chunks[2], &colors);

    // Show error if present
    if let Some(error) = &app.error {
        render_error_modal(f, error, &colors);
    }

    // Show success notification if present
    if let Some(success) = &app.success_message {
        render_success_notification(f, success, &colors);
    }

    // Show input modal when in input modes
    if matches!(app.input_mode, InputMode::InsertUrl | InputMode::SearchMode) {
        render_input_modal(f, app, &colors);
    }

    // Show filter modal when in filter mode
    if app.filter_mode {
        render_filter_modal(f, app, &colors);
    }

    // Show category input modal when in category name input mode
    if app.input_mode == InputMode::CategoryNameInput {
        render_category_input_modal(f, app, &colors);
    }
}

fn render_title_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect, colors: &ColorScheme) {
    // Create tabs for navigation
    let titles = ["Dashboard", "Feeds", "Items", "Detail", "Categories"];
    let selected_tab = match app.view {
        View::Dashboard => 0,
        View::FeedList => 1,
        View::FeedItems => 2,
        View::FeedItemDetail => 3,
        View::CategoryManagement => 4,
    };

    // Theme-specific loading animation
    let loading_symbols = colors.get_loading_frames();

    // Create title with loading indicator if loading
    let title = if app.is_loading {
        format!(
            " {} Refreshing feeds... ",
            loading_symbols[app.loading_indicator % loading_symbols.len()]
        )
    } else {
        format!(" {} Feedr ", colors.get_icon_dashboard())
    };

    // Create tab highlight effect with theme-specific indicators
    let selection_indicator = colors.get_selection_indicator();
    let tabs = Tabs::new(
        titles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let prefix = if i == selected_tab {
                    format!("{} ", selection_indicator)
                } else {
                    "  ".to_string()
                };
                Line::from(vec![Span::styled(
                    format!("{}{}", prefix, t),
                    if i == selected_tab {
                        Style::default()
                            .fg(colors.highlight)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(colors.text_secondary)
                    },
                )])
            })
            .collect(),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(if app.is_loading {
                colors.border_active
            } else {
                colors.border_normal
            })
            .border_style(Style::default().fg(if app.is_loading {
                colors.highlight
            } else {
                colors.border
            }))
            .title(title)
            .title_alignment(Alignment::Center)
            .padding(Padding::new(2, 2, 0, 0)),
    )
    .style(
        Style::default()
            .fg(colors.text_secondary)
            .bg(colors.surface),
    )
    .select(selected_tab)
    .divider(symbols::line::VERTICAL);

    f.render_widget(tabs, area);
}

fn render_dashboard<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect, colors: &ColorScheme) {
    let search_icon = colors.get_icon_search();
    let mut title = if app.is_searching {
        format!(" {} Search Results: '{}' ", search_icon, app.search_query)
    } else {
        format!(" {} Latest Updates ", colors.get_icon_dashboard())
    };

    // Add filter indicators to title if any filters are active
    if app.filter_options.is_active() {
        title = format!("{} | {} Filtered", title, search_icon);
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
    let items: Vec<ListItem> = items_to_display
        .iter()
        .enumerate()
        .map(|(idx, &(feed_idx, item_idx))| {
             let (feed, item) = if app.is_searching {
                app.search_item(idx).unwrap()
            } else {
                (&app.feeds[feed_idx], &app.feeds[feed_idx].items[item_idx])
            };

        let date_str = item.formatted_date.as_deref().unwrap_or("Unknown date");
            let is_selected = app.selected_item == Some(idx);
            let is_read = app.is_item_read(feed_idx, item_idx);

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
        })
        .collect();

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

    f.render_stateful_widget(dashboard_list, area, &mut state);
}

fn render_feed_list<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect, colors: &ColorScheme) {
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

    if app.feeds.is_empty() {
        // Theme-specific empty feed ASCII art
        let mut text = Text::default();
        let art_lines = colors.get_empty_feed_art();

        for line in &art_lines {
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
                    text.lines.push(Line::from(line.as_str()));
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
                    text.lines.push(Line::from(line.as_str()));
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

    // Modify to show category indicators next to feeds
    let items: Vec<ListItem> = app
        .feeds
        .iter()
        .enumerate()
        .map(|(i, feed)| {
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

            // Feed title with category tag and domain
            let feed_title = format!("{}{}", feed.title, category_tag);
            let domain = extract_domain(&feed.url);
            let domain_text = format!(" · {}", domain);

            // Build the spans
            let title_style = Style::default()
                .fg(if Some(i) == app.selected_feed {
                    colors.text
                } else {
                    colors.text_secondary
                })
                .add_modifier(if Some(i) == app.selected_feed {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });

            let content = vec![
                Span::styled(feed_title, title_style),
                Span::styled(domain_text, Style::default().fg(colors.muted)),
            ];

            ListItem::new(Line::from(content))
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

fn render_feed_items<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect, colors: &ColorScheme) {
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

                // Better formatted snippet with HTML cleanup
                let snippet = if let Some(desc) = &item.description {
                    let plain_text = html2text::from_read(desc.as_bytes(), 50);
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

fn render_item_detail<B: Backend>(
    f: &mut Frame<B>,
    app: &mut App,
    area: Rect,
    colors: &ColorScheme,
) {
    if let Some(item) = app.current_item() {
        // Split the area into header and content with better proportions
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(9), // Header - increased for better spacing
                Constraint::Min(0),    // Content
            ])
            .split(area);

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

        // Process content with enhanced formatting
        let description = if let Some(desc) = &item.description {
            // Convert HTML to plain text with better width for readability
            let raw_text = from_read(desc.as_bytes(), 100);
            format_content_for_reading(&raw_text)
        } else {
            "No description available".to_string()
        };

        // Calculate the viewport height (accounting for borders and padding)
        let viewport_height = chunks[1]
            .height
            .saturating_sub(2) // borders (top and bottom)
            .saturating_sub(4); // increased padding (top and bottom)

        // Calculate the content width (accounting for borders and padding)
        let content_width = chunks[1]
            .width
            .saturating_sub(2) // borders (left and right)
            .saturating_sub(8) // increased padding for better reading width
            as usize;

        // Calculate the number of lines the wrapped content will take
        let content_lines = count_wrapped_lines(&description, content_width);

        // Update the max scroll value
        app.update_detail_max_scroll(content_lines, viewport_height);
        app.clamp_detail_scroll();

        // Create theme-specific scroll indicator
        let scroll_arrows = if colors.border_normal == BorderType::Double {
            ("▼", "▲") // Dark: solid arrows
        } else {
            ("↓", "↑") // Light: simple arrows
        };

        let scroll_indicator = if app.detail_max_scroll > 0 {
            let scroll_pct =
                (app.detail_vertical_scroll as f32 / app.detail_max_scroll as f32 * 100.0) as u16;
            if app.detail_vertical_scroll == 0 {
                format!(
                    " {} Article Content · Scroll {} for more ",
                    article_icon, scroll_arrows.0
                )
            } else if app.detail_vertical_scroll >= app.detail_max_scroll {
                format!(" {} Article Content · End of article ", article_icon)
            } else {
                format!(" {} Article Content · {}% ", article_icon, scroll_pct)
            }
        } else {
            format!(" {} Article Content ", article_icon)
        };

        // Create content paragraph with theme-specific styling
        let content = Paragraph::new(description)
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

        f.render_widget(content, chunks[1]);
    }
}

fn render_help_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect, colors: &ColorScheme) {
    // Match on the input mode and view to determine the help text and style
    let (help_text, _style) = match app.input_mode {
        InputMode::Normal => {
            let help_text = match app.view {
                View::Dashboard => {
                    if app.feeds.is_empty() {
                        "a: Add feed | t: Theme | q: Quit | CTRL+C: Manage categories"
                    } else {
                        "↑/↓: Navigate | ENTER: View | Space: Toggle read | a: Add feed | r: Refresh | f: Filter | /: Search | t: Theme | q: Quit"
                    }
                }
                View::FeedList => {
                    if app.feeds.is_empty() {
                        "a: Add feed | t: Theme | q: Quit | TAB: Dashboard | CTRL+C: Categories"
                    } else {
                        "a: Add feed | c: Assign to category | DEL: Remove feed | ENTER: Open | t: Theme | q: Quit | CTRL+C: Categories"
                    }
                }
                View::CategoryManagement => {
                    "n: New category | e: Edit | d: Delete | SPACE: Toggle feeds | c: Add selected feed | t: Theme | ESC/q: Back"
                }
                View::FeedItems => {
                    "h/esc: back | home: dashboard | enter: view | Space: Toggle read | o: open | /: search | t: theme | q: quit"
                }
                View::FeedItemDetail => {
                    "h/esc: back | home: dashboard | ↑/↓: scroll | PgUp/PgDn: fast | Space: Toggle read | o: open | t: theme | q: quit"
                }
            };
            (help_text, Style::default().fg(colors.text))
        }
        InputMode::InsertUrl => (
            "Enter feed URL (e.g., https://news.ycombinator.com/rss)",
            Style::default().fg(colors.highlight),
        ),
        InputMode::SearchMode => (
            "Enter search term (press ENTER to search)",
            Style::default().fg(colors.highlight),
        ),
        InputMode::FilterMode => ("", Style::default().fg(colors.muted)),
        InputMode::CategoryNameInput => ("", Style::default().fg(colors.muted)),
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
                        .fg(colors.highlight)
                        .add_modifier(Modifier::BOLD),
                ));

                // Add the description in normal text color
                spans.push(Span::styled(desc, Style::default().fg(colors.text)));
            } else {
                spans.push(Span::styled(trimmed, Style::default().fg(colors.text)));
            }

            // Add separator unless this is the last item
            if idx < parts.len() - 1 {
                spans.push(Span::styled(" | ", Style::default().fg(colors.border)));
            }
        }

        let command_icon = if colors.border_normal == BorderType::Double {
            "◈" // Dark: tech diamond
        } else {
            "💡" // Light: lightbulb
        };

        let help = Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(colors.border_normal)
                    .border_style(Style::default().fg(colors.border))
                    .style(Style::default().bg(colors.surface))
                    .title(format!(" {} Commands ", command_icon))
                    .title_alignment(Alignment::Center)
                    .padding(Padding::new(1, 1, 0, 0)),
            );
        f.render_widget(help, area);
    }
}

fn render_error_modal<B: Backend>(f: &mut Frame<B>, error: &str, colors: &ColorScheme) {
    let area = centered_rect(60, 30, f.size());

    // Clear the background
    f.render_widget(Clear, area);

    // Theme-specific error icon
    let error_icon = colors.get_icon_error();

    // Create a theme-specific error modal
    let error_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("{} Error", error_icon),
            Style::default()
                .fg(colors.error)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(error, Style::default().fg(colors.text))),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to dismiss",
            Style::default().fg(colors.text_secondary),
        )),
    ];

    let error_text = Paragraph::new(error_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(colors.border_focus_type)
                .border_style(Style::default().fg(colors.error))
                .style(Style::default().bg(colors.surface))
                .padding(Padding::new(3, 3, 2, 2)),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(error_text, area);
}

fn render_success_notification<B: Backend>(f: &mut Frame<B>, message: &str, colors: &ColorScheme) {
    // Create a theme-specific notification in the top-right corner
    let success_icon = colors.get_icon_success();
    let msg_width = (message.len() + 6).min(50) as u16;
    let area = Rect {
        x: f.size().width.saturating_sub(msg_width + 2),
        y: 2,
        width: msg_width.min(f.size().width),
        height: 3,
    };

    // Clear the background
    f.render_widget(Clear, area);

    // Create a theme-specific success notification
    let success_text = Paragraph::new(Line::from(vec![Span::styled(
        format!(" {} {} ", success_icon, message),
        Style::default()
            .fg(colors.success)
            .add_modifier(Modifier::BOLD),
    )]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(colors.border_normal)
            .border_style(Style::default().fg(colors.success))
            .style(Style::default().bg(colors.surface)),
    )
    .alignment(Alignment::Center);

    f.render_widget(success_text, area);
}

fn render_input_modal<B: Backend>(f: &mut Frame<B>, app: &App, colors: &ColorScheme) {
    let area = centered_rect(70, 25, f.size());

    // Clear the background
    f.render_widget(Clear, area);

    // Create modal title and help text with theme-specific icons
    let (title, help_text, icon) = if matches!(app.input_mode, InputMode::InsertUrl) {
        let link_icon = if colors.border_normal == BorderType::Double {
            "◎" // Dark: target/link
        } else {
            "🔗" // Light: link
        };
        (
            "Add Feed URL",
            "Enter the RSS feed URL and press Enter",
            link_icon,
        )
    } else {
        (
            "Search",
            "Enter search terms and press Enter",
            colors.get_icon_search(),
        )
    };

    // Create a modern input modal
    let mut lines = Vec::new();

    // Add title with icon
    lines.push(Line::from(vec![Span::styled(
        format!("{} {}", icon, title),
        Style::default()
            .fg(colors.text)
            .add_modifier(Modifier::BOLD),
    )]));

    // Add separator
    lines.push(Line::from(""));

    // Add help text
    lines.push(Line::from(vec![Span::styled(
        help_text,
        Style::default().fg(colors.text_secondary),
    )]));

    // Add spacer
    lines.push(Line::from(""));

    // Add input field with cursor
    let input_display = format!("{}█", app.input);
    lines.push(Line::from(vec![Span::styled(
        input_display,
        Style::default()
            .fg(colors.highlight)
            .add_modifier(Modifier::BOLD),
    )]));

    // Add spacer
    lines.push(Line::from(""));

    // Add controls help
    lines.push(Line::from(vec![
        Span::styled(
            "Enter",
            Style::default()
                .fg(colors.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to submit · ", Style::default().fg(colors.text_secondary)),
        Span::styled(
            "Esc",
            Style::default()
                .fg(colors.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to cancel", Style::default().fg(colors.text_secondary)),
    ]));

    let input_paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(colors.border_focus_type)
            .border_style(Style::default().fg(colors.border_focus))
            .style(Style::default().bg(colors.surface))
            .padding(Padding::new(3, 3, 2, 2)),
    );

    f.render_widget(input_paragraph, area);
}

fn render_filter_modal<B: Backend>(f: &mut Frame<B>, app: &App, colors: &ColorScheme) {
    let area = centered_rect(70, 60, f.size());

    // Clear the area
    f.render_widget(Clear, area);

    // Theme-specific filter icon
    let filter_icon = colors.get_icon_search();

    // Create filter selection UI with theme-specific styling
    let mut text = vec![
        // Header
        Line::from(vec![
            Span::styled(
                format!("  {}  ", filter_icon),
                Style::default().fg(colors.primary),
            ),
            Span::styled(
                "Feed Filters",
                Style::default()
                    .fg(colors.highlight)
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
        Span::styled("  c - Category: ", Style::default().fg(colors.text)),
        Span::styled(
            category_status,
            Style::default().fg(if app.filter_options.category.is_some() {
                colors.highlight
            } else {
                colors.muted
            }),
        ),
        Span::styled(
            if !available_categories.is_empty() {
                format!(" ({})", available_categories.join(", "))
            } else {
                "".to_string()
            },
            Style::default().fg(colors.muted),
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
        Span::styled("  t - Time/Age: ", Style::default().fg(colors.text)),
        Span::styled(
            age_status,
            Style::default().fg(if app.filter_options.age.is_some() {
                colors.highlight
            } else {
                colors.muted
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
        Span::styled("  a - Author: ", Style::default().fg(colors.text)),
        Span::styled(
            author_status,
            Style::default().fg(if app.filter_options.has_author.is_some() {
                colors.highlight
            } else {
                colors.muted
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
        Span::styled("  r - Read status: ", Style::default().fg(colors.text)),
        Span::styled(
            read_status,
            Style::default().fg(if app.filter_options.read_status.is_some() {
                colors.highlight
            } else {
                colors.muted
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
        Span::styled("  l - Length: ", Style::default().fg(colors.text)),
        Span::styled(
            length_status,
            Style::default().fg(if app.filter_options.min_length.is_some() {
                colors.highlight
            } else {
                colors.muted
            }),
        ),
    ]));

    // Clear filters option
    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled("  x - ", Style::default().fg(colors.text)),
        Span::styled("Clear all filters", Style::default().fg(colors.error)),
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
        Style::default().fg(colors.muted),
    )]));

    // Add the filter summary
    if active_count > 0 {
        text.push(Line::from(""));
        text.push(Line::from(vec![Span::styled(
            format!("  Current filters: {}", app.get_filter_summary()),
            Style::default().fg(colors.secondary),
        )]));
    }

    text.push(Line::from(""));
    text.push(Line::from(vec![Span::styled(
        "  Press Esc to close this dialog",
        Style::default().fg(colors.text),
    )]));

    let filter_paragraph = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(colors.border_focus_type)
            .border_style(Style::default().fg(colors.border_focus))
            .style(Style::default().bg(colors.surface))
            .title(format!(" {} Filter Options ", filter_icon))
            .title_alignment(Alignment::Center)
            .padding(Padding::new(3, 3, 2, 2)),
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

// Helper function to format content for better reading experience
fn format_content_for_reading(text: &str) -> String {
    let mut formatted_lines = Vec::new();
    let mut current_paragraph = Vec::new();
    let mut in_list = false;

    for line in text.lines() {
        let trimmed = line.trim();

        // Detect list items (lines starting with -, *, •, numbers, etc.)
        let is_list_item = trimmed.starts_with('-')
            || trimmed.starts_with('*')
            || trimmed.starts_with('•')
            || trimmed.starts_with("  - ")
            || trimmed.starts_with("  * ")
            || (trimmed.len() > 2
                && trimmed.chars().next().unwrap_or(' ').is_ascii_digit()
                && trimmed.chars().nth(1) == Some('.'));

        if trimmed.is_empty() {
            // Empty line - end current paragraph
            if !current_paragraph.is_empty() {
                formatted_lines.push(current_paragraph.join(" "));
                current_paragraph.clear();
                formatted_lines.push(String::new()); // Add spacing between paragraphs
                in_list = false;
            }
        } else if is_list_item {
            // List item - preserve as its own line
            if !current_paragraph.is_empty() {
                formatted_lines.push(current_paragraph.join(" "));
                current_paragraph.clear();
            }
            formatted_lines.push(format!("  {}", trimmed));
            in_list = true;
        } else if in_list && trimmed.starts_with("  ") {
            // Continuation of list item
            formatted_lines.push(format!("    {}", trimmed.trim()));
        } else {
            // Regular text - accumulate into current paragraph
            if in_list && !current_paragraph.is_empty() {
                // Starting new paragraph after list
                formatted_lines.push(String::new());
                in_list = false;
            }
            current_paragraph.push(trimmed.to_string());
        }
    }

    // Add any remaining paragraph
    if !current_paragraph.is_empty() {
        formatted_lines.push(current_paragraph.join(" "));
    }

    // Clean up excessive empty lines (max 2 in a row becomes 1)
    let mut result = Vec::new();
    let mut empty_count = 0;

    for line in formatted_lines {
        if line.is_empty() {
            empty_count += 1;
            if empty_count <= 1 {
                result.push(line);
            }
        } else {
            empty_count = 0;
            result.push(line);
        }
    }

    result.join("\n")
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
                let wrapped_lines = line_width.div_ceil(width).max(1);
                line_count = line_count.saturating_add(wrapped_lines as u16);
            }
        }
    }

    // If text doesn't end with newline, we still have the lines we counted
    // If text is empty, return at least 1 line
    line_count.max(1)
}

// Update the render_category_management function to show feeds when a category is expanded
fn render_category_management<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    area: Rect,
    colors: &ColorScheme,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(3),    // Category list
            Constraint::Length(5), // Help text
        ])
        .split(area);

    // Theme-specific category icon
    let category_icon = if colors.border_normal == BorderType::Double {
        "◈◈" // Dark: double tech diamond
    } else {
        "📂" // Light: folder
    };

    // Add a title block
    let title = match &app.category_action {
        Some(CategoryAction::AddFeedToCategory(url)) => {
            // Show which feed is being assigned to a category
            let feed_idx = app.feeds.iter().position(|f| f.url == *url);
            let feed_title = feed_idx
                .and_then(|idx| app.feeds.get(idx))
                .map_or("Unknown Feed", |feed| feed.title.as_str());
            format!(
                " {} Add '{}' to Category ",
                category_icon,
                truncate_str(feed_title, 30)
            )
        }
        _ => format!(" {} Category Management ", category_icon),
    };

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_type(colors.border_normal)
        .title(title)
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(colors.border))
        .style(Style::default().bg(colors.surface))
        .padding(Padding::new(1, 1, 0, 0));

    f.render_widget(title_block, chunks[0]);

    // Prepare list items for categories and their feeds with theme-specific indicators
    let expand_icon = if colors.border_normal == BorderType::Double {
        ("▼", "▶") // Dark: solid arrows
    } else {
        ("⌄", "›") // Light: minimal arrows
    };
    let feed_arrow = colors.get_arrow_right();

    let mut list_items = Vec::new();
    let mut list_indices = Vec::new(); // To map UI index to category index

    if app.categories.is_empty() {
        list_items.push(ListItem::new(Line::from(Span::styled(
            "No categories yet. Press 'n' to create a new category.",
            Style::default().fg(colors.muted),
        ))));
    } else {
        for (cat_idx, category) in app.categories.iter().enumerate() {
            // Add category to the list with theme-specific expansion indicator
            let icon = if category.expanded {
                expand_icon.0
            } else {
                expand_icon.1
            };
            let feed_count = category.feed_count();
            let count_text = if feed_count == 1 {
                "1 feed".to_string()
            } else {
                format!("{} feeds", feed_count)
            };

            let style = if Some(cat_idx) == app.selected_category {
                Style::default()
                    .fg(colors.highlight)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.text)
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
                        Style::default().fg(colors.accent)
                    } else {
                        Style::default().fg(colors.muted)
                    };

                    list_items.push(ListItem::new(Line::from(Span::styled(
                        format!("   {} {}", feed_arrow, truncate_str(&feed.title, 40)),
                        feed_style,
                    ))));
                    list_indices.push(None); // None means this is a feed, not a category
                }

                // Show a message if the category is empty
                if feeds_in_category.is_empty() {
                    list_items.push(ListItem::new(Line::from(Span::styled(
                        "   (No feeds in this category)",
                        Style::default().fg(colors.muted),
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
                .border_type(colors.border_normal)
                .title(format!(" {} Categories ", category_icon))
                .border_style(Style::default().fg(colors.border))
                .style(Style::default().bg(colors.surface))
                .padding(Padding::new(2, 1, 1, 1)),
        )
        .highlight_style(
            Style::default()
                .bg(colors.selected_bg)
                .fg(colors.highlight)
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
        .border_type(colors.border_normal)
        .title(" Controls ")
        .border_style(Style::default().fg(colors.muted));

    let help_para = Paragraph::new(help_text)
        .block(help_block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(help_para, chunks[2]);
}

// Add a new function to render the category name input modal
fn render_category_input_modal<B: Backend>(f: &mut Frame<B>, app: &App, colors: &ColorScheme) {
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

    // Create title block with theme-specific border
    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_alignment(Alignment::Center)
        .border_type(colors.border_normal)
        .border_style(Style::default().fg(colors.border))
        .style(Style::default().bg(colors.surface))
        .padding(Padding::new(1, 1, 0, 0));

    f.render_widget(title_block, chunks[0]);

    // Create input field with theme-specific focus border
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(" Name ")
        .border_type(colors.border_focus_type)
        .border_style(Style::default().fg(colors.border_focus))
        .style(Style::default().bg(colors.surface))
        .padding(Padding::new(1, 1, 0, 0));

    let input_text = Paragraph::new(app.input.as_str())
        .block(input_block)
        .style(Style::default().fg(colors.highlight));

    f.render_widget(input_text, chunks[1]);

    // Position cursor at the end of input
    let cursor_x = app.input.width() as u16 + chunks[1].x + 1; // +1 for border
    let cursor_y = chunks[1].y + 1;
    f.set_cursor(cursor_x, cursor_y);

    // Help text with theme-specific border
    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_type(colors.border_normal)
        .title(" Controls ")
        .border_style(Style::default().fg(colors.muted));

    let help_text = "ENTER: Confirm | ESC: Cancel";
    let help_para = Paragraph::new(help_text)
        .block(help_block)
        .alignment(Alignment::Center);

    f.render_widget(help_para, chunks[3]);
}

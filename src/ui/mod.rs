use crate::app::{App, InputMode, View};
use crate::config::Theme;
use crate::keybindings::{key_display, KeyAction};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::{self},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Tabs},
    Frame,
};

mod categories;
mod dashboard;
mod detail;
mod feed_items;
mod feed_list;
mod modals;
mod starred;
mod summary;
pub(crate) mod utils;

use categories::{render_category_input_modal, render_category_management};
use dashboard::render_dashboard;
use detail::render_item_detail;
use feed_items::render_feed_items;
use feed_list::render_feed_list;
use modals::{
    render_error_modal, render_feed_selection_modal, render_filter_modal, render_help_overlay,
    render_input_modal, render_link_overlay, render_success_notification,
};
use starred::render_starred;
use summary::render_summary;

// Re-export extract_domain so it remains accessible as crate::ui::extract_domain
pub use feed_list::extract_domain;

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
    pub fn get_empty_feed_art(&self) -> &'static [&'static str] {
        if self.border_normal == BorderType::Double {
            // Dark theme: Cyberpunk terminal
            &[
                "                                           ",
                "       ╔═══════════════════╗               ",
                "       ║  ◢◣  C Y B E R  ◤◥  ║               ",
                "       ║   ═══════════════  ║               ",
                "       ║   > NO_SIGNAL_    ║               ",
                "       ║   > INIT_FEED...  ║               ",
                "       ╚═══════════════════╝               ",
                "                                           ",
            ]
        } else {
            // Light theme: Zen garden with simple plant
            &[
                "                                           ",
                "              _                            ",
                "             ( )                           ",
                "              |                            ",
                "             / \\                           ",
                "            /   \\                          ",
                "           -------                         ",
                "                                           ",
            ]
        }
    }

    /// Get theme-specific dashboard welcome art
    pub fn get_dashboard_art(&self) -> &'static [&'static str] {
        if self.border_normal == BorderType::Double {
            // Dark theme: Cyberpunk glitch aesthetic
            &[
                "                                                ",
                "  ███████╗███████╗███████╗██████╗ ██████╗      ",
                "  ██╔════╝██╔════╝██╔════╝██╔══██╗██╔══██╗     ",
                "  █████╗  █████╗  █████╗  ██║  ██║██████╔╝     ",
                "  ██╔══╝  ██╔══╝  ██╔══╝  ██║  ██║██╔══██╗     ",
                "  ██║     ███████╗███████╗██████╔╝██║  ██║     ",
                "  ╚═╝     ╚══════╝╚══════╝╚═════╝ ╚═╝  ╚═╝     ",
                "  ═══════════════════════════════════════════  ",
                "  ◢◣ NEURAL FEED INTERFACE v2.0 ◤◥             ",
                "  ═══════════════════════════════════════════  ",
                "                                                ",
                "  ▸ INITIALIZE: Press 'a' to add feed URL       ",
                "  ▸ CONNECT TO DATA STREAMS                     ",
                "                                                ",
            ]
        } else {
            // Light theme: Zen minimalist
            &[
                "                                                ",
                "                                                ",
                "            F  e  e  d  r                      ",
                "                                                ",
                "         ─────────────────                     ",
                "                                                ",
                "         A mindful RSS reader                   ",
                "                                                ",
                "                                                ",
                "  🍃  Begin by adding your first feed           ",
                "       Press 'a' to add a feed URL              ",
                "                                                ",
                "                                                ",
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
    // Use the cached color scheme from app state
    let colors = app.color_scheme.clone();

    // Set background color for the entire terminal
    let bg_block = Block::default().style(Style::default().bg(colors.background));
    f.render_widget(bg_block, f.size());

    // Main layout division — compact mode uses tighter spacing
    let chunks = if app.compact {
        Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([
                Constraint::Length(1), // Compact title bar
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Compact help bar
            ])
            .split(f.size())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(4), // Title/tab bar (slightly taller)
                Constraint::Min(0),    // Main content
                Constraint::Length(4), // Help bar (slightly taller for breathing room)
            ])
            .split(f.size())
    };

    if app.compact {
        render_compact_title_bar(f, app, chunks[0], &colors);
    } else {
        render_title_bar(f, app, chunks[0], &colors);
    }

    match app.view {
        View::Dashboard => render_dashboard(f, app, chunks[1], &colors),
        View::FeedList => render_feed_list(f, app, chunks[1], &colors),
        View::FeedItems => render_feed_items(f, app, chunks[1], &colors),
        View::FeedItemDetail => render_item_detail(f, app, chunks[1], &colors),
        View::CategoryManagement => render_category_management(f, app, chunks[1], &colors),
        View::Starred => render_starred(f, app, chunks[1], &colors),
        View::Summary => render_summary(f, app, chunks[1], &colors),
    }

    if app.compact {
        render_compact_help_bar(f, app, chunks[2], &colors);
    } else {
        render_help_bar(f, app, chunks[2], &colors);
    }

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

    // Show feed selection modal when picking from discovered feeds
    if app.input_mode == InputMode::SelectDiscoveredFeed {
        render_feed_selection_modal(f, app, &colors);
    }

    // Show filter modal when in filter mode
    if app.filter_mode {
        render_filter_modal(f, app, &colors);
    }

    // Show category input modal when in category name input mode
    if app.input_mode == InputMode::CategoryNameInput {
        render_category_input_modal(f, app, &colors);
    }

    // Show link extraction overlay
    if app.show_link_overlay {
        render_link_overlay(f, app, &colors);
    }

    // Show help overlay on top of everything
    if app.show_help_overlay {
        render_help_overlay(f, app, &colors);
    }
}

fn render_title_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect, colors: &ColorScheme) {
    // Create tabs for navigation
    let titles = [
        "Dashboard",
        "Feeds",
        "Items",
        "Detail",
        "Categories",
        "Starred",
        "What's New",
    ];
    let selected_tab = match app.view {
        View::Dashboard => 0,
        View::FeedList => 1,
        View::FeedItems => 2,
        View::FeedItemDetail => 3,
        View::CategoryManagement => 4,
        View::Starred => 5,
        View::Summary => 6,
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

fn render_compact_title_bar<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    area: Rect,
    colors: &ColorScheme,
) {
    let view_name = match app.view {
        View::Dashboard => "Dashboard",
        View::FeedList => "Feeds",
        View::FeedItems => "Items",
        View::FeedItemDetail => "Detail",
        View::CategoryManagement => "Categories",
        View::Starred => "Starred",
        View::Summary => "What's New",
    };

    let title = if app.is_loading {
        let frames = colors.get_loading_frames();
        format!(
            " Feedr > {} {} ",
            view_name,
            frames[app.loading_indicator % frames.len()]
        )
    } else {
        format!(" Feedr > {} ", view_name)
    };

    let bar = Paragraph::new(Line::from(vec![Span::styled(
        title,
        Style::default()
            .fg(colors.primary)
            .add_modifier(Modifier::BOLD)
            .bg(colors.surface),
    )]))
    .style(Style::default().bg(colors.surface));
    f.render_widget(bar, area);
}

fn render_help_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect, colors: &ColorScheme) {
    // Match on the input mode and view to determine the help text and style
    let (help_text, _style) = match app.input_mode {
        InputMode::Normal => {
            let help_text = match app.view {
                View::Dashboard => {
                    if app.feeds.is_empty() {
                        format!(
                            "{}: Add feed | {}: Theme | {}: Quit | {}: Quit | CTRL+C: Manage categories",
                            key_display(&KeyAction::AddFeed, &app.keybindings),
                            key_display(&KeyAction::ToggleTheme, &app.keybindings),
                            key_display(&KeyAction::Quit, &app.keybindings),
                            key_display(&KeyAction::ForceQuit, &app.keybindings),
                        )
                    } else {
                        format!(
                            "{}/{}: Navigate | {}: View | {}: Star | {}: Toggle read | {}: Mark all read | {}: Preview | {}: Add | {}: Refresh | {}: Filter | {}: Search | {}: Help | {}: Quit",
                            key_display(&KeyAction::MoveUp, &app.keybindings),
                            key_display(&KeyAction::MoveDown, &app.keybindings),
                            key_display(&KeyAction::Select, &app.keybindings),
                            key_display(&KeyAction::ToggleStar, &app.keybindings),
                            key_display(&KeyAction::ToggleRead, &app.keybindings),
                            key_display(&KeyAction::MarkAllRead, &app.keybindings),
                            key_display(&KeyAction::TogglePreview, &app.keybindings),
                            key_display(&KeyAction::AddFeed, &app.keybindings),
                            key_display(&KeyAction::Refresh, &app.keybindings),
                            key_display(&KeyAction::OpenFilter, &app.keybindings),
                            key_display(&KeyAction::OpenSearch, &app.keybindings),
                            key_display(&KeyAction::Help, &app.keybindings),
                            key_display(&KeyAction::Quit, &app.keybindings),
                        )
                    }
                }
                View::FeedList => {
                    if app.feeds.is_empty() && app.categories.is_empty() {
                        format!(
                            "{}: Add feed | {}: Theme | {}: Back | {}: Quit | TAB: Dashboard | CTRL+C: Categories",
                            key_display(&KeyAction::AddFeed, &app.keybindings),
                            key_display(&KeyAction::ToggleTheme, &app.keybindings),
                            key_display(&KeyAction::Quit, &app.keybindings),
                            key_display(&KeyAction::ForceQuit, &app.keybindings),
                        )
                    } else {
                        format!(
                            "{}/{}: Navigate | {}: Open | Space: Expand/Collapse | d: Delete | c: Category | {}: Mark read | {}: Add | {}: Help | {}: Back",
                            key_display(&KeyAction::MoveUp, &app.keybindings),
                            key_display(&KeyAction::MoveDown, &app.keybindings),
                            key_display(&KeyAction::Select, &app.keybindings),
                            key_display(&KeyAction::MarkAllRead, &app.keybindings),
                            key_display(&KeyAction::AddFeed, &app.keybindings),
                            key_display(&KeyAction::Help, &app.keybindings),
                            key_display(&KeyAction::Quit, &app.keybindings),
                        )
                    }
                }
                View::CategoryManagement => {
                    "n: New category | e: Edit | d: Delete | SPACE: Toggle feeds | c: Add selected feed | t: Theme | ESC/q: Back".to_string()
                }
                View::FeedItems => {
                    "h/esc/q: back | home: dashboard | enter: view | s: Star | Space: Toggle read | o: open | /: search | t: theme | Ctrl+Q: quit".to_string()
                }
                View::FeedItemDetail => {
                    "h/esc/q: back | home: dashboard | \u{2191}/\u{2193}: scroll | PgUp/PgDn: fast | s: Star | Space: Toggle read | o: open | t: theme | Ctrl+Q: quit".to_string()
                }
                View::Starred => {
                    "\u{2191}/\u{2193}: Navigate | ENTER: View | s: Unstar | Space: Toggle read | o: Open | Tab: Switch view | q: Back | Ctrl+Q: Quit".to_string()
                }
                View::Summary => "Press any key to continue to Dashboard | q: Back | Ctrl+Q: Quit".to_string()
            };
            (help_text, Style::default().fg(colors.text))
        }
        InputMode::InsertUrl => (
            "Enter feed URL (e.g., https://news.ycombinator.com/rss)".to_string(),
            Style::default().fg(colors.highlight),
        ),
        InputMode::SearchMode => (
            "Type to search (results update live) | ENTER: keep results | ESC: cancel".to_string(),
            Style::default().fg(colors.highlight),
        ),
        InputMode::FilterMode => ("".to_string(), Style::default().fg(colors.muted)),
        InputMode::CategoryNameInput => ("".to_string(), Style::default().fg(colors.muted)),
        InputMode::SelectDiscoveredFeed => (
            "j/k: Navigate | Enter: Select feed | Esc: Cancel".to_string(),
            Style::default().fg(colors.highlight),
        ),
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

fn render_compact_help_bar<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    area: Rect,
    colors: &ColorScheme,
) {
    if !matches!(app.input_mode, InputMode::Normal) {
        return;
    }

    let help_text = match app.view {
        View::Dashboard => "q:quit a:add r:refresh /:search f:filter p:preview ?:help",
        View::FeedList => "q:back a:add enter:open space:expand d:del c:category m:read",
        View::FeedItems => "q:back enter:view o:open s:star /:search",
        View::FeedItemDetail => "q:back j/k:scroll o:open s:star space:read",
        View::CategoryManagement => "q:back n:new e:edit d:del",
        View::Starred => "q:back enter:view s:unstar o:open",
        View::Summary => "any key:continue",
    };

    let spans: Vec<Span> = help_text
        .split(' ')
        .enumerate()
        .flat_map(|(i, part)| {
            let mut result = Vec::new();
            if i > 0 {
                result.push(Span::styled(" ", Style::default().fg(colors.border)));
            }
            if let Some(pos) = part.find(':') {
                let (key, desc) = part.split_at(pos + 1);
                result.push(Span::styled(
                    key,
                    Style::default()
                        .fg(colors.highlight)
                        .add_modifier(Modifier::BOLD),
                ));
                result.push(Span::styled(desc, Style::default().fg(colors.text)));
            } else {
                result.push(Span::styled(part, Style::default().fg(colors.text)));
            }
            result
        })
        .collect();

    let help = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .style(Style::default().bg(colors.surface));
    f.render_widget(help, area);
}

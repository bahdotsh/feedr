use crate::app::{App, InputMode, LinkType, TimeFilter, View};
use crate::keybindings::{key_display, KeyAction};
use crate::ui::utils::{centered_rect_with_min, truncate_str};
use crate::ui::ColorScheme;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap},
    Frame,
};

pub(super) fn render_error_modal<B: Backend>(f: &mut Frame<B>, error: &str, colors: &ColorScheme) {
    let area = centered_rect_with_min(60, 30, 40, 8, f.size());

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

pub(super) fn render_success_notification<B: Backend>(
    f: &mut Frame<B>,
    message: &str,
    colors: &ColorScheme,
) {
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

pub(super) fn render_input_modal<B: Backend>(f: &mut Frame<B>, app: &App, colors: &ColorScheme) {
    let area = centered_rect_with_min(70, 25, 50, 12, f.size());

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
            "Enter the RSS feed URL and press Enter".to_string(),
            link_icon,
        )
    } else {
        let result_count = app.filtered_items.len();
        let search_help = if app.input.is_empty() {
            "Search across all feeds (results update live)".to_string()
        } else {
            format!(
                "{} result{} found",
                result_count,
                if result_count == 1 { "" } else { "s" }
            )
        };
        ("Search", search_help, colors.get_icon_search())
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
        help_text.clone(),
        Style::default().fg(colors.text_secondary),
    )]));

    // Add spacers
    lines.push(Line::from(""));
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

    // Main modal paragraph (no input text)
    let modal_paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(colors.border_focus_type)
            .border_style(Style::default().fg(colors.border_focus))
            .style(Style::default().bg(colors.surface))
            .padding(Padding::new(3, 3, 2, 2)),
    );
    f.render_widget(modal_paragraph, area);

    let input_paragraph = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::NONE))
        .style(
            Style::default()
                .fg(colors.highlight)
                .add_modifier(Modifier::BOLD),
        );

    // Offsets: 1 (border) + 3 (padding left/right), 1 (border) + 2 (padding top) + 4 (title, sep, help, spacer)
    let input_rect = Rect::new(area.x + 4, area.y + 7, area.width.saturating_sub(8), 1);

    f.render_widget(input_paragraph, input_rect);

    let cursor_x = input_rect.x + app.input.chars().count() as u16;
    f.set_cursor(cursor_x.min(input_rect.x + input_rect.width), input_rect.y);
}

pub(super) fn render_feed_selection_modal<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    colors: &ColorScheme,
) {
    // Cap visible items to avoid overflow; 8 items × ~2 lines each = 16 content lines max
    let max_visible: usize = 8;
    let total = app.discovered_feeds.len();
    let selected = app.discovered_feed_selection;

    // Scroll window: keep selected item visible
    let scroll_offset = if total <= max_visible {
        0
    } else {
        selected
            .saturating_sub(max_visible - 1)
            .min(total - max_visible)
    };
    let visible_end = (scroll_offset + max_visible).min(total);

    let visible_count = (visible_end - scroll_offset) as u16;
    // Height: 2 (border) + 4 (padding) + 1 (title) + 1 (count) + 2 (spacers) + items*2 + 1 (controls)
    let min_h = 11 + visible_count * 2;
    let area = centered_rect_with_min(70, 40, 50, min_h.min(30), f.size());

    f.render_widget(Clear, area);

    let icon = if colors.border_normal == BorderType::Double {
        "◎"
    } else {
        "🔗"
    };

    let mut lines = Vec::new();

    lines.push(Line::from(vec![Span::styled(
        format!("{} Select Feed", icon),
        Style::default()
            .fg(colors.text)
            .add_modifier(Modifier::BOLD),
    )]));

    lines.push(Line::from(""));

    let scroll_hint = if total > max_visible {
        format!(
            "{} feed(s) discovered on this page (showing {}-{} of {})",
            total,
            scroll_offset + 1,
            visible_end,
            total
        )
    } else {
        format!("{} feed(s) discovered on this page", total)
    };
    lines.push(Line::from(vec![Span::styled(
        scroll_hint,
        Style::default().fg(colors.text_secondary),
    )]));

    lines.push(Line::from(""));

    for (i, feed) in app
        .discovered_feeds
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(max_visible)
    {
        let is_selected = i == selected;
        let badge = format!("[{}]", feed.feed_type);
        let prefix = if is_selected { "> " } else { "  " };

        let style = if is_selected {
            Style::default()
                .fg(colors.highlight)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.text)
        };

        let badge_style = if is_selected {
            Style::default()
                .fg(colors.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.text_secondary)
        };

        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(format!("{} ", badge), badge_style),
            Span::styled(feed.title.clone(), style),
        ]));

        // Show URL on a second line if it differs from the title
        if feed.url != feed.title {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(&feed.url, Style::default().fg(colors.text_secondary)),
            ]));
        }
    }

    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled(
            "j/k",
            Style::default()
                .fg(colors.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" navigate · ", Style::default().fg(colors.text_secondary)),
        Span::styled(
            "Enter",
            Style::default()
                .fg(colors.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" select · ", Style::default().fg(colors.text_secondary)),
        Span::styled(
            "Esc",
            Style::default()
                .fg(colors.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel", Style::default().fg(colors.text_secondary)),
    ]));

    let modal = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(colors.border_focus_type)
            .border_style(Style::default().fg(colors.border_focus))
            .style(Style::default().bg(colors.surface))
            .padding(Padding::new(3, 3, 2, 2)),
    );
    f.render_widget(modal, area);
}

pub(super) fn render_filter_modal<B: Backend>(f: &mut Frame<B>, app: &App, colors: &ColorScheme) {
    let area = centered_rect_with_min(70, 60, 50, 18, f.size());

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

    // Starred filter
    let starred_status = match app.filter_options.starred_only {
        Some(true) => "[Starred]",
        Some(false) => "[Not starred]",
        None => "[Off]",
    };

    text.push(Line::from(vec![
        Span::styled("  s - Starred: ", Style::default().fg(colors.text)),
        Span::styled(
            starred_status,
            Style::default().fg(if app.filter_options.starred_only.is_some() {
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
            "  Active Filters: {}/6  |  Showing: {}/{} items",
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

pub(super) fn render_help_overlay<B: Backend>(f: &mut Frame<B>, app: &App, colors: &ColorScheme) {
    let area = centered_rect_with_min(80, 85, 60, 24, f.size());
    f.render_widget(Clear, area);

    let title_icon = if colors.border_normal == BorderType::Double {
        "◈"
    } else {
        "📖"
    };

    // Build help content organized by sections
    let mut lines: Vec<Line> = Vec::new();

    let section_style = Style::default()
        .fg(colors.secondary)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default()
        .fg(colors.highlight)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(colors.text);
    let sep_style = Style::default().fg(colors.border);

    // Helper to add a keybinding line
    let add_key = |key: &str, desc: &str, lines: &mut Vec<Line<'_>>| {
        lines.push(Line::from(vec![
            Span::styled("    ", desc_style),
            Span::styled(format!("{:<16}", key), key_style),
            Span::styled(desc.to_string(), desc_style),
        ]));
    };

    let separator = Line::from(Span::styled(
        "  ─────────────────────────────────────",
        sep_style,
    ));

    let kd = |action: &KeyAction| key_display(action, &app.keybindings);

    // Global section
    lines.push(Line::from(Span::styled("  Global", section_style)));
    lines.push(Line::from(""));
    add_key(&kd(&KeyAction::ForceQuit), "Quit from any view", &mut lines);
    add_key(&kd(&KeyAction::Help), "Show this help", &mut lines);
    add_key(
        &kd(&KeyAction::ToggleTheme),
        "Toggle theme (dark/light)",
        &mut lines,
    );
    add_key(&kd(&KeyAction::Refresh), "Refresh all feeds", &mut lines);
    add_key(&kd(&KeyAction::NextTab), "Next view", &mut lines);
    add_key(&kd(&KeyAction::PrevTab), "Previous view", &mut lines);
    lines.push(Line::from(""));
    lines.push(separator.clone());
    lines.push(Line::from(""));

    // Current view-specific section
    match app.view {
        View::Dashboard => {
            lines.push(Line::from(Span::styled("  Dashboard", section_style)));
            lines.push(Line::from(""));
            add_key(&kd(&KeyAction::MoveUp), "Navigate up", &mut lines);
            add_key(&kd(&KeyAction::MoveDown), "Navigate down", &mut lines);
            add_key(
                &format!("Shift+{}", kd(&KeyAction::ScrollPreviewUp)),
                "Scroll preview pane",
                &mut lines,
            );
            add_key(&kd(&KeyAction::Select), "View article detail", &mut lines);
            add_key(
                &kd(&KeyAction::OpenInBrowser),
                "Open in browser",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::ToggleRead),
                "Toggle read/unread",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::ToggleStar),
                "Star/unstar article",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::TogglePreview),
                "Toggle preview pane",
                &mut lines,
            );
            add_key(&kd(&KeyAction::AddFeed), "Add new feed", &mut lines);
            add_key(&kd(&KeyAction::OpenFilter), "Open filter menu", &mut lines);
            add_key(
                &kd(&KeyAction::MarkAllRead),
                "Mark all visible as read",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::CycleCategory),
                "Cycle category filter",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::OpenSearch),
                "Search across all feeds",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::OpenCategoryManagement),
                "Manage categories",
                &mut lines,
            );
            add_key(&kd(&KeyAction::Quit), "Quit", &mut lines);
        }
        View::FeedList => {
            lines.push(Line::from(Span::styled("  Feeds", section_style)));
            lines.push(Line::from(""));
            add_key(&kd(&KeyAction::MoveUp), "Navigate up", &mut lines);
            add_key(&kd(&KeyAction::MoveDown), "Navigate down", &mut lines);
            add_key(
                &kd(&KeyAction::Select),
                "Open feed / expand category",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::ToggleExpand),
                "Expand/collapse category",
                &mut lines,
            );
            add_key(&kd(&KeyAction::AddFeed), "Add new feed", &mut lines);
            add_key(
                &kd(&KeyAction::DeleteFeed),
                "Delete feed or category",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::MarkAllRead),
                "Mark feed/category as read",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::AssignCategory),
                "Assign feed to category",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::OpenCategoryManagement),
                "Manage categories",
                &mut lines,
            );
            add_key(&kd(&KeyAction::OpenSearch), "Search", &mut lines);
            add_key(&kd(&KeyAction::Quit), "Back to Dashboard", &mut lines);
        }
        View::FeedItems => {
            lines.push(Line::from(Span::styled("  Feed Items", section_style)));
            lines.push(Line::from(""));
            add_key(&kd(&KeyAction::MoveUp), "Navigate up", &mut lines);
            add_key(&kd(&KeyAction::MoveDown), "Navigate down", &mut lines);
            add_key(&kd(&KeyAction::Select), "View article detail", &mut lines);
            add_key(
                &kd(&KeyAction::OpenInBrowser),
                "Open in browser",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::ToggleRead),
                "Toggle read/unread",
                &mut lines,
            );
            add_key(&kd(&KeyAction::ToggleStar), "Star/unstar", &mut lines);
            add_key(&kd(&KeyAction::MarkAllRead), "Mark all as read", &mut lines);
            add_key(&kd(&KeyAction::OpenSearch), "Search", &mut lines);
            add_key(&kd(&KeyAction::Back), "Back to Feeds", &mut lines);
            add_key(&kd(&KeyAction::Home), "Back to Dashboard", &mut lines);
            add_key(&kd(&KeyAction::Quit), "Back to Feeds", &mut lines);
        }
        View::FeedItemDetail => {
            lines.push(Line::from(Span::styled("  Article Detail", section_style)));
            lines.push(Line::from(""));
            add_key(&kd(&KeyAction::MoveUp), "Scroll up", &mut lines);
            add_key(&kd(&KeyAction::MoveDown), "Scroll down", &mut lines);
            add_key(
                &kd(&KeyAction::PageUp),
                "Scroll fast up (10 lines)",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::PageDown),
                "Scroll fast down (10 lines)",
                &mut lines,
            );
            add_key(&kd(&KeyAction::JumpTop), "Jump to top", &mut lines);
            add_key(&kd(&KeyAction::JumpBottom), "Jump to bottom", &mut lines);
            add_key(
                &kd(&KeyAction::OpenInBrowser),
                "Open in browser",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::ToggleRead),
                "Toggle read/unread",
                &mut lines,
            );
            add_key(&kd(&KeyAction::ToggleStar), "Star/unstar", &mut lines);
            add_key(
                &kd(&KeyAction::ExtractLinks),
                "Extract links/images",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::OpenSearch),
                "Search across all feeds",
                &mut lines,
            );
            add_key(&kd(&KeyAction::Back), "Back", &mut lines);
            add_key(&kd(&KeyAction::Home), "Back to Dashboard", &mut lines);
            add_key(&kd(&KeyAction::Quit), "Back", &mut lines);
        }
        View::Starred => {
            lines.push(Line::from(Span::styled("  Starred", section_style)));
            lines.push(Line::from(""));
            add_key(&kd(&KeyAction::MoveUp), "Navigate up", &mut lines);
            add_key(&kd(&KeyAction::MoveDown), "Navigate down", &mut lines);
            add_key(&kd(&KeyAction::Select), "View article detail", &mut lines);
            add_key(
                &kd(&KeyAction::OpenInBrowser),
                "Open in browser",
                &mut lines,
            );
            add_key(
                &kd(&KeyAction::ToggleRead),
                "Toggle read/unread",
                &mut lines,
            );
            add_key(&kd(&KeyAction::ToggleStar), "Unstar article", &mut lines);
            add_key(&kd(&KeyAction::MarkAllRead), "Mark all as read", &mut lines);
            add_key(
                &kd(&KeyAction::OpenSearch),
                "Search across all feeds",
                &mut lines,
            );
            add_key(&kd(&KeyAction::Quit), "Back to Dashboard", &mut lines);
        }
        View::CategoryManagement => {
            lines.push(Line::from(Span::styled("  Categories", section_style)));
            lines.push(Line::from(""));
            add_key("k/↑", "Navigate up", &mut lines);
            add_key("j/↓", "Navigate down", &mut lines);
            add_key("n", "Create new category", &mut lines);
            add_key("e", "Rename category", &mut lines);
            add_key("d", "Delete category", &mut lines);
            add_key("Space", "Expand/collapse", &mut lines);
            add_key("Enter", "Assign feed (when adding)", &mut lines);
            add_key("q/Esc", "Back to Feeds", &mut lines);
        }
        View::Summary => {
            lines.push(Line::from(Span::styled("  What's New", section_style)));
            lines.push(Line::from(""));
            add_key("Any key", "Dismiss and go to Dashboard", &mut lines);
        }
    }

    lines.push(Line::from(""));
    lines.push(separator);
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("    ", desc_style),
        Span::styled(
            format!(
                "{:<16}",
                format!("Esc/{}/{}", kd(&KeyAction::Help), kd(&KeyAction::Quit))
            ),
            key_style,
        ),
        Span::styled("Dismiss this help", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    ", desc_style),
        Span::styled(format!("{:<16}", kd(&KeyAction::MoveDown)), key_style),
        Span::styled("Scroll help", desc_style),
    ]));

    let paragraph = Paragraph::new(lines)
        .scroll((app.help_overlay_scroll, 0))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(format!(" {} Keyboard Shortcuts ", title_icon))
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(colors.border_focus_type)
                .border_style(Style::default().fg(colors.primary))
                .style(Style::default().bg(colors.surface))
                .padding(Padding::new(2, 2, 1, 1)),
        );

    f.render_widget(paragraph, area);
}

pub(super) fn render_link_overlay<B: Backend>(f: &mut Frame<B>, app: &App, colors: &ColorScheme) {
    let area = centered_rect_with_min(75, 70, 55, 20, f.size());
    f.render_widget(Clear, area);

    let link_count = app
        .extracted_links
        .iter()
        .filter(|l| matches!(l.link_type, LinkType::Link))
        .count();
    let image_count = app
        .extracted_links
        .iter()
        .filter(|l| matches!(l.link_type, LinkType::Image))
        .count();

    let title = format!(" Links ({}) | Images ({}) ", link_count, image_count);

    let inner_height = area.height.saturating_sub(4) as usize; // account for borders and padding
    let scroll_offset = if app.selected_link >= inner_height {
        app.selected_link - inner_height + 1
    } else {
        0
    };

    let mut lines: Vec<Line> = Vec::new();
    for (idx, link) in app.extracted_links.iter().enumerate().skip(scroll_offset) {
        let is_selected = idx == app.selected_link;
        let type_icon = match link.link_type {
            LinkType::Link => "\u{1F517}",
            LinkType::Image => "\u{1F5BC}",
        };
        let prefix = if is_selected { "\u{25B8} " } else { "  " };

        // Truncate URL for display
        let max_url_len = area.width.saturating_sub(10) as usize;
        let display_url = truncate_str(&link.url, max_url_len);

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(colors.highlight)),
            Span::styled(
                format!("{} ", type_icon),
                Style::default().fg(colors.secondary),
            ),
            Span::styled(
                link.text.clone(),
                Style::default()
                    .fg(if is_selected {
                        colors.text
                    } else {
                        colors.text_secondary
                    })
                    .add_modifier(if is_selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(display_url, Style::default().fg(colors.muted)),
        ]));

        if idx - scroll_offset >= inner_height {
            break;
        }
    }

    let kd = |action: &KeyAction| key_display(action, &app.keybindings);
    let help_line = Line::from(vec![
        Span::styled(
            kd(&KeyAction::MoveDown),
            Style::default()
                .fg(colors.highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": Navigate  ", Style::default().fg(colors.text)),
        Span::styled(
            format!(
                "{}/{}",
                kd(&KeyAction::Select),
                kd(&KeyAction::OpenInBrowser)
            ),
            Style::default()
                .fg(colors.highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": Open  ", Style::default().fg(colors.text)),
        Span::styled(
            format!(
                "Esc/{}/{}",
                kd(&KeyAction::Quit),
                kd(&KeyAction::ExtractLinks)
            ),
            Style::default()
                .fg(colors.highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": Close", Style::default().fg(colors.text)),
    ]);
    lines.push(Line::from(""));
    lines.push(help_line);

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(colors.border_focus_type)
            .border_style(Style::default().fg(colors.primary))
            .style(Style::default().bg(colors.surface))
            .padding(Padding::new(1, 1, 1, 0)),
    );

    f.render_widget(paragraph, area);
}

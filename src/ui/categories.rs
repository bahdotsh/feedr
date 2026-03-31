use crate::app::{App, CategoryAction};
use crate::ui::utils::{centered_rect_with_min, truncate_str};
use crate::ui::ColorScheme;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap,
    },
    Frame,
};
use unicode_width::UnicodeWidthStr;

// Update the render_category_management function to show feeds when a category is expanded
pub(super) fn render_category_management<B: Backend>(
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
pub(super) fn render_category_input_modal<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    colors: &ColorScheme,
) {
    let area = centered_rect_with_min(60, 20, 40, 8, f.size());

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

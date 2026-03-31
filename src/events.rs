// Keybinding configurability boundary
// =====================================
// Most actions in Dashboard, FeedList, FeedItems, FeedItemDetail, and
// Starred views use `app.key_matches(KeyAction::…)` and respect the
// user's [keybindings] config overrides.
//
// The following are intentionally hardcoded and will NOT change when
// users remap keys:
//   - Tab / Shift+Tab for view switching (structural navigation)
//   - Number keys 1/2/3 for demo feed shortcuts (Dashboard only)
//   - CategoryManagement: all keys (n/e/d/Enter/Space/r/j/k/q/Esc/?)
//   - FilterMode: all filter-cycling keys (c/t/a/r/s/l/x/Esc)
//   - SelectDiscoveredFeed: j/k/Enter/Esc
//   - All text input modes (InsertUrl, SearchMode, CategoryNameInput)
//   - Detail view: g/G and Ctrl+u/Ctrl+d for scrolling

use crate::app::{AddFeedResult, App, CategoryAction, InputMode, TimeFilter, TreeItem, View};
use crate::keybindings::KeyAction;
use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

// ── Shared action helpers ──────────────────────────────────────────
// These eliminate duplicated blocks that were identical across views.

fn handle_toggle_theme(app: &mut App) {
    if let Err(e) = app.toggle_theme() {
        app.error = Some(format!("Failed to toggle theme: {}", e));
    } else {
        app.success_message = Some("Theme toggled".to_string());
        app.success_message_time = Some(std::time::Instant::now());
    }
}

fn handle_refresh(app: &mut App) {
    if !app.refresh_in_progress {
        app.refresh_requested = true;
    }
}

fn handle_open_search(app: &mut App) {
    app.input.clear();
    app.input_mode = InputMode::SearchMode;
}

fn handle_show_help(app: &mut App) {
    app.show_help_overlay = true;
    app.help_overlay_scroll = 0;
}

fn handle_toggle_star_current(app: &mut App) {
    if let Some(feed_idx) = app.selected_feed {
        if let Some(item_idx) = app.selected_item {
            match app.toggle_item_starred(feed_idx, item_idx) {
                Ok(is_now_starred) => {
                    app.success_message = Some(if is_now_starred {
                        "\u{2605} Starred".to_string()
                    } else {
                        "\u{2606} Unstarred".to_string()
                    });
                    app.success_message_time = Some(std::time::Instant::now());
                }
                Err(e) => {
                    app.error = Some(format!("Failed to toggle star: {}", e));
                }
            }
        }
    }
}

fn handle_toggle_read_current(app: &mut App) {
    if let Some(feed_idx) = app.selected_feed {
        if let Some(item_idx) = app.selected_item {
            match app.toggle_item_read(feed_idx, item_idx) {
                Ok(is_now_read) => {
                    app.success_message = Some(if is_now_read {
                        "\u{2713} Marked as read".to_string()
                    } else {
                        "\u{25CB} Marked as unread".to_string()
                    });
                    app.success_message_time = Some(std::time::Instant::now());
                }
                Err(e) => {
                    app.error = Some(format!("Failed to toggle read status: {}", e));
                }
            }
        }
    }
}

// ── Event entry point ──────────────────────────────────────────────

pub(crate) fn handle_events(app: &mut App) -> Result<bool> {
    let event = event::read()?;

    // Handle mouse events
    if let Event::Mouse(mouse) = &event {
        return handle_mouse_event(app, *mouse);
    }

    if let Event::Key(key) = event {
        return handle_key_event(app, key);
    }
    Ok(false)
}

pub(crate) fn handle_key_event(app: &mut App, key: crossterm::event::KeyEvent) -> Result<bool> {
    if matches!(key.kind, KeyEventKind::Release) {
        return Ok(false);
    }
    if app.error.is_some() {
        app.error = None;
        return Ok(false);
    }
    // Help overlay consumes all keys
    if app.show_help_overlay {
        if key.code == KeyCode::Esc
            || app.key_matches(KeyAction::Help, &key)
            || app.key_matches(KeyAction::Quit, &key)
            || app.key_matches(KeyAction::Back, &key)
        {
            app.show_help_overlay = false;
        } else if app.key_matches(KeyAction::MoveDown, &key) {
            app.help_overlay_scroll = app.help_overlay_scroll.saturating_add(1);
        } else if app.key_matches(KeyAction::MoveUp, &key) {
            app.help_overlay_scroll = app.help_overlay_scroll.saturating_sub(1);
        } else {
            app.show_help_overlay = false;
        }
        return Ok(false);
    }
    // Link overlay consumes all keys
    if app.show_link_overlay {
        if key.code == KeyCode::Esc
            || app.key_matches(KeyAction::Quit, &key)
            || app.key_matches(KeyAction::ExtractLinks, &key)
        {
            app.show_link_overlay = false;
        } else if app.key_matches(KeyAction::MoveDown, &key) {
            if !app.extracted_links.is_empty() {
                app.selected_link =
                    (app.selected_link + 1).min(app.extracted_links.len().saturating_sub(1));
            }
        } else if app.key_matches(KeyAction::MoveUp, &key) {
            app.selected_link = app.selected_link.saturating_sub(1);
        } else if app.key_matches(KeyAction::Select, &key)
            || app.key_matches(KeyAction::OpenInBrowser, &key)
        {
            if let Some(link) = app.extracted_links.get(app.selected_link) {
                if let Err(e) = open::that(&link.url) {
                    app.error = Some(format!("Failed to open link: {}", e));
                }
            }
        }
        return Ok(false);
    }
    // Force quit from any view
    if app.key_matches(KeyAction::ForceQuit, &key) {
        return Ok(true);
    }
    match app.input_mode {
        InputMode::Normal => match app.view {
            View::Dashboard => match key.code {
                // Keep hardcoded: demo feed shortcuts and tab switching
                KeyCode::Tab => {
                    if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                        app.view = View::Starred;
                        app.selected_item = None;
                    } else {
                        app.view = View::FeedList;
                        app.selected_item = None;
                    }
                }
                KeyCode::Char('1') => {
                    if app.feeds.is_empty() {
                        // Add Hacker News RSS
                        match app.add_feed("https://news.ycombinator.com/rss") {
                            Ok(AddFeedResult::Added) => {}
                            Ok(AddFeedResult::DiscoveredFeeds { .. }) => {
                                app.error =
                                    Some("URL returned an HTML page instead of a feed".to_string());
                            }
                            Err(e) => {
                                app.error = Some(format!("Failed to add feed: {}", e));
                            }
                        }
                    }
                }
                KeyCode::Char('2') => {
                    if app.feeds.is_empty() {
                        // Add TechCrunch RSS
                        match app.add_feed("https://feeds.feedburner.com/TechCrunch") {
                            Ok(AddFeedResult::Added) => {}
                            Ok(AddFeedResult::DiscoveredFeeds { .. }) => {
                                app.error =
                                    Some("URL returned an HTML page instead of a feed".to_string());
                            }
                            Err(e) => {
                                app.error = Some(format!("Failed to add feed: {}", e));
                            }
                        }
                    }
                }
                KeyCode::Char('3') => {
                    if app.feeds.is_empty() {
                        // Add NYTimes RSS
                        match app
                            .add_feed("https://rss.nytimes.com/services/xml/rss/nyt/HomePage.xml")
                        {
                            Ok(AddFeedResult::Added) => {}
                            Ok(AddFeedResult::DiscoveredFeeds { .. }) => {
                                app.error =
                                    Some("URL returned an HTML page instead of a feed".to_string());
                            }
                            Err(e) => {
                                app.error = Some(format!("Failed to add feed: {}", e));
                            }
                        }
                    }
                }
                // Configurable keybindings via match guards
                _ if app.key_matches(KeyAction::Quit, &key) => return Ok(true),
                // OpenCategoryManagement must come before CycleCategory
                // because Ctrl+c matches both (NONE modifier is a subset of any).
                _ if app.key_matches(KeyAction::OpenCategoryManagement, &key) => {
                    app.view = View::CategoryManagement;
                    app.selected_category = if !app.categories.is_empty() {
                        Some(0)
                    } else {
                        None
                    };
                }
                _ if app.key_matches(KeyAction::CycleCategory, &key) => {
                    let categories = app.get_available_categories();

                    if categories.is_empty() {
                        app.filter_options.category = None;
                    } else if app.filter_options.category.is_none() {
                        app.filter_options.category = Some(categories[0].clone());
                    } else if let Some(current) = app.filter_options.category.as_ref() {
                        let current_idx = categories.iter().position(|c| c == current);
                        if let Some(idx) = current_idx {
                            if idx + 1 < categories.len() {
                                app.filter_options.category = Some(categories[idx + 1].clone());
                            } else {
                                app.filter_options.category = None;
                            }
                        } else {
                            app.filter_options.category = Some(categories[0].clone());
                        }
                    }
                    app.apply_filters();
                }
                _ if app.key_matches(KeyAction::OpenFilter, &key) => {
                    app.filter_mode = true;
                    app.input_mode = InputMode::FilterMode;
                }
                _ if app.key_matches(KeyAction::AddFeed, &key) => {
                    app.input.clear();
                    app.input_mode = InputMode::InsertUrl;
                }
                _ if app.key_matches(KeyAction::Refresh, &key) => {
                    handle_refresh(app);
                }
                _ if app.key_matches(KeyAction::ToggleTheme, &key) => {
                    handle_toggle_theme(app);
                }
                _ if app.key_matches(KeyAction::OpenSearch, &key) => {
                    handle_open_search(app);
                }
                _ if app.key_matches(KeyAction::ToggleStar, &key) => {
                    if let Some(selected) = app.selected_item {
                        let active = app.active_dashboard_items();
                        let (feed_idx, item_idx) = if selected < active.len() {
                            active[selected]
                        } else {
                            return Ok(false);
                        };
                        match app.toggle_item_starred(feed_idx, item_idx) {
                            Ok(is_now_starred) => {
                                app.success_message = Some(if is_now_starred {
                                    "\u{2605} Starred".to_string()
                                } else {
                                    "\u{2606} Unstarred".to_string()
                                });
                                app.success_message_time = Some(std::time::Instant::now());
                            }
                            Err(e) => {
                                app.error = Some(format!("Failed to toggle star: {}", e));
                            }
                        }
                    }
                }
                _ if app.key_matches(KeyAction::TogglePreview, &key) => {
                    app.toggle_preview_pane();
                }
                _ if app.key_matches(KeyAction::ScrollPreviewUp, &key) && app.preview_pane => {
                    app.preview_scroll = app.preview_scroll.saturating_sub(1);
                }
                _ if app.key_matches(KeyAction::ScrollPreviewDown, &key) && app.preview_pane => {
                    if app.preview_scroll < app.preview_max_scroll {
                        app.preview_scroll = app.preview_scroll.saturating_add(1);
                    }
                }
                _ if app.key_matches(KeyAction::MoveUp, &key) => {
                    if let Some(selected) = app.selected_item {
                        if selected > 0 {
                            app.selected_item = Some(selected - 1);
                            app.reset_preview_scroll();
                        }
                    } else if !app.active_dashboard_items().is_empty() {
                        app.selected_item = Some(0);
                    }
                }
                _ if app.key_matches(KeyAction::MoveDown, &key) => {
                    if let Some(selected) = app.selected_item {
                        let len = app.active_dashboard_items().len();
                        if selected < len.saturating_sub(1) {
                            app.selected_item = Some(selected + 1);
                            app.reset_preview_scroll();
                        }
                    } else if !app.active_dashboard_items().is_empty() {
                        app.selected_item = Some(0);
                    }
                }
                _ if app.key_matches(KeyAction::Select, &key) => {
                    if let Some(selected) = app.selected_item {
                        let active = app.active_dashboard_items();
                        if selected < active.len() {
                            let (feed_idx, item_idx) = active[selected];
                            app.selected_feed = Some(feed_idx);
                            app.selected_item = Some(item_idx);
                            app.view = View::FeedItemDetail;
                            // Auto-mark as read when viewing detail
                            if let Err(e) = app.mark_item_as_read(feed_idx, item_idx) {
                                app.error = Some(format!("Failed to mark item as read: {}", e));
                            }
                        }
                    }
                }
                _ if app.key_matches(KeyAction::OpenInBrowser, &key) => {
                    if let Some(selected) = app.selected_item {
                        if let Some((_, item)) = app.active_dashboard_item(selected) {
                            if let Some(link) = &item.link {
                                if let Err(e) = open::that(link) {
                                    app.error = Some(format!("Failed to open link: {}", e));
                                }
                            }
                        }
                    }
                }
                _ if app.key_matches(KeyAction::ToggleRead, &key) => {
                    if let Some(selected) = app.selected_item {
                        let active = app.active_dashboard_items();
                        if selected < active.len() {
                            let (feed_idx, item_idx) = active[selected];
                            match app.toggle_item_read(feed_idx, item_idx) {
                                Ok(is_now_read) => {
                                    app.success_message = Some(if is_now_read {
                                        "\u{2713} Marked as read".to_string()
                                    } else {
                                        "\u{25CB} Marked as unread".to_string()
                                    });
                                    app.success_message_time = Some(std::time::Instant::now());
                                }
                                Err(e) => {
                                    app.error =
                                        Some(format!("Failed to toggle read status: {}", e));
                                }
                            }
                            // Reapply filters to update the display
                            app.apply_filters();
                        }
                    }
                }
                _ if app.key_matches(KeyAction::MarkAllRead, &key) => {
                    match app.mark_all_dashboard_read() {
                        Ok(count) => {
                            app.success_message =
                                Some(format!("\u{2713} Marked {} items as read", count));
                            app.success_message_time = Some(std::time::Instant::now());
                            app.apply_filters();
                        }
                        Err(e) => {
                            app.error = Some(format!("Failed to mark all read: {}", e));
                        }
                    }
                }
                _ if app.key_matches(KeyAction::Help, &key) => {
                    handle_show_help(app);
                }
                _ => {}
            },
            View::FeedList => match key.code {
                // Keep hardcoded: Tab for view switching
                KeyCode::Tab => {
                    if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                        app.view = View::Dashboard;
                        app.selected_item = None;
                    } else {
                        app.view = View::Starred;
                        app.selected_item = None;
                    }
                }
                // All other FeedList keys are configurable
                _ if app.key_matches(KeyAction::Quit, &key) => {
                    app.view = View::Dashboard;
                    app.selected_item = None;
                }
                _ if app.key_matches(KeyAction::Back, &key) => {
                    app.view = View::Dashboard;
                    app.selected_item = None;
                }
                _ if app.key_matches(KeyAction::DeleteFeed, &key) => {
                    if let Some(sel) = app.selected_tree_item {
                        match app.feed_tree.get(sel).cloned() {
                            Some(TreeItem::Feed(feed_idx, _)) => {
                                app.selected_feed = Some(feed_idx);
                                if let Err(e) = app.remove_current_feed() {
                                    app.error = Some(format!("Failed to remove feed: {}", e));
                                }
                                app.rebuild_feed_tree();
                            }
                            Some(TreeItem::Category(cat_idx)) => {
                                if let Err(e) = app.delete_category(cat_idx) {
                                    app.error = Some(format!("Failed to delete category: {}", e));
                                }
                            }
                            None => {}
                        }
                    }
                }
                _ if app.key_matches(KeyAction::Select, &key)
                    || app.key_matches(KeyAction::ToggleExpand, &key) =>
                {
                    if let Some(sel) = app.selected_tree_item {
                        match app.feed_tree.get(sel).cloned() {
                            Some(TreeItem::Feed(feed_idx, _)) => {
                                // Only Select opens a feed, ToggleExpand is a no-op on feeds
                                if app.key_matches(KeyAction::Select, &key) {
                                    app.selected_feed = Some(feed_idx);
                                    app.selected_item = Some(0);
                                    app.view = View::FeedItems;
                                }
                            }
                            Some(TreeItem::Category(cat_idx)) => {
                                if let Err(e) = app.toggle_category_expanded(cat_idx) {
                                    app.error = Some(format!("Failed to toggle category: {}", e));
                                }
                            }
                            None => {}
                        }
                    }
                }
                // OpenCategoryManagement before AssignCategory (modifier ordering)
                _ if app.key_matches(KeyAction::OpenCategoryManagement, &key) => {
                    app.view = View::CategoryManagement;
                    app.selected_category = if !app.categories.is_empty() {
                        Some(0)
                    } else {
                        None
                    };
                }
                _ if app.key_matches(KeyAction::AssignCategory, &key) => {
                    if let Some(sel) = app.selected_tree_item {
                        if let Some(TreeItem::Feed(feed_idx, _)) = app.feed_tree.get(sel) {
                            if *feed_idx < app.feeds.len() {
                                let feed_url = app.feeds[*feed_idx].url.clone();
                                app.category_action =
                                    Some(CategoryAction::AddFeedToCategory(feed_url));
                                app.view = View::CategoryManagement;
                            }
                        }
                    }
                }
                _ if app.key_matches(KeyAction::AddFeed, &key) => {
                    app.input.clear();
                    app.input_mode = InputMode::InsertUrl;
                }
                _ if app.key_matches(KeyAction::OpenSearch, &key) => {
                    handle_open_search(app);
                }
                _ if app.key_matches(KeyAction::Refresh, &key) => {
                    handle_refresh(app);
                }
                _ if app.key_matches(KeyAction::ToggleTheme, &key) => {
                    handle_toggle_theme(app);
                }
                _ if app.key_matches(KeyAction::MoveUp, &key) => {
                    if let Some(selected) = app.selected_tree_item {
                        if selected > 0 {
                            app.selected_tree_item = Some(selected - 1);
                        }
                    } else if !app.feed_tree.is_empty() {
                        app.selected_tree_item = Some(0);
                    }
                }
                _ if app.key_matches(KeyAction::MoveDown, &key) => {
                    if let Some(selected) = app.selected_tree_item {
                        if selected < app.feed_tree.len().saturating_sub(1) {
                            app.selected_tree_item = Some(selected + 1);
                        }
                    } else if !app.feed_tree.is_empty() {
                        app.selected_tree_item = Some(0);
                    }
                }
                _ if app.key_matches(KeyAction::MarkAllRead, &key) => {
                    if let Some(sel) = app.selected_tree_item {
                        match app.feed_tree.get(sel).cloned() {
                            Some(TreeItem::Feed(feed_idx, _)) => {
                                match app.mark_all_feed_read(feed_idx) {
                                    Ok(count) => {
                                        app.success_message = Some(format!(
                                            "\u{2713} Marked {} items as read",
                                            count
                                        ));
                                        app.success_message_time = Some(std::time::Instant::now());
                                    }
                                    Err(e) => {
                                        app.error = Some(format!("Failed to mark all read: {}", e))
                                    }
                                }
                            }
                            Some(TreeItem::Category(cat_idx)) => {
                                let feed_indices: Vec<usize> =
                                    if let Some(category) = app.categories.get(cat_idx) {
                                        let feed_urls: Vec<String> =
                                            category.feeds.iter().cloned().collect();
                                        app.feeds
                                            .iter()
                                            .enumerate()
                                            .filter(|(_, feed)| feed_urls.contains(&feed.url))
                                            .map(|(idx, _)| idx)
                                            .collect()
                                    } else {
                                        Vec::new()
                                    };
                                let mut total = 0;
                                for feed_idx in feed_indices {
                                    if let Ok(count) = app.mark_all_feed_read(feed_idx) {
                                        total += count;
                                    }
                                }
                                app.success_message =
                                    Some(format!("\u{2713} Marked {} items as read", total));
                                app.success_message_time = Some(std::time::Instant::now());
                            }
                            None => {}
                        }
                    }
                }
                _ if app.key_matches(KeyAction::Help, &key) => {
                    handle_show_help(app);
                }
                _ => {}
            },
            View::FeedItems => match key.code {
                // Configurable keybindings via match guards
                _ if app.key_matches(KeyAction::Quit, &key) => {
                    app.view = View::FeedList;
                    app.selected_item = None;
                }
                _ if app.key_matches(KeyAction::Back, &key) => {
                    app.view = View::FeedList;
                    app.selected_item = None;
                }
                _ if app.key_matches(KeyAction::Home, &key) => {
                    app.view = View::Dashboard;
                    app.selected_item = None;
                }
                _ if app.key_matches(KeyAction::ToggleStar, &key) => {
                    handle_toggle_star_current(app);
                }
                _ if app.key_matches(KeyAction::OpenSearch, &key) => {
                    handle_open_search(app);
                }
                _ if app.key_matches(KeyAction::Refresh, &key) => {
                    handle_refresh(app);
                }
                _ if app.key_matches(KeyAction::ToggleTheme, &key) => {
                    handle_toggle_theme(app);
                }
                _ if app.key_matches(KeyAction::MoveUp, &key) => {
                    if let Some(selected) = app.selected_item {
                        if selected > 0 {
                            app.selected_item = Some(selected - 1);
                        }
                    }
                }
                _ if app.key_matches(KeyAction::MoveDown, &key) => {
                    if let Some(selected) = app.selected_item {
                        if let Some(feed) = app.current_feed() {
                            if selected < feed.items.len().saturating_sub(1) {
                                app.selected_item = Some(selected + 1);
                            }
                        }
                    }
                }
                _ if app.key_matches(KeyAction::Select, &key) => {
                    if app.selected_item.is_some() {
                        app.view = View::FeedItemDetail;
                        if let Some(feed_idx) = app.selected_feed {
                            if let Some(item_idx) = app.selected_item {
                                if let Err(e) = app.mark_item_as_read(feed_idx, item_idx) {
                                    app.error = Some(format!("Failed to mark item as read: {}", e));
                                }
                            }
                        }
                    }
                }
                _ if app.key_matches(KeyAction::OpenInBrowser, &key) => {
                    if app.selected_item.is_some() {
                        if let Err(e) = app.open_current_item_in_browser() {
                            app.error = Some(format!("Failed to open link: {}", e));
                        }
                    }
                }
                _ if app.key_matches(KeyAction::ToggleRead, &key) => {
                    handle_toggle_read_current(app);
                }
                _ if app.key_matches(KeyAction::MarkAllRead, &key) => {
                    if let Some(feed_idx) = app.selected_feed {
                        match app.mark_all_feed_read(feed_idx) {
                            Ok(count) => {
                                app.success_message =
                                    Some(format!("\u{2713} Marked {} items as read", count));
                                app.success_message_time = Some(std::time::Instant::now());
                            }
                            Err(e) => {
                                app.error = Some(format!("Failed to mark all read: {}", e));
                            }
                        }
                    }
                }
                _ if app.key_matches(KeyAction::Help, &key) => {
                    handle_show_help(app);
                }
                _ => {}
            },
            View::FeedItemDetail => match key.code {
                // Keep hardcoded: page up/down with Ctrl guard, g/G jump, l for links
                KeyCode::PageUp | KeyCode::Char('u')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    // Scroll up by a larger amount (10 lines)
                    app.detail_vertical_scroll = app.detail_vertical_scroll.saturating_sub(10);
                    app.clamp_detail_scroll();
                }
                KeyCode::PageDown | KeyCode::Char('d')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    // Scroll down by a larger amount (10 lines), but not past the bottom
                    let new_scroll = app.detail_vertical_scroll.saturating_add(10);
                    app.detail_vertical_scroll = new_scroll.min(app.detail_max_scroll);
                }
                KeyCode::Char('g') => {
                    // Jump to the beginning (vim-style)
                    app.detail_vertical_scroll = 0;
                }
                KeyCode::Char('G') | KeyCode::End => {
                    // Jump to the end (vim-style with Shift or End key)
                    app.detail_vertical_scroll = app.detail_max_scroll;
                }
                KeyCode::Char('l') => {
                    app.extract_links_from_current_item();
                }
                // Configurable keybindings via match guards
                _ if app.key_matches(KeyAction::Quit, &key) => {
                    if app.is_searching {
                        app.exit_detail_view(View::Dashboard);
                        app.selected_item = Some(0);
                    } else {
                        app.exit_detail_view(View::FeedItems);
                    }
                }
                _ if app.key_matches(KeyAction::ToggleStar, &key) => {
                    handle_toggle_star_current(app);
                }
                _ if app.key_matches(KeyAction::Back, &key) => {
                    if app.is_searching {
                        app.exit_detail_view(View::Dashboard);
                        app.selected_item = Some(0);
                    } else {
                        app.exit_detail_view(View::FeedItems);
                    }
                }
                _ if app.key_matches(KeyAction::Home, &key) => {
                    app.exit_detail_view(View::Dashboard);
                    app.selected_item = None;
                }
                _ if app.key_matches(KeyAction::ToggleTheme, &key) => {
                    handle_toggle_theme(app);
                }
                _ if app.key_matches(KeyAction::MoveUp, &key) => {
                    app.detail_vertical_scroll = app.detail_vertical_scroll.saturating_sub(1);
                    app.clamp_detail_scroll();
                }
                _ if app.key_matches(KeyAction::MoveDown, &key) => {
                    if app.detail_vertical_scroll < app.detail_max_scroll {
                        app.detail_vertical_scroll = app.detail_vertical_scroll.saturating_add(1);
                    }
                }
                _ if app.key_matches(KeyAction::Refresh, &key) => {
                    handle_refresh(app);
                }
                _ if app.key_matches(KeyAction::OpenInBrowser, &key) => {
                    if let Err(e) = app.open_current_item_in_browser() {
                        app.error = Some(format!("Failed to open link: {}", e));
                    }
                }
                _ if app.key_matches(KeyAction::ToggleRead, &key) => {
                    handle_toggle_read_current(app);
                }
                _ if app.key_matches(KeyAction::OpenSearch, &key) => {
                    handle_open_search(app);
                }
                _ if app.key_matches(KeyAction::Help, &key) => {
                    handle_show_help(app);
                }
                _ => {}
            },
            View::Starred => match key.code {
                // Keep hardcoded: Tab for view switching
                KeyCode::Tab => {
                    if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                        app.view = View::FeedList;
                        app.selected_item = None;
                    } else {
                        app.view = View::Dashboard;
                        app.selected_item = None;
                    }
                }
                // Configurable keybindings via match guards
                _ if app.key_matches(KeyAction::Quit, &key) => {
                    app.view = View::Dashboard;
                    app.selected_item = None;
                }
                _ if app.key_matches(KeyAction::Back, &key) => {
                    app.view = View::Dashboard;
                    app.selected_item = None;
                }
                _ if app.key_matches(KeyAction::MoveUp, &key) => {
                    if let Some(selected) = app.selected_item {
                        if selected > 0 {
                            app.selected_item = Some(selected - 1);
                        }
                    } else {
                        let starred = app.get_starred_dashboard_items();
                        if !starred.is_empty() {
                            app.selected_item = Some(0);
                        }
                    }
                }
                _ if app.key_matches(KeyAction::MoveDown, &key) => {
                    let starred = app.get_starred_dashboard_items();
                    if let Some(selected) = app.selected_item {
                        if selected < starred.len().saturating_sub(1) {
                            app.selected_item = Some(selected + 1);
                        }
                    } else if !starred.is_empty() {
                        app.selected_item = Some(0);
                    }
                }
                _ if app.key_matches(KeyAction::Select, &key) => {
                    let starred = app.get_starred_dashboard_items();
                    if let Some(selected) = app.selected_item {
                        if selected < starred.len() {
                            let (feed_idx, item_idx) = starred[selected];
                            app.selected_feed = Some(feed_idx);
                            app.selected_item = Some(item_idx);
                            app.view = View::FeedItemDetail;
                            if let Err(e) = app.mark_item_as_read(feed_idx, item_idx) {
                                app.error = Some(format!("Failed to mark item as read: {}", e));
                            }
                        }
                    }
                }
                _ if app.key_matches(KeyAction::ToggleStar, &key) => {
                    let starred = app.get_starred_dashboard_items();
                    if let Some(selected) = app.selected_item {
                        if selected < starred.len() {
                            let (feed_idx, item_idx) = starred[selected];
                            match app.toggle_item_starred(feed_idx, item_idx) {
                                Ok(_) => {
                                    app.success_message = Some("\u{2606} Unstarred".to_string());
                                    app.success_message_time = Some(std::time::Instant::now());
                                    // Adjust selection after removal
                                    let new_starred = app.get_starred_dashboard_items();
                                    if new_starred.is_empty() {
                                        app.selected_item = None;
                                    } else if selected >= new_starred.len() {
                                        app.selected_item = Some(new_starred.len() - 1);
                                    }
                                }
                                Err(e) => {
                                    app.error = Some(format!("Failed to unstar: {}", e));
                                }
                            }
                        }
                    }
                }
                _ if app.key_matches(KeyAction::ToggleRead, &key) => {
                    let starred = app.get_starred_dashboard_items();
                    if let Some(selected) = app.selected_item {
                        if selected < starred.len() {
                            let (feed_idx, item_idx) = starred[selected];
                            match app.toggle_item_read(feed_idx, item_idx) {
                                Ok(is_now_read) => {
                                    app.success_message = Some(if is_now_read {
                                        "\u{2713} Marked as read".to_string()
                                    } else {
                                        "\u{25CB} Marked as unread".to_string()
                                    });
                                    app.success_message_time = Some(std::time::Instant::now());
                                }
                                Err(e) => {
                                    app.error =
                                        Some(format!("Failed to toggle read status: {}", e));
                                }
                            }
                        }
                    }
                }
                _ if app.key_matches(KeyAction::OpenInBrowser, &key) => {
                    let starred = app.get_starred_dashboard_items();
                    if let Some(selected) = app.selected_item {
                        if selected < starred.len() {
                            let (feed_idx, item_idx) = starred[selected];
                            let prev_feed = app.selected_feed;
                            app.selected_feed = Some(feed_idx);
                            app.selected_item = Some(item_idx);
                            if let Err(e) = app.open_current_item_in_browser() {
                                app.error = Some(format!("Failed to open link: {}", e));
                            }
                            // Restore selection for starred view
                            app.selected_feed = prev_feed;
                            app.selected_item = Some(selected);
                        }
                    }
                }
                _ if app.key_matches(KeyAction::ToggleTheme, &key) => {
                    handle_toggle_theme(app);
                }
                _ if app.key_matches(KeyAction::MarkAllRead, &key) => {
                    match app.mark_all_starred_read() {
                        Ok(count) => {
                            app.success_message =
                                Some(format!("\u{2713} Marked {} items as read", count));
                            app.success_message_time = Some(std::time::Instant::now());
                        }
                        Err(e) => {
                            app.error = Some(format!("Failed to mark all read: {}", e));
                        }
                    }
                }
                _ if app.key_matches(KeyAction::OpenSearch, &key) => {
                    handle_open_search(app);
                }
                _ if app.key_matches(KeyAction::Help, &key) => {
                    handle_show_help(app);
                }
                _ => {}
            },
            View::Summary => match key.code {
                KeyCode::Char('q') => {
                    app.view = View::Dashboard;
                    app.selected_item = Some(0);
                }
                _ => {
                    // Any key dismisses the summary
                    app.view = View::Dashboard;
                    app.selected_item = Some(0);
                }
            },
            View::CategoryManagement => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        // Return to previous view
                        app.view = View::FeedList;
                        app.category_action = None;
                    }
                    KeyCode::Char('t') => {
                        handle_toggle_theme(app);
                    }
                    KeyCode::Char('n') => {
                        // Create a new category
                        app.input.clear();
                        app.category_action = Some(CategoryAction::Create);
                        app.input_mode = InputMode::CategoryNameInput;
                    }
                    KeyCode::Char('e') if app.selected_category.is_some() => {
                        // Rename the selected category
                        if let Some(idx) = app.selected_category {
                            if idx < app.categories.len() {
                                app.input = app.categories[idx].name.clone();
                                app.category_action = Some(CategoryAction::Rename(idx));
                                app.input_mode = InputMode::CategoryNameInput;
                            }
                        }
                    }
                    KeyCode::Char('d') if app.selected_category.is_some() => {
                        // Delete the selected category
                        if let Some(idx) = app.selected_category {
                            if let Err(e) = app.delete_category(idx) {
                                app.error = Some(format!("Failed to delete category: {}", e));
                            }
                        }
                    }
                    KeyCode::Enter => {
                        // Add feed to category if that's the current action
                        if let Some(CategoryAction::AddFeedToCategory(ref feed_url)) =
                            app.category_action.clone()
                        {
                            if let Some(idx) = app.selected_category {
                                if let Err(e) = app.assign_feed_to_category(feed_url, idx) {
                                    app.error =
                                        Some(format!("Failed to assign feed to category: {}", e));
                                } else {
                                    // Success, go back to feed list
                                    app.view = View::FeedList;
                                    app.category_action = None;
                                }
                            }
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        // Select previous category
                        if let Some(selected) = app.selected_category {
                            if selected > 0 {
                                app.selected_category = Some(selected - 1);
                            }
                        } else if !app.categories.is_empty() {
                            app.selected_category = Some(0);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        // Select next category
                        if let Some(selected) = app.selected_category {
                            if selected < app.categories.len().saturating_sub(1) {
                                app.selected_category = Some(selected + 1);
                            }
                        } else if !app.categories.is_empty() {
                            app.selected_category = Some(0);
                        }
                    }
                    KeyCode::Char(' ') if app.selected_category.is_some() => {
                        // Toggle category expanded/collapsed
                        if let Some(idx) = app.selected_category {
                            if let Err(e) = app.toggle_category_expanded(idx) {
                                app.error = Some(format!("Failed to toggle category: {}", e));
                            }
                        }
                    }
                    KeyCode::Char('r') => {
                        // Remove a feed from the selected category
                        if let Some(CategoryAction::AddFeedToCategory(ref feed_url)) =
                            app.category_action.clone()
                        {
                            if let Some(idx) = app.selected_category {
                                if let Err(e) = app.remove_feed_from_category(feed_url, idx) {
                                    app.error =
                                        Some(format!("Failed to remove feed from category: {}", e));
                                }
                            }
                        }
                    }
                    KeyCode::Char('?') => {
                        app.show_help_overlay = true;
                        app.help_overlay_scroll = 0;
                    }
                    _ => {}
                }
            }
        },
        InputMode::InsertUrl => match key.code {
            KeyCode::Enter => {
                let url = app.input.trim().to_string();
                if !url.is_empty() {
                    match app.add_feed(&url) {
                        Ok(AddFeedResult::Added) => {}
                        Ok(AddFeedResult::DiscoveredFeeds { feeds, page_url }) => {
                            if feeds.is_empty() {
                                app.error = Some(format!(
                                    "No RSS/Atom feed links found on this page: {}",
                                    page_url
                                ));
                            } else {
                                app.discovered_feeds = feeds;
                                app.discovered_feed_selection = 0;
                                app.input_mode = InputMode::SelectDiscoveredFeed;
                                app.input.clear();
                                return Ok(false);
                            }
                        }
                        Err(e) => {
                            app.error = Some(format!("Failed to add feed: {}", e));
                        }
                    }
                }
                app.input.clear();
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                app.input.clear();
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) => {
                app.input.push(c);
            }
            KeyCode::Backspace => {
                app.input.pop();
            }
            _ => {}
        },
        InputMode::SelectDiscoveredFeed => match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if app.discovered_feed_selection > 0 {
                    app.discovered_feed_selection -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.discovered_feed_selection + 1 < app.discovered_feeds.len() {
                    app.discovered_feed_selection += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(discovered) = app.discovered_feeds.get(app.discovered_feed_selection) {
                    let feed_url = discovered.url.clone();
                    match app.add_feed(&feed_url) {
                        Ok(AddFeedResult::Added) => {}
                        Ok(AddFeedResult::DiscoveredFeeds { .. }) => {
                            app.error =
                                Some("Discovered feed URL also returned an HTML page".to_string());
                        }
                        Err(e) => {
                            app.error = Some(format!("Failed to add discovered feed: {}", e));
                        }
                    }
                }
                app.discovered_feeds.clear();
                app.discovered_feed_selection = 0;
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                app.discovered_feeds.clear();
                app.discovered_feed_selection = 0;
                app.input_mode = InputMode::Normal;
            }
            _ => {}
        },
        InputMode::SearchMode => match key.code {
            KeyCode::Enter => {
                // Results already shown live; just exit search input mode
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                app.input.clear();
                app.is_searching = false;
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) => {
                app.input.push(c);
                let query = app.input.clone();
                app.live_search(&query);
            }
            KeyCode::Backspace => {
                app.input.pop();
                let query = app.input.clone();
                app.live_search(&query);
            }
            _ => {}
        },
        InputMode::FilterMode => match key.code {
            KeyCode::Esc => {
                app.filter_mode = false;
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Char('c') => {
                let categories = app.get_available_categories();
                app.filter_options.category = if categories.is_empty() {
                    None
                } else {
                    match &app.filter_options.category {
                        None => Some(categories[0].clone()),
                        Some(current) => categories
                            .iter()
                            .position(|c| c == current)
                            .and_then(|idx| categories.get(idx + 1).cloned()),
                    }
                };
                app.apply_filters();
            }
            KeyCode::Char('t') => {
                // Cycle through time filters
                if app.filter_options.age.is_none() {
                    app.filter_options.age = Some(TimeFilter::Today);
                } else if app.filter_options.age == Some(TimeFilter::Today) {
                    app.filter_options.age = Some(TimeFilter::ThisWeek);
                } else if app.filter_options.age == Some(TimeFilter::ThisWeek) {
                    app.filter_options.age = Some(TimeFilter::ThisMonth);
                } else if app.filter_options.age == Some(TimeFilter::ThisMonth) {
                    app.filter_options.age = Some(TimeFilter::Older);
                } else {
                    app.filter_options.age = None;
                }
                app.apply_filters();
            }
            KeyCode::Char('a') => {
                // Toggle author filter
                app.filter_options.has_author = match app.filter_options.has_author {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
                app.apply_filters();
            }
            KeyCode::Char('r') => {
                // Toggle read status filter
                app.filter_options.read_status = match app.filter_options.read_status {
                    None => Some(true),        // Show read
                    Some(true) => Some(false), // Show unread
                    Some(false) => None,       // Show all
                };
                app.apply_filters();
            }
            KeyCode::Char('s') => {
                // Cycle through starred filter
                app.filter_options.starred_only = match app.filter_options.starred_only {
                    None => Some(true),        // Show starred
                    Some(true) => Some(false), // Show unstarred
                    Some(false) => None,       // Show all
                };
                app.apply_filters();
            }
            KeyCode::Char('l') => {
                // Cycle through content length filters
                app.filter_options.min_length = match app.filter_options.min_length {
                    None => Some(100),       // Short
                    Some(100) => Some(500),  // Medium
                    Some(500) => Some(1000), // Long
                    _ => None,               // All
                };
                app.apply_filters();
            }
            KeyCode::Char('x') => {
                // Clear all filters
                app.filter_options.reset();
                app.apply_filters();
            }
            _ => {}
        },
        InputMode::CategoryNameInput => {
            match key.code {
                KeyCode::Enter => {
                    // Process category name input
                    match app.category_action.clone() {
                        Some(CategoryAction::Create) => {
                            let input = app.input.clone();
                            if let Err(e) = app.create_category(&input) {
                                app.error = Some(format!("Failed to create category: {}", e));
                            }
                        }
                        Some(CategoryAction::Rename(idx)) => {
                            let input = app.input.clone();
                            if let Err(e) = app.rename_category(idx, &input) {
                                app.error = Some(format!("Failed to rename category: {}", e));
                            }
                        }
                        _ => {}
                    }
                    app.input.clear();
                    app.input_mode = InputMode::Normal;
                }
                KeyCode::Esc => {
                    // Cancel the operation
                    app.input.clear();
                    app.input_mode = InputMode::Normal;
                    app.category_action = None;
                }
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Char(c) => {
                    app.input.push(c);
                }
                _ => {}
            }
        }
    }
    Ok(false)
}

fn handle_mouse_event(app: &mut App, mouse: MouseEvent) -> Result<bool> {
    // Dismiss overlays on any click
    if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        if app.show_help_overlay {
            app.show_help_overlay = false;
            return Ok(false);
        }
        if app.show_link_overlay {
            app.show_link_overlay = false;
            return Ok(false);
        }
    }

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            // Scroll up — same as pressing 'k'
            if app.input_mode == InputMode::Normal {
                match app.view {
                    View::Dashboard => {
                        if app.preview_pane {
                            app.preview_scroll = app.preview_scroll.saturating_sub(3);
                        } else if let Some(selected) = app.selected_item {
                            if selected > 0 {
                                app.selected_item = Some(selected - 1);
                                app.reset_preview_scroll();
                            }
                        }
                    }
                    View::FeedList => {
                        if let Some(selected) = app.selected_tree_item {
                            if selected > 0 {
                                app.selected_tree_item = Some(selected - 1);
                            }
                        }
                    }
                    View::FeedItems => {
                        if let Some(selected) = app.selected_item {
                            if selected > 0 {
                                app.selected_item = Some(selected - 1);
                            }
                        }
                    }
                    View::FeedItemDetail => {
                        app.detail_vertical_scroll = app.detail_vertical_scroll.saturating_sub(3);
                        app.clamp_detail_scroll();
                    }
                    View::Starred => {
                        if let Some(selected) = app.selected_item {
                            if selected > 0 {
                                app.selected_item = Some(selected - 1);
                            }
                        }
                    }
                    View::CategoryManagement => {
                        if let Some(selected) = app.selected_category {
                            if selected > 0 {
                                app.selected_category = Some(selected - 1);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        MouseEventKind::ScrollDown => {
            // Scroll down — same as pressing 'j'
            if app.input_mode == InputMode::Normal {
                match app.view {
                    View::Dashboard => {
                        if app.preview_pane {
                            if app.preview_scroll < app.preview_max_scroll {
                                app.preview_scroll = app.preview_scroll.saturating_add(3);
                            }
                        } else if let Some(selected) = app.selected_item {
                            let len = app.active_dashboard_items().len();
                            if selected < len.saturating_sub(1) {
                                app.selected_item = Some(selected + 1);
                                app.reset_preview_scroll();
                            }
                        } else if !app.active_dashboard_items().is_empty() {
                            app.selected_item = Some(0);
                        }
                    }
                    View::FeedList => {
                        if let Some(selected) = app.selected_tree_item {
                            if selected < app.feed_tree.len().saturating_sub(1) {
                                app.selected_tree_item = Some(selected + 1);
                            }
                        } else if !app.feed_tree.is_empty() {
                            app.selected_tree_item = Some(0);
                        }
                    }
                    View::FeedItems => {
                        if let Some(selected) = app.selected_item {
                            if let Some(feed) = app.current_feed() {
                                if selected < feed.items.len().saturating_sub(1) {
                                    app.selected_item = Some(selected + 1);
                                }
                            }
                        }
                    }
                    View::FeedItemDetail => {
                        if app.detail_vertical_scroll < app.detail_max_scroll {
                            app.detail_vertical_scroll =
                                app.detail_vertical_scroll.saturating_add(3);
                        }
                    }
                    View::Starred => {
                        let starred_len = app.get_starred_dashboard_items().len();
                        if let Some(selected) = app.selected_item {
                            if selected < starred_len.saturating_sub(1) {
                                app.selected_item = Some(selected + 1);
                            }
                        } else if starred_len > 0 {
                            app.selected_item = Some(0);
                        }
                    }
                    View::CategoryManagement => {
                        if let Some(selected) = app.selected_category {
                            if selected < app.categories.len().saturating_sub(1) {
                                app.selected_category = Some(selected + 1);
                            }
                        } else if !app.categories.is_empty() {
                            app.selected_category = Some(0);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ExtractedLink, LinkType};
    use crate::feed::{Feed, FeedItem};
    use chrono::Utc;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn make_test_app() -> App {
        let mut app = App::new();
        app.feeds = vec![
            Feed {
                url: "https://example.com/feed1".to_string(),
                title: "Feed One".to_string(),
                title_lower: "feed one".to_string(),
                items: vec![
                    FeedItem {
                        title: "Old Article".to_string(),
                        title_lower: "old article".to_string(),
                        link: Some("https://example.com/old".to_string()),
                        description: Some("Old content".to_string()),
                        pub_date: None,
                        author: Some("Author A".to_string()),
                        formatted_date: None,
                        parsed_date: Some(Utc::now() - chrono::Duration::days(30)),
                        plain_text: Some("Old content".to_string()),
                    },
                    FeedItem {
                        title: "New Article".to_string(),
                        title_lower: "new article".to_string(),
                        link: Some("https://example.com/new".to_string()),
                        description: Some("New content".to_string()),
                        pub_date: None,
                        author: None,
                        formatted_date: None,
                        parsed_date: Some(Utc::now() - chrono::Duration::hours(1)),
                        plain_text: Some("New content".to_string()),
                    },
                ],
            },
            Feed {
                url: "https://example.com/feed2".to_string(),
                title: "Feed Two".to_string(),
                title_lower: "feed two".to_string(),
                items: vec![FeedItem {
                    title: "Another New".to_string(),
                    title_lower: "another new".to_string(),
                    link: Some("https://example.com/another".to_string()),
                    description: Some("Another new content".to_string()),
                    pub_date: None,
                    author: Some("Author B".to_string()),
                    formatted_date: None,
                    parsed_date: Some(Utc::now() - chrono::Duration::hours(2)),
                    plain_text: Some("Another new content".to_string()),
                }],
            },
        ];
        app.update_dashboard();
        app.rebuild_feed_tree();
        app
    }

    #[test]
    fn test_force_quit_from_any_view() {
        let views = vec![
            View::Dashboard,
            View::FeedList,
            View::FeedItems,
            View::FeedItemDetail,
            View::Starred,
            View::CategoryManagement,
            View::Summary,
        ];
        for view in views {
            let mut app = make_test_app();
            app.view = view.clone();
            let key = make_key(KeyCode::Char('q'), KeyModifiers::CONTROL);
            let result = handle_key_event(&mut app, key).unwrap();
            assert!(result, "Force quit should return true from {:?}", view);
        }
    }

    #[test]
    fn test_help_overlay_consumes_keys() {
        let mut app = make_test_app();
        app.show_help_overlay = true;
        app.view = View::Dashboard;

        // Random key should dismiss the overlay
        let key = make_key(KeyCode::Char('x'), KeyModifiers::NONE);
        let result = handle_key_event(&mut app, key).unwrap();
        assert!(!result);
        assert!(!app.show_help_overlay);
    }

    #[test]
    fn test_help_overlay_scroll() {
        let mut app = make_test_app();
        app.show_help_overlay = true;
        app.help_overlay_scroll = 0;

        // Scroll down
        let key = make_key(KeyCode::Char('j'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, key).unwrap();
        assert!(
            app.show_help_overlay,
            "Scrolling should not dismiss overlay"
        );
        assert_eq!(app.help_overlay_scroll, 1);

        // Scroll up
        let key = make_key(KeyCode::Char('k'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, key).unwrap();
        assert_eq!(app.help_overlay_scroll, 0);

        // Scroll up at 0 should not underflow
        let _ = handle_key_event(&mut app, key).unwrap();
        assert_eq!(app.help_overlay_scroll, 0);
    }

    #[test]
    fn test_help_overlay_esc_closes() {
        let mut app = make_test_app();
        app.show_help_overlay = true;

        let key = make_key(KeyCode::Esc, KeyModifiers::NONE);
        let result = handle_key_event(&mut app, key).unwrap();
        assert!(!result);
        assert!(!app.show_help_overlay);
    }

    #[test]
    fn test_help_overlay_does_not_quit() {
        let mut app = make_test_app();
        app.show_help_overlay = true;

        // 'q' is bound to Quit AND Help overlay close — should close overlay, not quit
        let key = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        let result = handle_key_event(&mut app, key).unwrap();
        assert!(!result, "Should not quit when help overlay is open");
        assert!(!app.show_help_overlay);
    }

    #[test]
    fn test_link_overlay_navigation() {
        let mut app = make_test_app();
        app.show_link_overlay = true;
        app.extracted_links = vec![
            ExtractedLink {
                url: "https://example.com/1".to_string(),
                text: "Link 1".to_string(),
                link_type: LinkType::Link,
            },
            ExtractedLink {
                url: "https://example.com/2".to_string(),
                text: "Link 2".to_string(),
                link_type: LinkType::Link,
            },
            ExtractedLink {
                url: "https://example.com/3".to_string(),
                text: "Link 3".to_string(),
                link_type: LinkType::Link,
            },
        ];
        app.selected_link = 0;

        // Move down
        let down = make_key(KeyCode::Char('j'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, down).unwrap();
        assert_eq!(app.selected_link, 1);
        assert!(app.show_link_overlay);

        // Move down again
        let _ = handle_key_event(&mut app, down).unwrap();
        assert_eq!(app.selected_link, 2);

        // Move down at end — should not go past last
        let _ = handle_key_event(&mut app, down).unwrap();
        assert_eq!(app.selected_link, 2);

        // Move up
        let up = make_key(KeyCode::Char('k'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, up).unwrap();
        assert_eq!(app.selected_link, 1);
    }

    #[test]
    fn test_link_overlay_esc_closes() {
        let mut app = make_test_app();
        app.show_link_overlay = true;
        app.extracted_links = vec![ExtractedLink {
            url: "https://example.com".to_string(),
            text: "Link".to_string(),
            link_type: LinkType::Link,
        }];

        let key = make_key(KeyCode::Esc, KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, key).unwrap();
        assert!(!app.show_link_overlay);
    }

    #[test]
    fn test_link_overlay_does_not_quit() {
        let mut app = make_test_app();
        app.show_link_overlay = true;
        app.extracted_links = vec![ExtractedLink {
            url: "https://example.com".to_string(),
            text: "Link".to_string(),
            link_type: LinkType::Link,
        }];

        let key = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        let result = handle_key_event(&mut app, key).unwrap();
        assert!(!result, "Should not quit when link overlay is open");
        assert!(!app.show_link_overlay);
    }

    #[test]
    fn test_error_dismissal_consumes_key() {
        let mut app = make_test_app();
        app.error = Some("Test error".to_string());
        app.view = View::Dashboard;

        // Any key should dismiss error without further action
        let key = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        let result = handle_key_event(&mut app, key).unwrap();
        assert!(!result, "Error dismissal should not quit");
        assert!(app.error.is_none());
        // Verify the key was consumed (we're still on Dashboard, not quitting)
        assert_eq!(app.view, View::Dashboard);
    }

    #[test]
    fn test_key_release_ignored() {
        let mut app = make_test_app();
        app.view = View::Dashboard;

        let key = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        let result = handle_key_event(&mut app, key).unwrap();
        assert!(!result, "Key release should be ignored");
    }

    #[test]
    fn test_tab_cycles_views_forward() {
        let mut app = make_test_app();
        app.view = View::Dashboard;

        let tab = make_key(KeyCode::Tab, KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, tab).unwrap();
        assert_eq!(app.view, View::FeedList);

        let _ = handle_key_event(&mut app, tab).unwrap();
        assert_eq!(app.view, View::Starred);

        let _ = handle_key_event(&mut app, tab).unwrap();
        assert_eq!(app.view, View::Dashboard);
    }

    #[test]
    fn test_shift_tab_cycles_views_backward() {
        let mut app = make_test_app();
        app.view = View::Dashboard;

        let shift_tab = make_key(KeyCode::Tab, KeyModifiers::SHIFT);
        let _ = handle_key_event(&mut app, shift_tab).unwrap();
        assert_eq!(app.view, View::Starred);

        let _ = handle_key_event(&mut app, shift_tab).unwrap();
        assert_eq!(app.view, View::FeedList);

        let _ = handle_key_event(&mut app, shift_tab).unwrap();
        assert_eq!(app.view, View::Dashboard);
    }

    #[test]
    fn test_dashboard_navigation() {
        let mut app = make_test_app();
        app.view = View::Dashboard;
        app.selected_item = None;

        // First MoveDown initializes selection to 0
        let down = make_key(KeyCode::Char('j'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, down).unwrap();
        assert_eq!(app.selected_item, Some(0));

        // Move down
        let _ = handle_key_event(&mut app, down).unwrap();
        assert_eq!(app.selected_item, Some(1));

        // Move up
        let up = make_key(KeyCode::Char('k'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, up).unwrap();
        assert_eq!(app.selected_item, Some(0));

        // Move up at 0 stays at 0
        let _ = handle_key_event(&mut app, up).unwrap();
        assert_eq!(app.selected_item, Some(0));
    }

    #[test]
    fn test_dashboard_quit() {
        let mut app = make_test_app();
        app.view = View::Dashboard;

        let key = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        let result = handle_key_event(&mut app, key).unwrap();
        assert!(result, "'q' on Dashboard should quit");
    }

    #[test]
    fn test_feedlist_quit_goes_to_dashboard() {
        let mut app = make_test_app();
        app.view = View::FeedList;

        let key = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        let result = handle_key_event(&mut app, key).unwrap();
        assert!(!result, "'q' on FeedList should not quit");
        assert_eq!(app.view, View::Dashboard);
    }

    #[test]
    fn test_help_opens_from_dashboard() {
        let mut app = make_test_app();
        app.view = View::Dashboard;

        let key = make_key(KeyCode::Char('?'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, key).unwrap();
        assert!(app.show_help_overlay);
        assert_eq!(app.help_overlay_scroll, 0);
    }

    #[test]
    fn test_mark_all_read_dashboard() {
        let mut app = make_test_app();
        app.view = View::Dashboard;
        app.read_items.clear();

        let key = make_key(KeyCode::Char('m'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, key).unwrap();

        assert!(app.is_item_read(0, 0));
        assert!(app.is_item_read(0, 1));
        assert!(app.is_item_read(1, 0));
        assert!(app.success_message.is_some());
    }

    #[test]
    fn test_feedlist_tree_navigation() {
        let mut app = make_test_app();
        app.view = View::FeedList;
        app.selected_tree_item = None;

        // First MoveDown initializes to 0
        let down = make_key(KeyCode::Char('j'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, down).unwrap();
        assert_eq!(app.selected_tree_item, Some(0));

        // Move down
        let _ = handle_key_event(&mut app, down).unwrap();
        assert_eq!(app.selected_tree_item, Some(1));

        // Move up
        let up = make_key(KeyCode::Char('k'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, up).unwrap();
        assert_eq!(app.selected_tree_item, Some(0));
    }

    #[test]
    fn test_mouse_scroll_down_dashboard() {
        let mut app = make_test_app();
        app.view = View::Dashboard;
        app.selected_item = Some(0);

        let mouse = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let _ = handle_mouse_event(&mut app, mouse).unwrap();
        assert_eq!(app.selected_item, Some(1));
    }

    #[test]
    fn test_mouse_click_dismisses_help() {
        let mut app = make_test_app();
        app.show_help_overlay = true;

        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let _ = handle_mouse_event(&mut app, mouse).unwrap();
        assert!(!app.show_help_overlay);
    }

    #[test]
    fn test_mouse_click_dismisses_link_overlay() {
        let mut app = make_test_app();
        app.show_link_overlay = true;

        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let _ = handle_mouse_event(&mut app, mouse).unwrap();
        assert!(!app.show_link_overlay);
    }

    #[test]
    fn test_category_management_navigation() {
        use crate::feed::FeedCategory;

        let mut app = make_test_app();
        app.view = View::CategoryManagement;

        // Create two categories
        let cat1 = FeedCategory::new("Tech");
        let cat2 = FeedCategory::new("News");
        app.categories = vec![cat1, cat2];
        app.selected_category = None;

        // 'j' selects first category when none selected
        let down = make_key(KeyCode::Char('j'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, down).unwrap();
        assert_eq!(app.selected_category, Some(0));

        // 'j' moves down
        let _ = handle_key_event(&mut app, down).unwrap();
        assert_eq!(app.selected_category, Some(1));

        // 'j' at end stays at end
        let _ = handle_key_event(&mut app, down).unwrap();
        assert_eq!(app.selected_category, Some(1));

        // 'k' moves up
        let up = make_key(KeyCode::Char('k'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, up).unwrap();
        assert_eq!(app.selected_category, Some(0));

        // 'k' at top stays at top
        let _ = handle_key_event(&mut app, up).unwrap();
        assert_eq!(app.selected_category, Some(0));

        // 'n' enters category name input mode
        let n = make_key(KeyCode::Char('n'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, n).unwrap();
        assert_eq!(app.input_mode, InputMode::CategoryNameInput);

        // Reset to normal mode for next test
        app.input_mode = InputMode::Normal;

        // 'd' deletes selected category
        app.selected_category = Some(1);
        let d = make_key(KeyCode::Char('d'), KeyModifiers::NONE);
        let _ = handle_key_event(&mut app, d).unwrap();
        assert_eq!(app.categories.len(), 1);
        assert_eq!(app.categories[0].name, "Tech");

        // 'q' goes back to FeedList
        let q = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        let result = handle_key_event(&mut app, q).unwrap();
        assert!(!result);
        assert_eq!(app.view, View::FeedList);
    }
}

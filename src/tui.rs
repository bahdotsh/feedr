use crate::app::{AddFeedResult, App, CategoryAction, InputMode, TimeFilter, View};
use crate::feed::Feed;
use crate::ui;
use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::sync::mpsc;
use std::{io, time::Duration};

pub fn run(mut app: App) -> Result<()> {
    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the main application loop
    let result = run_app(&mut terminal, &mut app);

    // Clean up terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Handle any errors from the application
    if let Err(err) = result {
        println!("Error: {:?}", err);
    }

    Ok(())
}

/// Spawn background threads to fetch all bookmarked feeds, sending results through the channel.
/// Returns the sender's pending count and the receiver.
fn spawn_feed_refresh(app: &mut App) -> (usize, mpsc::Receiver<(usize, Result<Feed>)>) {
    let (feed_tx, feed_rx) = mpsc::channel::<(usize, Result<Feed>)>();
    let mut pending_count: usize = 0;

    if !app.bookmarks.is_empty() {
        let timeout = app.config.network.http_timeout;
        let user_agent = app.config.network.user_agent.clone();
        let all_headers = app.feed_headers.clone();

        if let Ok(client) = Feed::build_client(timeout) {
            pending_count = app.bookmarks.len();
            app.is_loading = true;
            app.refresh_in_progress = true;
            for (idx, url) in app.bookmarks.iter().enumerate() {
                let client = client.clone();
                let url = url.clone();
                let ua = user_agent.clone();
                let tx = feed_tx.clone();
                let hdrs = all_headers.get(&url).cloned();
                std::thread::spawn(move || {
                    let result = Feed::fetch_url(&url, &client, Some(&ua), hdrs.as_ref())
                        .and_then(|r| r.into_feed());
                    let _ = tx.send((idx, result));
                });
            }
        }
    }

    (pending_count, feed_rx)
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = Duration::from_millis(app.config.ui.tick_rate);
    let error_timeout = Duration::from_millis(app.config.ui.error_display_timeout);

    // Initial load of bookmarked feeds
    let (mut pending_count, mut feed_rx) = spawn_feed_refresh(app);

    loop {
        terminal.draw(|f| {
            app.update_compact_mode(f.size().height);
            ui::render(f, app);
        })?;

        // Check if a refresh was requested (by 'r' key or auto-refresh)
        if app.refresh_requested {
            app.refresh_requested = false;
            if !app.refresh_in_progress {
                app.feeds.clear();
                app.update_dashboard();
                let (count, rx) = spawn_feed_refresh(app);
                pending_count = count;
                feed_rx = rx;
            }
        }

        // Drain any feeds that arrived from background threads
        if pending_count > 0 {
            while let Ok((idx, result)) = feed_rx.try_recv() {
                if let Ok(feed) = result {
                    // Insert at the correct position to maintain bookmark order,
                    // or append if earlier feeds haven't arrived yet
                    let insert_pos = app
                        .feeds
                        .iter()
                        .position(|f| {
                            app.bookmarks
                                .iter()
                                .position(|b| b == &f.url)
                                .unwrap_or(usize::MAX)
                                > idx
                        })
                        .unwrap_or(app.feeds.len());
                    app.feeds.insert(insert_pos, feed);
                    app.update_dashboard();
                }
                pending_count -= 1;
                if pending_count == 0 {
                    app.is_loading = false;
                    app.refresh_in_progress = false;
                    app.last_refresh = Some(std::time::Instant::now());
                    app.update_dashboard();
                    // Show summary view if there are new items since last session
                    if app.show_summary {
                        app.show_summary = false;
                        let (total, _) = app.get_summary_stats();
                        if total > 0 {
                            app.view = View::Summary;
                        }
                    }
                    // Save current time as session time now that feeds are loaded
                    let _ = app.save_data();
                }
            }
        }

        // If loading, use a shorter timeout for animation
        let timeout = if app.is_loading {
            tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0))
        } else if app.error.is_some() {
            error_timeout
        } else {
            tick_rate
        };

        if event::poll(timeout)? {
            // Handle user input
            if handle_events(app)? {
                return Ok(());
            }
        } else if last_tick.elapsed() >= tick_rate {
            // Update animation frame on tick
            if app.is_loading {
                app.update_loading_indicator();
            }

            // Clear error after timeout
            if app.error.is_some() && last_tick.elapsed() >= error_timeout {
                app.error = None;
            }

            // Clear success message after a shorter timeout (1.5 seconds)
            let success_timeout = Duration::from_millis(1500);
            if let Some(msg_time) = app.success_message_time {
                if app.success_message.is_some() && msg_time.elapsed() >= success_timeout {
                    app.success_message = None;
                    app.success_message_time = None;
                }
            }

            // Check if auto-refresh should trigger
            if app.should_auto_refresh() {
                app.refresh_requested = true;
            }

            last_tick = std::time::Instant::now();
        }
    }
}

fn handle_events(app: &mut App) -> Result<bool> {
    if let Event::Key(key) = event::read()? {
        if matches!(key.kind, KeyEventKind::Release) {
            return Ok(false);
        }
        if app.error.is_some() {
            app.error = None;
            return Ok(false);
        }
        // Ctrl+Q quits from any view
        if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(true);
        }
        match app.input_mode {
            InputMode::Normal => match app.view {
                View::Dashboard => match key.code {
                    KeyCode::Char('q') => return Ok(true),
                    KeyCode::Char('f') => {
                        app.filter_mode = true;
                        app.input_mode = InputMode::FilterMode;
                    }
                    KeyCode::Char('c') => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            // Switch to category management view
                            app.view = View::CategoryManagement;
                            app.selected_category = if !app.categories.is_empty() {
                                Some(0)
                            } else {
                                None
                            };
                        } else {
                            // Get available categories
                            let categories = app.get_available_categories();

                            if categories.is_empty() {
                                // No categories available, toggle off if on
                                app.filter_options.category = None;
                            } else {
                                // Cycle through available categories
                                if app.filter_options.category.is_none() {
                                    // Set to first category
                                    app.filter_options.category = Some(categories[0].clone());
                                } else {
                                    // Find current index and move to next
                                    let current = app.filter_options.category.as_ref().unwrap();
                                    let current_idx = categories.iter().position(|c| c == current);

                                    if let Some(idx) = current_idx {
                                        if idx < categories.len() - 1 {
                                            // Move to next category
                                            app.filter_options.category =
                                                Some(categories[idx + 1].clone());
                                        } else {
                                            // Wrap around to None
                                            app.filter_options.category = None;
                                        }
                                    } else {
                                        // Current category not found, set to first
                                        app.filter_options.category = Some(categories[0].clone());
                                    }
                                }
                            }
                            app.apply_filters();
                        }
                    }
                    KeyCode::Tab => {
                        if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                            app.view = View::Starred;
                            app.selected_item = None;
                        } else {
                            app.view = View::FeedList;
                            app.selected_item = None;
                        }
                    }
                    KeyCode::Char('a') => {
                        app.input.clear();
                        app.input_mode = InputMode::InsertUrl;
                    }
                    KeyCode::Char('r') => {
                        if !app.refresh_in_progress {
                            app.refresh_requested = true;
                        }
                    }
                    KeyCode::Char('t') => {
                        // Toggle theme
                        if let Err(e) = app.toggle_theme() {
                            app.error = Some(format!("Failed to toggle theme: {}", e));
                        } else {
                            app.success_message = Some("Theme toggled".to_string());
                            app.success_message_time = Some(std::time::Instant::now());
                        }
                    }
                    KeyCode::Char('/') => {
                        app.input.clear();
                        app.input_mode = InputMode::SearchMode;
                    }
                    KeyCode::Char('1') => {
                        if app.feeds.is_empty() {
                            // Add Hacker News RSS
                            match app.add_feed("https://news.ycombinator.com/rss") {
                                Ok(AddFeedResult::Added) => {}
                                Ok(AddFeedResult::DiscoveredFeeds { .. }) => {
                                    app.error = Some(
                                        "URL returned an HTML page instead of a feed".to_string(),
                                    );
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
                                    app.error = Some(
                                        "URL returned an HTML page instead of a feed".to_string(),
                                    );
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
                            match app.add_feed(
                                "https://rss.nytimes.com/services/xml/rss/nyt/HomePage.xml",
                            ) {
                                Ok(AddFeedResult::Added) => {}
                                Ok(AddFeedResult::DiscoveredFeeds { .. }) => {
                                    app.error = Some(
                                        "URL returned an HTML page instead of a feed".to_string(),
                                    );
                                }
                                Err(e) => {
                                    app.error = Some(format!("Failed to add feed: {}", e));
                                }
                            }
                        }
                    }
                    KeyCode::Char('s') => {
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
                    KeyCode::Char('p') => {
                        app.toggle_preview_pane();
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) && app.preview_pane {
                            // Scroll preview up
                            app.preview_scroll = app.preview_scroll.saturating_sub(1);
                        } else if let Some(selected) = app.selected_item {
                            if selected > 0 {
                                app.selected_item = Some(selected - 1);
                                app.reset_preview_scroll();
                            }
                        } else if !app.active_dashboard_items().is_empty() {
                            app.selected_item = Some(0);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) && app.preview_pane {
                            // Scroll preview down
                            if app.preview_scroll < app.preview_max_scroll {
                                app.preview_scroll = app.preview_scroll.saturating_add(1);
                            }
                        } else if let Some(selected) = app.selected_item {
                            let len = app.active_dashboard_items().len();
                            if len > 0 && selected < len - 1 {
                                app.selected_item = Some(selected + 1);
                                app.reset_preview_scroll();
                            }
                        } else if !app.active_dashboard_items().is_empty() {
                            app.selected_item = Some(0);
                        }
                    }
                    KeyCode::Enter => {
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
                    KeyCode::Char('o') => {
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
                    KeyCode::Char(' ') => {
                        if let Some(selected) = app.selected_item {
                            let active = app.active_dashboard_items();
                            if selected < active.len() {
                                let (feed_idx, item_idx) = active[selected];
                                match app.toggle_item_read(feed_idx, item_idx) {
                                    Ok(is_now_read) => {
                                        app.success_message = Some(if is_now_read {
                                            "✓ Marked as read".to_string()
                                        } else {
                                            "○ Marked as unread".to_string()
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
                    _ => {}
                },
                View::FeedList => match key.code {
                    KeyCode::Char('q') => {
                        app.view = View::Dashboard;
                        app.selected_item = None;
                    }
                    KeyCode::Tab => {
                        if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                            app.view = View::Dashboard;
                            app.selected_item = None;
                        } else {
                            app.view = View::Starred;
                            app.selected_item = None;
                        }
                    }
                    KeyCode::Char('a') => {
                        app.input.clear();
                        app.input_mode = InputMode::InsertUrl;
                    }
                    KeyCode::Char('d') => {
                        if let Err(e) = app.remove_current_feed() {
                            app.error = Some(format!("Failed to remove feed: {}", e));
                        }
                    }
                    KeyCode::Char('h') | KeyCode::Esc | KeyCode::Home => {
                        app.view = View::Dashboard;
                        app.selected_item = None;
                    }
                    KeyCode::Char('/') => {
                        app.input.clear();
                        app.input_mode = InputMode::SearchMode;
                    }
                    KeyCode::Char('r') => {
                        if !app.refresh_in_progress {
                            app.refresh_requested = true;
                        }
                    }
                    KeyCode::Char('t') => {
                        // Toggle theme
                        if let Err(e) = app.toggle_theme() {
                            app.error = Some(format!("Failed to toggle theme: {}", e));
                        } else {
                            app.success_message = Some("Theme toggled".to_string());
                            app.success_message_time = Some(std::time::Instant::now());
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let Some(selected) = app.selected_feed {
                            if selected > 0 {
                                app.selected_feed = Some(selected - 1);
                            }
                        } else if !app.feeds.is_empty() {
                            app.selected_feed = Some(0);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if let Some(selected) = app.selected_feed {
                            if selected < app.feeds.len() - 1 {
                                app.selected_feed = Some(selected + 1);
                            }
                        } else if !app.feeds.is_empty() {
                            app.selected_feed = Some(0);
                        }
                    }
                    KeyCode::Enter => {
                        if app.selected_feed.is_some() {
                            app.selected_item = Some(0);
                            app.view = View::FeedItems;
                        }
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Switch to category management view
                        app.view = View::CategoryManagement;
                        app.selected_category = if !app.categories.is_empty() {
                            Some(0)
                        } else {
                            None
                        };
                    }
                    KeyCode::Char('c')
                        if app.selected_feed.is_some()
                            && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        // Assign the selected feed to a category
                        if let Some(feed_idx) = app.selected_feed {
                            if feed_idx < app.feeds.len() {
                                let feed_url = app.feeds[feed_idx].url.clone();
                                app.category_action =
                                    Some(CategoryAction::AddFeedToCategory(feed_url));
                                app.view = View::CategoryManagement;
                            }
                        }
                    }
                    _ => {}
                },
                View::FeedItems => match key.code {
                    KeyCode::Char('q') => {
                        app.view = View::FeedList;
                        app.selected_item = None;
                    }
                    KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace => {
                        app.view = View::FeedList;
                        app.selected_item = None;
                    }
                    KeyCode::Home => {
                        app.view = View::Dashboard;
                        app.selected_item = None;
                    }
                    KeyCode::Char('s') => {
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
                    KeyCode::Char('/') => {
                        app.input.clear();
                        app.input_mode = InputMode::SearchMode;
                    }
                    KeyCode::Char('r') => {
                        if !app.refresh_in_progress {
                            app.refresh_requested = true;
                        }
                    }
                    KeyCode::Char('t') => {
                        // Toggle theme
                        if let Err(e) = app.toggle_theme() {
                            app.error = Some(format!("Failed to toggle theme: {}", e));
                        } else {
                            app.success_message = Some("Theme toggled".to_string());
                            app.success_message_time = Some(std::time::Instant::now());
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let Some(selected) = app.selected_item {
                            if selected > 0 {
                                app.selected_item = Some(selected - 1);
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if let Some(selected) = app.selected_item {
                            let feed = app.current_feed().unwrap();
                            if selected < feed.items.len() - 1 {
                                app.selected_item = Some(selected + 1);
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if app.selected_item.is_some() {
                            app.view = View::FeedItemDetail;
                            if let Some(feed_idx) = app.selected_feed {
                                if let Some(item_idx) = app.selected_item {
                                    if let Err(e) = app.mark_item_as_read(feed_idx, item_idx) {
                                        app.error =
                                            Some(format!("Failed to mark item as read: {}", e));
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('o') => {
                        if app.selected_item.is_some() {
                            if let Err(e) = app.open_current_item_in_browser() {
                                app.error = Some(format!("Failed to open link: {}", e));
                            }
                        }
                    }
                    KeyCode::Char(' ') => {
                        if let Some(feed_idx) = app.selected_feed {
                            if let Some(item_idx) = app.selected_item {
                                match app.toggle_item_read(feed_idx, item_idx) {
                                    Ok(is_now_read) => {
                                        app.success_message = Some(if is_now_read {
                                            "✓ Marked as read".to_string()
                                        } else {
                                            "○ Marked as unread".to_string()
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
                    _ => {}
                },
                View::FeedItemDetail => match key.code {
                    KeyCode::Char('q') => {
                        if app.is_searching {
                            app.exit_detail_view(View::Dashboard);
                            app.selected_item = Some(0);
                        } else {
                            app.exit_detail_view(View::FeedItems);
                        }
                    }
                    KeyCode::Char('s') => {
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
                    KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace => {
                        if app.is_searching {
                            // Return to search results
                            app.exit_detail_view(View::Dashboard);
                            app.selected_item = Some(0);
                        } else {
                            // Return to feed items
                            app.exit_detail_view(View::FeedItems);
                        }
                    }
                    KeyCode::Home => {
                        app.exit_detail_view(View::Dashboard);
                        app.selected_item = None;
                    }
                    KeyCode::Char('t') => {
                        // Toggle theme
                        if let Err(e) = app.toggle_theme() {
                            app.error = Some(format!("Failed to toggle theme: {}", e));
                        } else {
                            app.success_message = Some("Theme toggled".to_string());
                            app.success_message_time = Some(std::time::Instant::now());
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.detail_vertical_scroll = app.detail_vertical_scroll.saturating_sub(1);
                        // Clamping is done in the render function, but we can also clamp here
                        app.clamp_detail_scroll();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        // Only scroll down if we haven't reached the bottom
                        if app.detail_vertical_scroll < app.detail_max_scroll {
                            app.detail_vertical_scroll =
                                app.detail_vertical_scroll.saturating_add(1);
                        }
                    }
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
                    KeyCode::Char('r') => {
                        if !app.refresh_in_progress {
                            app.refresh_requested = true;
                        }
                    }
                    KeyCode::Char('o') => {
                        if let Err(e) = app.open_current_item_in_browser() {
                            app.error = Some(format!("Failed to open link: {}", e));
                        }
                    }
                    KeyCode::Char(' ') => {
                        if let Some(feed_idx) = app.selected_feed {
                            if let Some(item_idx) = app.selected_item {
                                match app.toggle_item_read(feed_idx, item_idx) {
                                    Ok(is_now_read) => {
                                        app.success_message = Some(if is_now_read {
                                            "✓ Marked as read".to_string()
                                        } else {
                                            "○ Marked as unread".to_string()
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
                    _ => {}
                },
                View::Starred => match key.code {
                    KeyCode::Char('q') => {
                        app.view = View::Dashboard;
                        app.selected_item = None;
                    }
                    KeyCode::Tab => {
                        if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                            app.view = View::FeedList;
                            app.selected_item = None;
                        } else {
                            app.view = View::Dashboard;
                            app.selected_item = None;
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('h') => {
                        app.view = View::Dashboard;
                        app.selected_item = None;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
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
                    KeyCode::Down | KeyCode::Char('j') => {
                        let starred = app.get_starred_dashboard_items();
                        if let Some(selected) = app.selected_item {
                            if selected < starred.len().saturating_sub(1) {
                                app.selected_item = Some(selected + 1);
                            }
                        } else if !starred.is_empty() {
                            app.selected_item = Some(0);
                        }
                    }
                    KeyCode::Enter => {
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
                    KeyCode::Char('s') => {
                        let starred = app.get_starred_dashboard_items();
                        if let Some(selected) = app.selected_item {
                            if selected < starred.len() {
                                let (feed_idx, item_idx) = starred[selected];
                                match app.toggle_item_starred(feed_idx, item_idx) {
                                    Ok(_) => {
                                        app.success_message =
                                            Some("\u{2606} Unstarred".to_string());
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
                    KeyCode::Char(' ') => {
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
                    KeyCode::Char('o') => {
                        let starred = app.get_starred_dashboard_items();
                        if let Some(selected) = app.selected_item {
                            if selected < starred.len() {
                                let (feed_idx, item_idx) = starred[selected];
                                app.selected_feed = Some(feed_idx);
                                app.selected_item = Some(item_idx);
                                if let Err(e) = app.open_current_item_in_browser() {
                                    app.error = Some(format!("Failed to open link: {}", e));
                                }
                                // Restore selection for starred view
                                app.selected_item = Some(selected);
                            }
                        }
                    }
                    KeyCode::Char('t') => {
                        if let Err(e) = app.toggle_theme() {
                            app.error = Some(format!("Failed to toggle theme: {}", e));
                        } else {
                            app.success_message = Some("Theme toggled".to_string());
                            app.success_message_time = Some(std::time::Instant::now());
                        }
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
                            // Toggle theme
                            if let Err(e) = app.toggle_theme() {
                                app.error = Some(format!("Failed to toggle theme: {}", e));
                            } else {
                                app.success_message = Some("Theme toggled".to_string());
                                app.success_message_time = Some(std::time::Instant::now());
                            }
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
                                        app.error = Some(format!(
                                            "Failed to assign feed to category: {}",
                                            e
                                        ));
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
                                if selected < app.categories.len() - 1 {
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
                                        app.error = Some(format!(
                                            "Failed to remove feed from category: {}",
                                            e
                                        ));
                                    }
                                }
                            }
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
                    if let Some(discovered) =
                        app.discovered_feeds.get(app.discovered_feed_selection)
                    {
                        let feed_url = discovered.url.clone();
                        match app.add_feed(&feed_url) {
                            Ok(AddFeedResult::Added) => {}
                            Ok(AddFeedResult::DiscoveredFeeds { .. }) => {
                                app.error = Some(
                                    "Discovered feed URL also returned an HTML page".to_string(),
                                );
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
    }
    Ok(false)
}

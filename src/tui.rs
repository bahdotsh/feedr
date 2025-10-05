use crate::app::{App, CategoryAction, InputMode, TimeFilter, View};
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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = Duration::from_millis(100); // 100ms for smooth animation

    loop {
        terminal.draw(|f| ui::render(f, app))?;

        // If loading, use a shorter timeout for animation
        let timeout = if app.is_loading {
            tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0))
        } else if app.error.is_some() {
            Duration::from_millis(3000)
        } else {
            Duration::from_millis(100)
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
            if app.error.is_some() && last_tick.elapsed() >= Duration::from_millis(3000) {
                app.error = None;
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
                        // Check if shift modifier is pressed
                        if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                            // With Shift+Tab, go from Feeds to Dashboard
                            if matches!(app.view, View::FeedList) {
                                app.view = View::Dashboard;
                            }
                        } else {
                            // With Tab, go from Dashboard to Feeds
                            if matches!(app.view, View::Dashboard) {
                                app.view = View::FeedList;
                            }
                        }
                    }
                    KeyCode::Char('a') => {
                        app.input.clear();
                        app.input_mode = InputMode::InsertUrl;
                    }
                    KeyCode::Char('r') => {
                        // Set loading flag before starting refresh
                        app.is_loading = true;

                        if let Err(e) = app.refresh_feeds() {
                            app.error = Some(format!("Failed to refresh feeds: {}", e));
                        }

                        // Refresh completed
                        app.is_loading = false;
                    }
                    KeyCode::Char('/') => {
                        app.input.clear();
                        app.input_mode = InputMode::SearchMode;
                    }
                    KeyCode::Char('1') => {
                        if app.feeds.is_empty() {
                            // Add Hacker News RSS
                            if let Err(e) = app.add_feed("https://news.ycombinator.com/rss") {
                                app.error = Some(format!("Failed to add feed: {}", e));
                            }
                        }
                    }
                    KeyCode::Char('2') => {
                        if app.feeds.is_empty() {
                            // Add TechCrunch RSS
                            if let Err(e) = app.add_feed("https://feeds.feedburner.com/TechCrunch")
                            {
                                app.error = Some(format!("Failed to add feed: {}", e));
                            }
                        }
                    }
                    KeyCode::Char('3') => {
                        if app.feeds.is_empty() {
                            // Add NYTimes RSS
                            if let Err(e) = app.add_feed(
                                "https://rss.nytimes.com/services/xml/rss/nyt/HomePage.xml",
                            ) {
                                app.error = Some(format!("Failed to add feed: {}", e));
                            }
                        }
                    }
                    KeyCode::Up => {
                        if let Some(selected) = app.selected_item {
                            if selected > 0 {
                                app.selected_item = Some(selected - 1);
                            }
                        } else if !app.dashboard_items.is_empty() {
                            app.selected_item = Some(0);
                        }
                    }
                    KeyCode::Down => {
                        if let Some(selected) = app.selected_item {
                            if selected < app.dashboard_items.len() - 1 {
                                app.selected_item = Some(selected + 1);
                            }
                        } else if !app.dashboard_items.is_empty() {
                            app.selected_item = Some(0);
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(selected) = app.selected_item {
                            if app.is_searching && selected < app.filtered_items.len() {
                                let (feed_idx, item_idx) = app.filtered_items[selected];
                                app.selected_feed = Some(feed_idx);
                                app.selected_item = Some(item_idx);
                                app.detail_vertical_scroll = 0; // Reset scroll when entering detail view
                                app.view = View::FeedItemDetail;
                            } else if selected < app.dashboard_items.len() {
                                let (feed_idx, item_idx) = app.dashboard_items[selected];
                                app.selected_feed = Some(feed_idx);
                                app.selected_item = Some(item_idx);
                                app.detail_vertical_scroll = 0; // Reset scroll when entering detail view
                                app.view = View::FeedItemDetail;
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
                    _ => {}
                },
                View::FeedList => match key.code {
                    KeyCode::Char('q') => return Ok(true),
                    KeyCode::Tab => {
                        app.view = View::Dashboard;
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
                        // Set loading flag before starting refresh
                        app.is_loading = true;

                        if let Err(e) = app.refresh_feeds() {
                            app.error = Some(format!("Failed to refresh feeds: {}", e));
                        }

                        // Refresh completed
                        app.is_loading = false;
                    }
                    KeyCode::Up => {
                        if let Some(selected) = app.selected_feed {
                            if selected > 0 {
                                app.selected_feed = Some(selected - 1);
                            }
                        } else if !app.feeds.is_empty() {
                            app.selected_feed = Some(0);
                        }
                    }
                    KeyCode::Down => {
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
                    KeyCode::Char('q') => return Ok(true),
                    KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace => {
                        app.view = View::FeedList;
                        app.selected_item = None;
                    }
                    KeyCode::Home => {
                        app.view = View::Dashboard;
                        app.selected_item = None;
                    }
                    KeyCode::Char('/') => {
                        app.input.clear();
                        app.input_mode = InputMode::SearchMode;
                    }
                    KeyCode::Char('r') => {
                        // Set loading flag before starting refresh
                        app.is_loading = true;

                        if let Err(e) = app.refresh_feeds() {
                            app.error = Some(format!("Failed to refresh feeds: {}", e));
                        }

                        // Refresh completed
                        app.is_loading = false;
                    }
                    KeyCode::Up => {
                        if let Some(selected) = app.selected_item {
                            if selected > 0 {
                                app.selected_item = Some(selected - 1);
                            }
                        }
                    }
                    KeyCode::Down => {
                        if let Some(selected) = app.selected_item {
                            let feed = app.current_feed().unwrap();
                            if selected < feed.items.len() - 1 {
                                app.selected_item = Some(selected + 1);
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if app.selected_item.is_some() {
                            app.detail_vertical_scroll = 0; // Reset scroll when entering detail view
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
                    _ => {}
                },
                View::FeedItemDetail => match key.code {
                    KeyCode::Char('q') => return Ok(true),
                    KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace => {
                        // Reset detail scroll to top when leaving.
                        app.detail_vertical_scroll = 0;
                        if app.is_searching {
                            // Return to search results
                            app.view = View::Dashboard;
                            app.selected_item = Some(0);
                        } else {
                            // Return to feed items
                            app.view = View::FeedItems;
                        }
                    }
                    KeyCode::Home => {
                        app.view = View::Dashboard;
                        app.selected_item = None;
                        // Reset detail scroll to top when leaving.
                        app.detail_vertical_scroll = 0;
                    }
                    KeyCode::Up => {
                        app.detail_vertical_scroll = app.detail_vertical_scroll.saturating_sub(1);
                        // Clamping is done in the render function, but we can also clamp here
                        app.clamp_detail_scroll();
                    }
                    KeyCode::Down => {
                        // Only scroll down if we haven't reached the bottom
                        if app.detail_vertical_scroll < app.detail_max_scroll {
                            app.detail_vertical_scroll =
                                app.detail_vertical_scroll.saturating_add(1);
                        }
                    }
                    KeyCode::PageUp => {
                        // Scroll up by a larger amount (10 lines)
                        app.detail_vertical_scroll = app.detail_vertical_scroll.saturating_sub(10);
                        app.clamp_detail_scroll();
                    }
                    KeyCode::PageDown => {
                        // Scroll down by a larger amount (10 lines), but not past the bottom
                        let new_scroll = app.detail_vertical_scroll.saturating_add(10);
                        app.detail_vertical_scroll = new_scroll.min(app.detail_max_scroll);
                    }
                    KeyCode::Char('g') => {
                        // Jump to the beginning (vim-style)
                        app.detail_vertical_scroll = 0;
                    }
                    KeyCode::Char('G') => {
                        // Jump to the end (vim-style with Shift)
                        app.detail_vertical_scroll = app.detail_max_scroll;
                    }
                    KeyCode::End => {
                        // Jump to the end
                        app.detail_vertical_scroll = app.detail_max_scroll;
                    }
                    KeyCode::Char('r') => {
                        // Set loading flag before starting refresh
                        app.is_loading = true;

                        if let Err(e) = app.refresh_feeds() {
                            app.error = Some(format!("Failed to refresh feeds: {}", e));
                        }

                        // Refresh completed
                        app.is_loading = false;
                    }
                    KeyCode::Char('o') => {
                        if let Err(e) = app.open_current_item_in_browser() {
                            app.error = Some(format!("Failed to open link: {}", e));
                        }
                    }
                    _ => {}
                },
                View::CategoryManagement => {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            // Return to previous view
                            app.view = View::FeedList;
                            app.category_action = None;
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
                        KeyCode::Up => {
                            // Select previous category
                            if let Some(selected) = app.selected_category {
                                if selected > 0 {
                                    app.selected_category = Some(selected - 1);
                                }
                            } else if !app.categories.is_empty() {
                                app.selected_category = Some(0);
                            }
                        }
                        KeyCode::Down => {
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
                            Ok(_) => {}
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
            InputMode::SearchMode => match key.code {
                KeyCode::Enter => {
                    let query = app.input.trim().to_string();
                    app.search_feeds(&query);
                    app.selected_item = Some(0);
                    app.view = View::Dashboard; // Show search results in dashboard
                    app.input_mode = InputMode::Normal;
                }
                KeyCode::Esc => {
                    app.input.clear();
                    app.is_searching = false;
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
            InputMode::FilterMode => match key.code {
                KeyCode::Esc => {
                    app.filter_mode = false;
                    app.input_mode = InputMode::Normal;
                }
                KeyCode::Char('c') => {
                    // Toggle category filter
                    if app.filter_options.category.is_none() {
                        // Cycle through available categories (tech, news, etc.)
                        app.filter_options.category = Some("tech".to_string());
                    } else if app.filter_options.category.as_deref() == Some("tech") {
                        app.filter_options.category = Some("news".to_string());
                    } else if app.filter_options.category.as_deref() == Some("news") {
                        app.filter_options.category = Some("science".to_string());
                    } else {
                        app.filter_options.category = None;
                    }
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

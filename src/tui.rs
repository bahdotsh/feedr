use crate::app::{App, InputMode, View};
use crate::ui;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
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
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        // Handle timeout for clearing error messages
        if app.error.is_some() {
            if event::poll(Duration::from_millis(3000))? {
                // An event arrived before the timeout, handle it
                if handle_events(app)? {
                    return Ok(()); // Exit the app if handle_events returns true
                }
            } else {
                // Timeout expired, clear the error
                app.error = None;
            }
        } else if event::poll(Duration::from_millis(100))? {
            // No error is displaying, poll for events normally
            if handle_events(app)? {
                return Ok(()); // Exit the app if handle_events returns true
            }
        }
    }
}

fn handle_events(app: &mut App) -> Result<bool> {
    if let Event::Key(key) = event::read()? {
        match app.input_mode {
            InputMode::Normal => match app.view {
                View::Dashboard => match key.code {
                    KeyCode::Char('q') => return Ok(true),
                    KeyCode::Char('a') => {
                        app.input.clear();
                        app.input_mode = InputMode::InsertUrl;
                    }
                    KeyCode::Char('r') => {
                        if let Err(e) = app.refresh_feeds() {
                            app.error = Some(format!("Failed to refresh feeds: {}", e));
                        }
                    }
                    KeyCode::Char('f') => {
                        app.view = View::FeedList;
                    }
                    KeyCode::Char('/') => {
                        app.input.clear();
                        app.input_mode = InputMode::SearchMode;
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
                                app.view = View::FeedItemDetail;
                            } else if selected < app.dashboard_items.len() {
                                let (feed_idx, item_idx) = app.dashboard_items[selected];
                                app.selected_feed = Some(feed_idx);
                                app.selected_item = Some(item_idx);
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
                        if let Err(e) = app.refresh_feeds() {
                            app.error = Some(format!("Failed to refresh feeds: {}", e));
                        }
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
                            app.view = View::FeedItemDetail;
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
                    }
                    KeyCode::Char('o') => {
                        if let Err(e) = app.open_current_item_in_browser() {
                            app.error = Some(format!("Failed to open link: {}", e));
                        }
                    }
                    _ => {}
                },
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
        }
    }
    Ok(false)
}

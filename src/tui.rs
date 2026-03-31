use crate::app::{App, View};
use crate::events::handle_events;
use crate::feed::Feed;
use crate::ui;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
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
                app.rebuild_feed_tree();
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
                    app.rebuild_feed_tree();
                }
                pending_count -= 1;
                if pending_count == 0 {
                    app.is_loading = false;
                    app.refresh_in_progress = false;
                    let now = std::time::Instant::now();
                    app.last_refresh = Some(now);
                    for url in &app.bookmarks {
                        app.last_feed_refresh.insert(url.clone(), now);
                    }
                    app.update_dashboard();
                    app.rebuild_feed_tree();
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

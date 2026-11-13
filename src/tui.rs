use crate::app::{App, View};
use crate::events::handle_events;
use crate::feed::{ExtractedArticle, Feed};
use crate::ui;
use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::io::Write;
use std::panic::AssertUnwindSafe;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::{io, time::Duration};

/// Hard cap on extraction worker threads in flight. The previous code spawned
/// one thread per queued request unconditionally, which fanned out as 20+
/// concurrent HTTP requests on a refresh of several `fulltext = true` feeds —
/// often to the same host. A small pool plus per-domain throttling matches
/// the politeness budget the feed-refresh path already uses.
const EXTRACTION_MAX_INFLIGHT: usize = 4;

/// Saturating decrement of the inflight counter — used by BOTH the worker's
/// exit path and the watchdog. The watchdog releases a slot for any Pending
/// it flips to `Failed("timed out")` so a truly dead worker can't leak
/// budget for the rest of the session. If both paths fire for the same
/// slot (slow worker the watchdog wrote off, then completed anyway), the
/// saturating sub clamps at zero instead of underflowing to usize::MAX —
/// which would effectively turn the bound into "spawn as many workers as
/// you want until the heap runs out".
fn release_inflight_slot(counter: &AtomicUsize) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |x| {
        Some(x.saturating_sub(1))
    });
}

/// RAII guard that re-enters the alt-screen + raw mode + mouse capture on drop,
/// so a panic during a pipe-to child invocation cannot leave the terminal in
/// a broken state.
struct TerminalRestoreGuard;

impl TerminalRestoreGuard {
    fn enter() -> io::Result<Self> {
        // Leave the TUI before handing the tty to the child.
        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
        disable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        // Best-effort restore — ignore errors during unwind.
        let _ = enable_raw_mode();
        let _ = execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture);
    }
}

/// Suspend the TUI, run argv with the given stdin payload (or inherited stdin
/// if `stdin_payload` is None), then re-enter the TUI and force a redraw.
/// Drains any pending terminal events so terminal-probe responses from the
/// child don't leak into our key handler.
pub fn suspend_for_command<B: Backend>(
    terminal: &mut Terminal<B>,
    argv: &[String],
    stdin_payload: Option<&[u8]>,
) -> Result<std::process::ExitStatus> {
    if argv.is_empty() {
        anyhow::bail!("empty command");
    }
    let status = {
        let _guard = TerminalRestoreGuard::enter()?;
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        if stdin_payload.is_some() {
            cmd.stdin(Stdio::piped());
        }
        // stdout/stderr inherited so user sees output of the child.
        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn '{}'", argv[0]))?;
        // Write the stdin payload from a dedicated thread so a payload larger
        // than the pipe buffer (typically 64 KiB on Linux) cannot deadlock the
        // parent against a child that hasn't started reading yet.
        let writer = match (stdin_payload, child.stdin.take()) {
            (Some(payload), Some(mut stdin)) => {
                let payload = payload.to_vec();
                Some(std::thread::spawn(move || {
                    let _ = stdin.write_all(&payload);
                    // stdin drops here, closing the pipe so the child sees EOF.
                }))
            }
            _ => None,
        };
        let status = child.wait().context("failed to wait for child")?;
        if let Some(w) = writer {
            let _ = w.join();
        }
        status
    };
    // Drain any pending input that might have arrived during the child's run
    // (e.g. terminal-probe responses from less/vim).
    while event::poll(Duration::from_millis(0))? {
        let _ = event::read();
    }
    terminal.clear()?;
    Ok(status)
}

/// Spawn a child detached: stdio nulled, parent does not block. A reaper
/// thread waits on the child so it doesn't linger as a zombie for the
/// lifetime of the TUI session — `Child` left undropped on Unix would
/// otherwise stay `<defunct>` until parent exit. Used by exec_on_new where
/// we may fan out many children and must not block the main loop.
pub fn spawn_detached(argv: &[String]) -> Result<()> {
    if argv.is_empty() {
        anyhow::bail!("empty command");
    }
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn '{}'", argv[0]))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

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

/// Drain all queued macro steps in FIFO order. Action steps mutate app
/// state inline, PipeTo suspends the terminal for a blocking child, Exec
/// spawns detached. On the first failure of any step the rest of the queue
/// is dropped — a "failure" is any condition that surfaces `app.error`
/// (spawn failures, missing article context, or an Action that sets the
/// error itself, e.g. `open-in-browser` with no URL). PipeTo non-zero exits
/// are not treated as failures: a script that returns 1 to mean "skip" is a
/// reasonable usage, and the user already saw the child's output.
fn drain_macro_steps<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) {
    use crate::keybindings::MacroStep;
    while let Some(step) = app.pending_macro_steps.pop_front() {
        // Track whether this step introduced a new error so a chain like
        // `open-in-browser ; toggle-star` halts when `open-in-browser` fails,
        // matching the contract documented above and the PipeTo / Exec paths.
        let pre_error = app.error.is_some();
        match step {
            MacroStep::Action(a) => crate::events::dispatch_action(app, a),
            MacroStep::PipeTo {
                argv_template,
                stdin,
            } => {
                let (argv, payload) =
                    match crate::events::build_pipe_invocation(app, &argv_template, stdin) {
                        Some(v) => v,
                        None => {
                            app.error = Some("pipe-to: no article in focus".to_string());
                            app.pending_macro_steps.clear();
                            break;
                        }
                    };
                if let Err(e) = suspend_for_command(terminal, &argv, Some(&payload)) {
                    app.error = Some(format!("pipe-to: {}", e));
                    app.pending_macro_steps.clear();
                    break;
                }
            }
            MacroStep::Exec { argv_template } => {
                let argv = match crate::events::build_exec_invocation(app, &argv_template) {
                    Some(v) => v,
                    None => {
                        app.error = Some("exec: no article in focus".to_string());
                        app.pending_macro_steps.clear();
                        break;
                    }
                };
                if let Err(e) = spawn_detached(&argv) {
                    app.error = Some(format!("exec: {}", e));
                    app.pending_macro_steps.clear();
                    break;
                }
            }
        }
        if !pre_error && app.error.is_some() {
            app.pending_macro_steps.clear();
            break;
        }
    }
}

/// Build the argv list to spawn for each genuinely-new item on this feed.
/// `newly_seen` must come from `mark_feed_seen` — this helper consumes the
/// pre-computed indices so multiple consumers (exec_on_new + fulltext) can
/// share one `mark_feed_seen` call per feed arrival instead of double-marking.
///
/// Returns Vec rather than spawning + saving inline so a refresh batch can
/// `save_data` ONCE before spawning ANY child, instead of once per feed
/// arrival (write amplification of N for N bookmarks). Crash semantics still
/// AT-MOST-ONCE: see [`flush_exec_on_new`] for the persistence ordering.
///
/// No-op when the hook is not configured.
fn collect_exec_on_new(app: &App, feed: &Feed, newly_seen: &[usize]) -> Vec<Vec<String>> {
    let argv_template = match app.exec_on_new_template.as_ref() {
        Some(t) => t.clone(),
        None => return Vec::new(),
    };
    if newly_seen.is_empty() {
        return Vec::new();
    }
    newly_seen
        .iter()
        .filter_map(|&idx| {
            let item = feed.items.get(idx)?;
            let ctx = crate::app::article_context_from(feed, item);
            Some(crate::app::expand_argv_template(&argv_template, &ctx))
        })
        .collect()
}

/// For feeds with `fulltext = true`, queue an extraction request for each
/// newly-seen item. Uses the same newly-seen set as `exec_on_new` so an
/// item is only auto-extracted once (and not on the first ever fetch of a
/// feed — that one seeds the seen-set silently, matching the exec_on_new
/// no-firehose contract).
///
/// Auto-mode applies a scheme + private-IP allowlist (`is_safe_auto_url`):
/// the URL comes from feed XML, which a hostile feed could populate with
/// `http://169.254.169.254/...` or other internal targets to trick the
/// reader into probing the local network. Manual `Shift+F` is the user's
/// explicit action and bypasses this gate.
fn enqueue_fulltext_for_new(app: &mut App, feed: &Feed, newly_seen: &[usize]) {
    if !app.fulltext_feeds.contains(&feed.url) {
        return;
    }
    for &idx in newly_seen {
        let Some(item) = feed.items.get(idx) else {
            continue;
        };
        let Some(url) = item.link.as_deref() else {
            continue;
        };
        if !crate::feed::is_safe_auto_url(url) {
            continue;
        }
        let id = crate::app::make_item_id(feed, item);
        // Auto path → use the safe-redirect client. The URL came from feed
        // XML, so a hostile feed could publish a public-looking link that
        // 302s to RFC1918 / metadata endpoints; revalidating each hop closes
        // that window. Auto requests go to the back of the queue so an
        // explicit user `Shift+F` is never starved behind a refresh batch.
        app.request_extraction(id, url.to_string(), true, false);
    }
}

/// Persist the updated seen-set, then spawn all collected exec_on_new
/// children. The "save then spawn" order is the AT-MOST-ONCE guarantee:
/// a kill between save and spawn loses notifications, but a restart never
/// re-fires the same item. Matters for side-effecting hooks like
/// `wallabag-cli add` where a duplicate is actively wrong.
fn flush_exec_on_new(app: &mut App, pending: Vec<Vec<String>>) {
    if pending.is_empty() {
        return;
    }
    let _ = app.save_data();
    let mut first_failure_recorded = false;
    for argv in pending {
        if let Err(e) = spawn_detached(&argv) {
            if !first_failure_recorded {
                app.error = Some(format!("exec_on_new: {}", e));
                first_failure_recorded = true;
            }
        }
    }
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

/// Drain pending extraction requests up to the concurrency budget, applying
/// per-domain throttling that matches the existing feed-refresh policy.
/// Each spawned worker:
///   * fetches the article URL with the existing reqwest blocking client
///     (no per-feed auth headers — see `extract_article`)
///   * runs Readability and posts the result back through `tx`
///   * catches panics from `dom_smoothie` on hostile HTML and surfaces them
///     as a `Failed` result so the slot doesn't strand on `Pending` forever
///   * decrements `inflight` regardless of success / failure / panic, so the
///     pool can't deadlock when a worker dies
///
/// Requests deferred for throttling or budget reasons stay on the queue;
/// `run_app` calls this every loop iteration so they fire on the next tick.
fn spawn_pending_extractions(
    app: &mut App,
    tx: &mpsc::Sender<(String, u64, Result<ExtractedArticle>)>,
    inflight: &Arc<AtomicUsize>,
    manual_client: &mut Option<reqwest::blocking::Client>,
    safe_client: &mut Option<reqwest::blocking::Client>,
) {
    if app.pending_extraction_requests.is_empty() {
        return;
    }
    let timeout = app.config.network.http_timeout;
    let user_agent = app.config.network.user_agent.clone();
    // `manual_client` / `safe_client` are owned by `run_app` so the
    // `reqwest::blocking::Client`s are built at most once per session
    // instead of once per tick that has work. Lazy build still: if the
    // session never uses Shift+F we never construct the manual client,
    // and vice versa.
    let rate_limit = Duration::from_millis(app.config.general.refresh_rate_limit_delay);
    let now = std::time::Instant::now();

    // Pop only as many as the concurrency budget AND per-domain spacing
    // allow. Anything we can't run right now gets pushed back to the front
    // in original order so it retries next tick.
    let mut deferred: Vec<crate::app::ExtractionRequest> = Vec::new();
    loop {
        // Relaxed is sufficient: the counter is just a bound on concurrent
        // workers and doesn't synchronize any other memory. Result hand-off
        // happens through the mpsc channel, which carries its own ordering.
        let in_use = inflight.load(Ordering::Relaxed);
        if in_use >= EXTRACTION_MAX_INFLIGHT {
            break;
        }
        let Some(req) = app.pending_extraction_requests.pop_front() else {
            break;
        };
        // Drop stale requests whose slot was evicted by LRU or removed
        // because their feed was deleted — running them would burn an HTTP
        // round-trip only to have `record_extraction_result` discard the
        // outcome. Cheap monotonic-correctness gate; must come before the
        // throttle defer so a stale id doesn't bounce back onto the queue.
        // Match BOTH the Pending state and the request's generation —
        // an evicted-and-recreated slot keeps the same id but gets a fresh
        // generation, so a stale queued request must be discarded here.
        match app.extracted.get(&req.id) {
            Some(crate::app::ExtractionState::Pending { generation, .. })
                if *generation == req.generation => {}
            _ => continue,
        }
        let domain = App::extract_domain_from_url(&req.url);
        let too_soon = !rate_limit.is_zero()
            && app
                .last_domain_fetch
                .get(&domain)
                .map(|last| now.duration_since(*last) < rate_limit)
                .unwrap_or(false);
        if too_soon {
            deferred.push(req);
            continue;
        }
        // Build (or reuse) the client that matches this request's redirect
        // policy. A build failure here is permanent for this tick — re-queue
        // the request and stop.
        let client = if req.safe_redirects {
            match safe_client {
                Some(c) => c.clone(),
                None => match Feed::build_safe_redirect_client(timeout) {
                    Ok(c) => {
                        *safe_client = Some(c.clone());
                        c
                    }
                    Err(_) => {
                        app.pending_extraction_requests.push_front(req);
                        break;
                    }
                },
            }
        } else {
            match manual_client {
                Some(c) => c.clone(),
                None => match Feed::build_client(timeout) {
                    Ok(c) => {
                        *manual_client = Some(c.clone());
                        c
                    }
                    Err(_) => {
                        app.pending_extraction_requests.push_front(req);
                        break;
                    }
                },
            }
        };
        // Claim the budget slot before spawning so a concurrent caller (or
        // the very next loop iteration) sees the slot taken. We defer the
        // `last_domain_fetch` stamp until AFTER `thread::Builder::spawn`
        // returns Ok — a rare OS-level spawn failure should re-queue
        // without burning an extra rate-limit window for the next attempt.
        inflight.fetch_add(1, Ordering::Relaxed);
        let ua = user_agent.clone();
        let tx = tx.clone();
        let inflight_for_worker = Arc::clone(inflight);
        let req_id = req.id.clone();
        let req_url = req.url.clone();
        let req_generation = req.generation;
        // `thread::Builder::spawn` returns `io::Result` instead of panicking
        // on OS-level thread-creation failure (rare, but a panic from the
        // main TUI loop would crash the app). On failure, release the slot
        // and re-queue the request so the next tick retries.
        let spawn_result = std::thread::Builder::new().spawn(move || {
            // `extract_article` and (via dom_smoothie) Readability touch
            // arbitrary HTML; a panic from the worker would otherwise leave
            // the slot stuck on `Pending` for the rest of the session and
            // there'd be no way for the user to retry.
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                crate::feed::extract_article(&req_url, &client, Some(&ua))
            }));
            let result = match outcome {
                Ok(r) => r,
                Err(_) => Err(anyhow::anyhow!(
                    "extraction worker panicked (hostile HTML?)"
                )),
            };
            let _ = tx.send((req_id, req_generation, result));
            release_inflight_slot(&inflight_for_worker);
        });
        if spawn_result.is_err() {
            release_inflight_slot(inflight);
            app.pending_extraction_requests.push_front(req);
            break;
        }
        // Spawn succeeded — flip the slot's `spawned` flag so the watchdog
        // knows this slot actually holds an inflight permit. Without this,
        // a queue-stranded request (rate-limited beyond the watchdog
        // window) would have its inflight slot "released" by the watchdog
        // even though it never claimed one — silently widening the
        // effective worker cap by one per spurious release.
        app.mark_extraction_spawned(&req.id, req.generation);
        // Stamp the domain so the next request for the same host respects
        // the rate-limit. Done after spawn so a failed spawn doesn't leave
        // a stale stamp behind.
        if !domain.is_empty() {
            app.last_domain_fetch.insert(domain, now);
        }
    }
    // Re-enqueue throttled requests at the front, preserving their original
    // FIFO order. Without `.rev()` we'd flip them.
    for req in deferred.into_iter().rev() {
        app.pending_extraction_requests.push_front(req);
    }
}

/// Drain `app.pending_image_render` and emit Kitty graphics escapes to
/// place the image at the reserved rect. On view-change (no pending image
/// this frame but had one last frame), emit a clear-all so previous-frame
/// placements don't bleed into the next view. Errors are swallowed — a
/// failed image emit must never crash the TUI.
fn emit_inline_image(app: &mut App) -> Result<()> {
    use std::io::Write;
    if let Some(pi) = app.pending_image_render.take() {
        let mut stdout = io::stdout().lock();
        let _ = app
            .image_cache
            .render_at(&mut stdout, &pi.url, pi.col, pi.row, pi.cols, pi.rows);
        let _ = stdout.flush();
        app.last_frame_had_image = true;
    } else if app.last_frame_had_image {
        let mut stdout = io::stdout().lock();
        let _ = app.image_cache.clear_terminal(&mut stdout);
        let _ = stdout.flush();
        app.last_frame_had_image = false;
    }
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = Duration::from_millis(app.config.ui.tick_rate);
    let error_timeout = Duration::from_millis(app.config.ui.error_display_timeout);

    // Long-lived channel for extraction worker completions. Outlives any
    // individual refresh because manual `F` extractions can fire at any time.
    // The `u64` is the request's generation tag, echoed back by the worker so
    // `record_extraction_result` can drop late results from
    // evicted-and-recreated slots.
    let (extract_tx, extract_rx) = mpsc::channel::<(String, u64, Result<ExtractedArticle>)>();
    // Bounds concurrent extraction workers. Each spawned worker bumps this
    // on entry and drops it on exit (even on panic), so the pool never
    // deadlocks even if `dom_smoothie` blows up on hostile HTML.
    let extract_inflight = Arc::new(AtomicUsize::new(0));
    // HTTP clients for extraction workers. Owned at the loop level (not
    // rebuilt every call to `spawn_pending_extractions`) so a busy refresh
    // doesn't churn `reqwest::blocking::Client` instances per tick.
    // Lazy-built on first use of each path: a session that never hits
    // `Shift+F` never constructs `extract_manual_client`, and vice versa.
    let mut extract_manual_client: Option<reqwest::blocking::Client> = None;
    let mut extract_safe_client: Option<reqwest::blocking::Client> = None;

    // Initial load of bookmarked feeds
    let (mut pending_count, mut feed_rx) = spawn_feed_refresh(app);

    loop {
        // Drain any background-fetched inline images so the next render
        // can use them. Cheap (try_recv loop, no I/O).
        app.image_cache.poll_completed();

        terminal.draw(|f| {
            app.update_compact_mode(f.size().height);
            ui::render(f, app);
        })?;

        // Inline-image post-pass. Runs AFTER `terminal.draw()` so all
        // ratatui escapes have been flushed; emitting our Kitty escapes
        // here paints them on top of the (intentionally blank) image
        // strip the detail view reserved in `app.pending_image_render`.
        // On view-change we emit `clear_terminal` so a previous frame's
        // image doesn't bleed into a non-detail view.
        emit_inline_image(app)?;

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
            // Accumulate exec_on_new spawns across every feed arriving in this
            // tick, so we only `save_data` once before spawning. Saving per
            // feed (the previous shape) was N whole-file JSON writes per
            // refresh.
            let mut pending_exec: Vec<Vec<String>> = Vec::new();
            while let Ok((idx, result)) = feed_rx.try_recv() {
                if let Ok(feed) = result {
                    // Compute newly-seen ONCE per feed arrival, shared
                    // between exec_on_new and fulltext consumers. Skip the
                    // mark entirely when neither consumer needs it — keeps
                    // `seen_items` empty for users without hooks, matching
                    // the original zero-cost contract.
                    let needs_newly_seen = app.exec_on_new_template.is_some()
                        || app.fulltext_feeds.contains(&feed.url);
                    let newly_seen = if needs_newly_seen {
                        crate::app::mark_feed_seen(app, &feed)
                    } else {
                        Vec::new()
                    };
                    pending_exec.extend(collect_exec_on_new(app, &feed, &newly_seen));
                    enqueue_fulltext_for_new(app, &feed, &newly_seen);
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
            // Persist seen-set ONCE for the whole batch, then spawn. Done
            // outside the per-feed loop so a refresh that brings back items
            // for many feeds writes the JSON file once instead of N times.
            flush_exec_on_new(app, pending_exec);
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

        // Spawn workers for any extraction requests queued by events or
        // by the feed-refresh path above. Run every iteration so manual
        // `F` requests don't have to wait for a tick boundary.
        spawn_pending_extractions(
            app,
            &extract_tx,
            &extract_inflight,
            &mut extract_manual_client,
            &mut extract_safe_client,
        );

        // Drain completed extractions back into app state. This is what
        // flips a `Pending` entry to `Ready` / `Failed`, which the detail
        // view reads from on the next draw.
        while let Ok((id, generation, result)) = extract_rx.try_recv() {
            app.record_extraction_result(id, generation, result);
        }

        // Watchdog: flip any Pending slot older than `http_timeout * N` to
        // `Failed("timed out")`. Cost ceiling per tick is bounded by
        // EXTRACTED_CACHE_CAP (500) map reads — at the configured tick rate
        // that's ~5k reads/sec worst case, negligible. Covers the rare case
        // where a worker thread is killed externally (OOM, signal, SIGSEGV
        // in a native dep) and never sends its result. `prune_*` returns
        // the count of pruned slots that were actually *spawned* (held an
        // inflight permit); queue-stranded prunes don't release inflight,
        // so a long backlog can't shrink the effective pool. The matching
        // saturating sub in the worker's exit path makes a spurious wake-up
        // (worker actually still alive, finishes later) safe — at worst we
        // transiently overcommit by up to `EXTRACTION_MAX_INFLIGHT` slots
        // (every "released-but-still-alive" worker contributes one slot of
        // over-commit until it actually exits), never underflow.
        let pruned = app
            .prune_stale_pending_extractions(Duration::from_secs(app.config.network.http_timeout));
        for _ in 0..pruned {
            release_inflight_slot(&extract_inflight);
        }

        if event::poll(timeout)? {
            // Handle user input
            if handle_events(app)? {
                return Ok(());
            }
            drain_macro_steps(terminal, app);
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

            // Time out a stranded macro-prefix wait so the next unrelated
            // keystroke isn't silently consumed as a macro follow-up.
            if let Some(since) = app.awaiting_macro_key_since {
                if since.elapsed() >= success_timeout {
                    app.awaiting_macro_key = false;
                    app.awaiting_macro_key_since = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybindings::{KeyAction, MacroStep};
    use ratatui::backend::TestBackend;

    fn test_terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(80, 24)).unwrap()
    }

    #[test]
    fn test_drain_halts_when_action_sets_error() {
        // A chain where step 1 errors must not run step 2. MoveUp is not
        // supported in macros (sets app.error to a "not supported" message);
        // OpenInBrowser with no focused article sets a different error. With
        // halt-on-error the first message survives, because step 2 never runs.
        // Without the halt, step 2 would overwrite app.error and a follow-up
        // side-effecting step would run silently.
        let mut app = App::new();
        app.error = None;
        app.pending_macro_steps
            .push_back(MacroStep::Action(KeyAction::MoveUp));
        app.pending_macro_steps
            .push_back(MacroStep::Action(KeyAction::OpenInBrowser));

        let mut terminal = test_terminal();
        drain_macro_steps(&mut terminal, &mut app);

        let err = app.error.as_deref().unwrap_or("");
        assert!(
            err.contains("move-up") && err.contains("not supported"),
            "first error must survive; got: {}",
            err
        );
        assert!(
            !err.contains("open-in-browser"),
            "step 2 must not have run (would have overwritten the error); got: {}",
            err
        );
        assert!(app.pending_macro_steps.is_empty());
    }

    #[test]
    fn test_drain_continues_when_steps_succeed() {
        // Sanity: a clean chain runs every step. Two `help` steps are no-ops
        // that don't set app.error; both must execute and leave the queue empty.
        let mut app = App::new();
        app.error = None;
        app.show_help_overlay = false;
        app.pending_macro_steps
            .push_back(MacroStep::Action(KeyAction::Help));
        app.pending_macro_steps
            .push_back(MacroStep::Action(KeyAction::Help));

        let mut terminal = test_terminal();
        drain_macro_steps(&mut terminal, &mut app);

        assert!(app.pending_macro_steps.is_empty());
        assert!(app.error.is_none());
        assert!(app.show_help_overlay);
    }

    #[test]
    fn test_release_inflight_slot_saturates_at_zero() {
        // The watchdog and worker-exit paths can race for the same slot.
        // The saturating decrement must clamp at zero rather than wrapping
        // to usize::MAX — otherwise the inflight bound is meaningless and
        // the pool would spawn workers without limit.
        let counter = AtomicUsize::new(0);
        release_inflight_slot(&counter);
        release_inflight_slot(&counter);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_release_inflight_slot_decrements_above_zero() {
        let counter = AtomicUsize::new(3);
        release_inflight_slot(&counter);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
        release_inflight_slot(&counter);
        release_inflight_slot(&counter);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
        // One more — must still clamp.
        release_inflight_slot(&counter);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_prune_returns_count_so_caller_can_release_inflight() {
        // Regression test for the inflight-leak fix: the watchdog must report
        // how many slots it pruned so `run_app` can release that many inflight
        // budget slots. Without the count, a chain of dead workers would
        // saturate the pool for the rest of the session.
        use crate::app::ExtractionState;
        let mut app = App::new();
        app.request_extraction(
            "stuck1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        app.request_extraction(
            "stuck2".to_string(),
            "https://ex.com/b".to_string(),
            false,
            false,
        );
        // Forcibly age both Pending slots past the watchdog window AND
        // mark them spawned=true so they represent real stuck workers
        // (the case the watchdog's inflight-release is meant for).
        let aged = std::time::Instant::now()
            .checked_sub(Duration::from_secs(3600))
            .expect("Instant::now() - 1h must be representable");
        for id in ["stuck1", "stuck2"] {
            if let Some(ExtractionState::Pending { generation, .. }) = app.extracted.get(id) {
                let gen = *generation;
                app.extracted.insert(
                    id.to_string(),
                    ExtractionState::Pending {
                        generation: gen,
                        started_at: aged,
                        spawned: true,
                    },
                );
            }
        }
        let pruned = app.prune_stale_pending_extractions(Duration::from_secs(30));
        assert_eq!(
            pruned, 2,
            "watchdog must report the spawned-pruned count for inflight release"
        );
    }

    #[test]
    fn test_record_extraction_result_flips_pending_to_ready() {
        use crate::app::ExtractionState;
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        // The request's generation is on the queued ExtractionRequest the
        // worker would have spawned for. Echoing it back through
        // `record_extraction_result` simulates the channel hand-off.
        let gen = app.pending_extraction_requests[0].generation;
        app.record_extraction_result(
            "id1".to_string(),
            gen,
            Ok(ExtractedArticle {
                title: "T".to_string(),
                plain_text: "body".to_string(),
                byline: None,
                site_name: None,
                source_url: "https://ex.com/a".to_string(),
            }),
        );
        match app.extracted.get("id1") {
            Some(ExtractionState::Ready(a)) => assert_eq!(a.plain_text, "body"),
            other => panic!("expected Ready, got {:?}", other),
        }
    }

    fn build_feed_with_items(url: &str, items: Vec<(&str, Option<&str>)>) -> Feed {
        Feed {
            url: url.to_string(),
            title: "T".to_string(),
            title_lower: "t".to_string(),
            items: items
                .into_iter()
                .map(|(title, link)| crate::feed::FeedItem {
                    title: title.to_string(),
                    link: link.map(|s| s.to_string()),
                    description: None,
                    pub_date: None,
                    author: None,
                    formatted_date: None,
                    parsed_date: None,
                    plain_text: None,
                    title_lower: title.to_lowercase(),
                    media: Vec::new(),
                    thumbnail: None,
                })
                .collect(),
        }
    }

    /// Regression lock: the Kitty escapes are emitted after
    /// `terminal.draw()` returns, so a frame with a visible modal must not
    /// queue an image placement — it would paint on top of the modal.
    #[test]
    fn test_modal_suppresses_pending_image_render() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut app = App::new();
        let mut feed = build_feed_with_items(
            "https://ex.com/feed.xml",
            vec![("Post", Some("https://ex.com/a"))],
        );
        feed.items[0].thumbnail = Some("https://ex.com/t.png".to_string());
        app.feeds.push(feed);
        app.view = crate::app::View::FeedItemDetail;
        app.selected_feed = Some(0);
        app.selected_item = Some(0);
        // Pretend the thumbnail is already decoded and the terminal speaks
        // Kitty, so the detail renderer reserves the strip and queues a
        // placement.
        app.image_cache
            .force_protocol(crate::image::ImageProtocol::Kitty);
        app.image_cache.insert_image(
            "https://ex.com/t.png".to_string(),
            Some(std::sync::Arc::new(image::DynamicImage::new_rgb8(100, 100))),
        );

        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| crate::ui::render(f, &mut app)).unwrap();
        assert!(
            app.pending_image_render.is_some(),
            "modal-free frame should queue an image placement"
        );

        app.show_help_overlay = true;
        terminal.draw(|f| crate::ui::render(f, &mut app)).unwrap();
        assert!(
            app.pending_image_render.is_none(),
            "frame with a visible modal must not queue an image placement"
        );

        // Closing the modal restores the placement on the next frame.
        app.show_help_overlay = false;
        terminal.draw(|f| crate::ui::render(f, &mut app)).unwrap();
        assert!(app.pending_image_render.is_some());
    }

    /// Regression lock: the 10-row image strip must give way to the
    /// article body — no strip (and no placement) on short terminals or
    /// in compact mode, where header + strip would squeeze the body to
    /// near-zero rows.
    #[test]
    fn test_image_strip_suppressed_on_short_or_compact_terminals() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut app = App::new();
        let mut feed = build_feed_with_items(
            "https://ex.com/feed.xml",
            vec![("Post", Some("https://ex.com/a"))],
        );
        feed.items[0].thumbnail = Some("https://ex.com/t.png".to_string());
        app.feeds.push(feed);
        app.view = crate::app::View::FeedItemDetail;
        app.selected_feed = Some(0);
        app.selected_item = Some(0);
        app.image_cache
            .force_protocol(crate::image::ImageProtocol::Kitty);
        app.image_cache.insert_image(
            "https://ex.com/t.png".to_string(),
            Some(std::sync::Arc::new(image::DynamicImage::new_rgb8(100, 100))),
        );

        // Short terminal: detail area falls below the strip threshold.
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| crate::ui::render(f, &mut app)).unwrap();
        assert!(
            app.pending_image_render.is_none(),
            "short terminal must not reserve the image strip"
        );

        // Tall terminal but compact mode forced on: also suppressed.
        let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
        app.compact = true;
        terminal.draw(|f| crate::ui::render(f, &mut app)).unwrap();
        assert!(
            app.pending_image_render.is_none(),
            "compact mode must not reserve the image strip"
        );

        // Sanity: same tall terminal, compact off → strip + placement.
        app.compact = false;
        terminal.draw(|f| crate::ui::render(f, &mut app)).unwrap();
        assert!(app.pending_image_render.is_some());
    }

    #[test]
    fn test_enqueue_fulltext_for_new_skips_when_feed_not_opted_in() {
        let mut app = App::new();
        let feed = build_feed_with_items(
            "https://ex.com/feed.xml",
            vec![("Post", Some("https://ex.com/a"))],
        );
        // Not in fulltext_feeds → must be a no-op even with newly-seen indices.
        enqueue_fulltext_for_new(&mut app, &feed, &[0]);
        assert!(app.pending_extraction_requests.is_empty());
        assert!(app.extracted.is_empty());
    }

    #[test]
    fn test_enqueue_fulltext_for_new_enqueues_safe_urls_with_safe_flag() {
        let mut app = App::new();
        let feed = build_feed_with_items(
            "https://ex.com/feed.xml",
            vec![("Post", Some("https://ex.com/a"))],
        );
        app.fulltext_feeds.insert(feed.url.clone());
        enqueue_fulltext_for_new(&mut app, &feed, &[0]);
        assert_eq!(app.pending_extraction_requests.len(), 1);
        // Auto path must always tag the request safe_redirects=true so the
        // worker picks the redirect-revalidating client.
        assert!(app.pending_extraction_requests[0].safe_redirects);
    }

    /// Regression lock: `enqueue_fulltext_for_new` must short-circuit on
    /// items that already have ANY extraction state (Ready / Failed /
    /// Pending). The contract lives in `request_extraction`'s
    /// `contains_key` guard, but it's load-bearing — without it, a refresh
    /// would re-extract every Ready item on every cycle (burning bandwidth,
    /// thrashing the LRU) and double-queue Pending items for in-flight
    /// workers.
    #[test]
    fn test_enqueue_fulltext_for_new_short_circuits_on_existing_state() {
        use crate::app::ExtractionState;
        let mut app = App::new();
        let feed = build_feed_with_items(
            "https://ex.com/feed.xml",
            vec![
                ("Cached-Ready", Some("https://ex.com/ready")),
                ("Cached-Failed", Some("https://ex.com/failed")),
                ("Cached-Pending", Some("https://ex.com/pending")),
                ("Fresh", Some("https://ex.com/fresh")),
            ],
        );
        app.fulltext_feeds.insert(feed.url.clone());

        let id_ready = crate::app::make_item_id(&feed, &feed.items[0]);
        let id_failed = crate::app::make_item_id(&feed, &feed.items[1]);
        let id_pending = crate::app::make_item_id(&feed, &feed.items[2]);

        // Drive Ready: Pending → record_extraction_result(Ok).
        app.request_extraction(
            id_ready.clone(),
            "https://ex.com/ready".to_string(),
            true,
            false,
        );
        let gen = match app.extracted.get(&id_ready) {
            Some(ExtractionState::Pending { generation, .. }) => *generation,
            other => panic!("expected Pending, got {:?}", other),
        };
        app.record_extraction_result(
            id_ready,
            gen,
            Ok(ExtractedArticle {
                title: "T".into(),
                plain_text: "body".into(),
                byline: None,
                site_name: None,
                source_url: "https://ex.com/ready".into(),
            }),
        );

        // Drive Failed: Pending → record_extraction_result(Err).
        app.request_extraction(
            id_failed.clone(),
            "https://ex.com/failed".to_string(),
            true,
            false,
        );
        let gen = match app.extracted.get(&id_failed) {
            Some(ExtractionState::Pending { generation, .. }) => *generation,
            other => panic!("expected Pending, got {:?}", other),
        };
        app.record_extraction_result(id_failed, gen, Err(anyhow::anyhow!("boom")));

        // Drive Pending: just leave it.
        app.request_extraction(
            id_pending,
            "https://ex.com/pending".to_string(),
            true,
            false,
        );

        // Clear the queue so we only observe what the next call adds.
        app.pending_extraction_requests.clear();

        // Re-observe ALL four items. Only the Fresh one should be queued —
        // the other three already have state and must be skipped.
        enqueue_fulltext_for_new(&mut app, &feed, &[0, 1, 2, 3]);

        assert_eq!(
            app.pending_extraction_requests.len(),
            1,
            "items with existing extraction state must not be re-queued"
        );
        assert_eq!(
            app.pending_extraction_requests[0].url, "https://ex.com/fresh",
            "only the fresh item should be queued"
        );
    }

    #[test]
    fn test_enqueue_fulltext_for_new_drops_unsafe_urls() {
        let mut app = App::new();
        let feed = build_feed_with_items(
            "https://ex.com/feed.xml",
            vec![
                ("Internal", Some("http://192.168.1.1/admin")),
                ("Metadata", Some("http://169.254.169.254/")),
                ("Bad scheme", Some("javascript:alert(1)")),
                ("Localhost", Some("http://localhost/x")),
                ("No link", None),
                ("Public", Some("https://ex.com/a")),
            ],
        );
        app.fulltext_feeds.insert(feed.url.clone());
        enqueue_fulltext_for_new(&mut app, &feed, &[0, 1, 2, 3, 4, 5]);
        // Only the public https URL should make it through.
        assert_eq!(app.pending_extraction_requests.len(), 1);
        assert_eq!(app.pending_extraction_requests[0].url, "https://ex.com/a");
    }

    /// Regression lock for the "single shared mark per feed arrival" contract.
    /// `mark_feed_seen` must only be called ONCE per feed delivery, even when
    /// both `exec_on_new` and `fulltext` are consuming the newly-seen set —
    /// calling it twice would double-mark and the second consumer would see
    /// an empty set. The fix lives at the `run_app` call site (gating on
    /// `exec_on_new_template.is_some() || fulltext_feeds.contains(...)`) and
    /// then handing the same `&[usize]` to both consumers.
    #[test]
    fn test_mark_feed_seen_shared_between_exec_on_new_and_fulltext() {
        let mut app = App::new();
        // Configure BOTH consumers for this feed.
        app.exec_on_new_template = Some(vec!["echo".to_string(), "%t".to_string()]);
        let feed = build_feed_with_items(
            "https://ex.com/feed.xml",
            vec![
                ("First", Some("https://ex.com/1")),
                ("Second", Some("https://ex.com/2")),
            ],
        );
        app.fulltext_feeds.insert(feed.url.clone());

        // Simulate the first refresh: feed is not yet seeded, so the first
        // mark seeds silently and returns an empty newly-seen set.
        let first_seen = crate::app::mark_feed_seen(&mut app, &feed);
        assert!(
            first_seen.is_empty(),
            "first observation of a feed must seed silently"
        );
        // Both consumers see the same (empty) list — neither fires.
        let exec = collect_exec_on_new(&app, &feed, &first_seen);
        assert!(exec.is_empty(), "exec_on_new must not fire on first fetch");
        enqueue_fulltext_for_new(&mut app, &feed, &first_seen);
        assert!(
            app.pending_extraction_requests.is_empty(),
            "fulltext must not fire on first fetch"
        );

        // Second refresh delivers the SAME items + one new item. The single
        // mark_feed_seen call MUST return only the new index, and both
        // consumers fed the same slice MUST both observe it. A second
        // mark_feed_seen call here would return [] (double-mark bug).
        let feed2 = build_feed_with_items(
            "https://ex.com/feed.xml",
            vec![
                ("First", Some("https://ex.com/1")),
                ("Second", Some("https://ex.com/2")),
                ("Third", Some("https://ex.com/3")),
            ],
        );
        let second_seen = crate::app::mark_feed_seen(&mut app, &feed2);
        assert_eq!(
            second_seen,
            vec![2],
            "only the genuinely-new item should be reported"
        );
        // A duplicate mark — the bug this whole regression test guards
        // against — would now return an empty slice.
        let dup = crate::app::mark_feed_seen(&mut app, &feed2);
        assert!(
            dup.is_empty(),
            "calling mark_feed_seen twice for one arrival would starve the second consumer"
        );

        // The actual contract: both consumers fed the SAME `second_seen`
        // slice must both react to the new item.
        let exec = collect_exec_on_new(&app, &feed2, &second_seen);
        assert_eq!(exec.len(), 1, "exec_on_new must fire for the new item");
        enqueue_fulltext_for_new(&mut app, &feed2, &second_seen);
        assert_eq!(
            app.pending_extraction_requests.len(),
            1,
            "fulltext must enqueue for the new item from the same newly-seen slice"
        );
        assert_eq!(app.pending_extraction_requests[0].url, "https://ex.com/3");
    }

    #[test]
    fn test_drain_preserves_preexisting_error_and_still_runs_step() {
        // Edge case: if `app.error` is already set before a step runs, the
        // step itself didn't set it — we must not treat it as a step failure.
        // (In practice this can't happen because handle_events clears app.error
        // on the next keypress, but the drain function shouldn't depend on that
        // invariant.)
        let mut app = App::new();
        app.error = Some("stale error".to_string());
        app.show_help_overlay = false;
        app.pending_macro_steps
            .push_back(MacroStep::Action(KeyAction::Help));
        app.pending_macro_steps
            .push_back(MacroStep::Action(KeyAction::Help));

        let mut terminal = test_terminal();
        drain_macro_steps(&mut terminal, &mut app);

        // Both steps ran despite the preexisting error.
        assert!(app.pending_macro_steps.is_empty());
        assert!(app.show_help_overlay);
    }

    #[test]
    fn test_read_mode_renders_fullscreen_and_suppresses_image_strip() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut app = App::new();
        let mut feed = build_feed_with_items(
            "https://ex.com/feed.xml",
            vec![("Post Title", Some("https://ex.com/a"))],
        );
        feed.items[0].description = Some("<p>Body content here</p>".to_string());
        feed.items[0].thumbnail = Some("https://ex.com/t.png".to_string());
        app.feeds.push(feed);
        app.view = crate::app::View::FeedItemDetail;
        app.selected_feed = Some(0);
        app.selected_item = Some(0);
        app.image_cache
            .force_protocol(crate::image::ImageProtocol::Kitty);
        app.image_cache.insert_image(
            "https://ex.com/t.png".to_string(),
            Some(std::sync::Arc::new(image::DynamicImage::new_rgb8(100, 100))),
        );

        let buffer_text = |term: &Terminal<TestBackend>| -> String {
            let buf = term.backend().buffer();
            (0..buf.area.height)
                .map(|y| {
                    (0..buf.area.width)
                        .map(|x| buf.get(x, y).symbol.as_str())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
        terminal.draw(|f| crate::ui::render(f, &mut app)).unwrap();
        let normal_screen = buffer_text(&terminal);
        assert!(app.pending_image_render.is_some());
        assert!(normal_screen.contains("Dashboard"));
        assert!(normal_screen.contains("Post Title"));
        assert!(normal_screen.contains("Body content here"));

        app.read_mode = true;
        terminal.draw(|f| crate::ui::render(f, &mut app)).unwrap();
        let read_mode_screen = buffer_text(&terminal);
        assert!(
            app.pending_image_render.is_none(),
            "read mode must suppress the image strip"
        );
        assert!(
            !read_mode_screen.contains("Dashboard"),
            "read mode must hide the top tab bar"
        );
        assert!(
            !read_mode_screen.contains("Post Title"),
            "read mode must hide the article metadata header"
        );
        assert!(
            read_mode_screen.contains("Body content here"),
            "read mode must still render the article body"
        );
    }
}

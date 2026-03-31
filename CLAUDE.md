# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Feedr is a terminal-based RSS/Atom feed reader built with Rust, using ratatui/crossterm for the TUI. It supports feed management, categorization, filtering, dual themes, OPML import, auto-refresh with per-domain rate limiting, feed auto-discovery from HTML pages, configurable keybindings, mouse support, and a help overlay.

## Build & Development Commands

```bash
cargo build --release          # Build optimized binary (LTO enabled)
cargo run --release             # Run the app
cargo test --verbose            # Run all tests
cargo test --all-features --verbose  # Run tests with all features
cargo test <test_name>          # Run a single test
cargo clippy --all-targets --all-features -- -D warnings  # Lint (CI-strict)
cargo fmt --all                 # Format code
cargo fmt --all -- --check      # Check formatting without changing files
```

MSRV: 1.75.0. CI runs tests on stable, beta, and 1.75.0.

## Architecture

**Single-threaded synchronous TUI** — no async runtime. The main loop in `tui.rs` polls for keyboard events, mutates state, then renders.

### Core modules

- **`app.rs`** — `App` struct holding all application state. All state mutations (feed ops, filtering, categorization, persistence) happen through its methods. This is the largest and most central file.
- **`tui.rs`** — Terminal setup/teardown, main event loop (`run_app`), and feed refresh logic.
- **`events.rs`** — All keyboard and mouse event handling (`handle_events`). Input dispatches based on `View` × `InputMode` enums. Separated from `tui.rs` for maintainability.
- **`keybindings.rs`** — `KeyAction` enum, default keybinding map, key string parsing, and config-driven keybinding overrides via `[keybindings]` TOML section.
- **`feed.rs`** — Data models (`Feed`, `FeedItem`, `FeedCategory`), RSS/Atom parsing via `feed-rs`, and HTML feed auto-discovery via `scraper`.
- **`config.rs`** — XDG-compliant config loading/saving (`~/.config/feedr/config.toml`). Includes `keybindings: HashMap<String, toml::Value>` for custom key overrides. Auto-generates defaults on first run.
- **`config_cli.rs`** — CLI subcommand handler for `feedr config list/get/set`.
- **`config_tui.rs`** — Interactive TUI config editor (`feedr config --tui`).
- **`config_ui.rs`** — Rendering for the TUI config editor.
- **`main.rs`** — CLI arg parsing (clap) and OPML import entry point.

### UI modules (`src/ui/`)

- **`mod.rs`** — Rendering dispatch, `ColorScheme` with two themes (dark cyberpunk, light zen), and shared layout helpers.
- **`dashboard.rs`** — Dashboard view with filters, search, and preview pane.
- **`feed_list.rs`** — Feed list and hierarchical tree view rendering.
- **`feed_items.rs`** — Feed items list rendering.
- **`detail.rs`** — Article detail view with scrolling and link extraction.
- **`starred.rs`** — Starred articles view.
- **`categories.rs`** — Category management UI.
- **`summary.rs`** — Session summary ("What's New") screen.
- **`modals.rs`** — Error, input, filter, link overlay, and help overlay modals.
- **`utils.rs`** — Shared rendering utilities.

### Key patterns

- **View + InputMode dispatch**: Event handling in `events.rs` matches on `(app.view, key.code)` nested inside `app.input_mode`. When adding new keybindings, place them in the correct View/InputMode branch.
- **Configurable keybindings**: All remappable actions are defined as `KeyAction` variants in `keybindings.rs`. The `KeyBindingMap` is built from defaults merged with user overrides from `config.keybindings`. Event handlers use `app.keybindings` to check matches instead of hardcoding key codes. Some structural keys (Tab/Shift+Tab, number keys, text input, category/filter mode keys) are intentionally hardcoded.
- **`q` key goes back, not quit**: `q` navigates back one view (e.g., FeedItems → FeedList → Dashboard). Only quits from Dashboard. `Ctrl+Q` is the universal quit from any view. The `Ctrl+Q` check is a guard at the top of `handle_events`, before the `match app.input_mode` block.
- **Feed auto-discovery**: When a user adds a non-RSS URL, `feed.rs` fetches the HTML and uses `scraper` to find `<link>` tags with RSS/Atom types. If feeds are found, a confirmation dialog lets the user pick which to subscribe to.
- **Mouse support**: `events.rs` handles `MouseEventKind::Down` (left click to select) and `MouseEventKind::ScrollDown`/`ScrollUp` for navigation.
- **Dashboard items**: `dashboard_items: Vec<(feed_idx, item_idx)>` is a derived index into `feeds`, rebuilt by `apply_filters()` whenever filters change.
- **Data persistence**: Saved to `~/.local/share/feedr/feedr_data.json` — bookmarks, categories, and read item tracking.
- **Error display is modal**: When `app.error` is `Some`, the keypress is consumed to dismiss it (not passed through to handlers). See the guard at the top of `handle_events`.
- **Rate limiting**: `last_domain_fetch: HashMap` throttles per-domain HTTP requests.
- **Authenticated feeds**: `feed_headers: HashMap<String, HashMap<String, String>>` in `App` maps feed URLs to custom HTTP headers. Built from `config.default_feeds` entries that have `headers`. Passed to `Feed::fetch_url()` at all fetch call sites.
- **Compact mode**: `app.compact` bool is updated each frame by `update_compact_mode(terminal_height)`. Rendering in `ui.rs` branches on `app.compact` for layout, title bar, help bar, and dashboard item format. Controlled by `config.ui.compact_mode` (`Auto`/`Always`/`Never`). Dialog modals use `centered_rect_with_min()` to enforce minimum dimensions regardless of compact mode.

## Commit Conventions

Uses **conventional commits** — `feat:`, `fix:`, `refactor:`, `docs:`, `perf:`, `test:`, `chore:`, `build:`, `style:`. Changelog is generated by git-cliff (`cliff.toml`).

## Testing

Integration tests live in `/tests/integration_test.rs` and test feed parsing against real URLs. Unit tests are inline in `config.rs`, `app.rs`, and `keybindings.rs`.

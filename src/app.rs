use crate::config::{CompactMode, Config};
use crate::feed::{ExtractedArticle, Feed, FeedCategory, FeedItem};
use crate::ui::ColorScheme;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Debug, Default)]
pub struct FilterOptions {
    pub category: Option<String>,   // Filter by feed category
    pub age: Option<TimeFilter>,    // Filter by content age
    pub has_author: Option<bool>,   // Filter for items with/without author
    pub read_status: Option<bool>,  // Filter for read/unread items
    pub min_length: Option<usize>,  // Filter by content length
    pub starred_only: Option<bool>, // Filter for starred/unstarred items
}

#[derive(Clone, Debug, PartialEq)]
pub enum TimeFilter {
    Today,
    ThisWeek,
    ThisMonth,
    Older,
}

impl FilterOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        self.category.is_some()
            || self.age.is_some()
            || self.has_author.is_some()
            || self.read_status.is_some()
            || self.min_length.is_some()
            || self.starred_only.is_some()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    InsertUrl,
    SearchMode,
    FilterMode,
    CategoryNameInput,    // For creating/renaming categories
    SelectDiscoveredFeed, // For picking from auto-discovered feeds
    ArticleSearch,        // In-article find: live query, n/N to jump, ESC to clear
}

#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Dashboard,
    FeedList,
    FeedItems,
    FeedItemDetail,
    CategoryManagement,
    Summary,
    Starred,
}

#[derive(Debug)]
pub struct App {
    pub config: Config,
    pub feeds: Vec<Feed>,
    pub bookmarks: Vec<String>,
    pub categories: Vec<FeedCategory>,
    pub selected_category: Option<usize>,
    pub input: String,
    pub input_mode: InputMode,
    pub selected_feed: Option<usize>,
    pub selected_item: Option<usize>,
    pub view: View,
    pub error: Option<String>,
    pub success_message: Option<String>,
    pub success_message_time: Option<Instant>,
    pub search_query: String,
    pub is_searching: bool,
    pub filtered_items: Vec<(usize, usize)>, // (feed_idx, item_idx) for search results
    pub dashboard_items: Vec<(usize, usize)>, // (feed_idx, item_idx) for dashboard
    pub is_loading: bool,                    // Flag to indicate loading/refreshing state
    pub loading_indicator: usize,            // For animated loading indicator
    pub filter_options: FilterOptions,
    pub filter_mode: bool,           // Whether we're in filter selection mode
    pub read_items: HashSet<String>, // Track read item IDs
    pub starred_items: HashSet<String>, // Track starred item IDs
    pub filtered_dashboard_items: Vec<(usize, usize)>, // Filtered items for dashboard
    pub category_action: Option<CategoryAction>, // For category management
    pub detail_vertical_scroll: u16, // Vertical scroll value for item detail view
    pub detail_max_scroll: u16,      // Maximum scroll value for current content
    pub last_refresh: Option<Instant>, // Track when last refresh occurred
    pub refresh_in_progress: bool,   // Prevent concurrent refreshes
    pub refresh_requested: bool,     // Signal to main loop to start a non-blocking refresh
    pub last_domain_fetch: HashMap<String, Instant>, // Track last fetch time per domain for rate limiting
    pub color_scheme: ColorScheme, // Cached color scheme to avoid per-frame construction
    pub last_session_time: Option<DateTime<Utc>>, // When the previous session started
    pub show_summary: bool,        // Whether to show summary after feeds load
    pub preview_pane: bool,        // Whether to show article preview pane
    pub preview_scroll: u16,       // Vertical scroll for preview pane
    pub preview_max_scroll: u16,   // Maximum scroll for preview content
    pub feed_headers: HashMap<String, HashMap<String, String>>, // Per-URL custom HTTP headers
    pub compact: bool,             // Whether compact mode is active
    pub discovered_feeds: Vec<crate::feed::DiscoveredFeed>, // Feeds discovered from HTML page
    pub discovered_feed_selection: usize, // Selected index in discovered feeds list
    pub feed_refresh_intervals: HashMap<String, u64>, // url -> per-feed refresh interval in seconds
    pub last_feed_refresh: HashMap<String, Instant>, // url -> last refresh time
    pub show_help_overlay: bool,   // Whether the help overlay is visible
    pub help_overlay_scroll: u16,  // Scroll position in the help overlay
    pub extracted_links: Vec<ExtractedLink>,
    pub show_link_overlay: bool,
    pub selected_link: usize,
    pub feed_tree: Vec<TreeItem>,
    pub selected_tree_item: Option<usize>, // index into feed_tree
    pub keybindings: crate::keybindings::KeyBindingMap,
    pub macros: Vec<crate::keybindings::MacroBinding>,
    pub macro_options: crate::keybindings::MacroOptions,
    /// True after the macro-prefix key has been pressed and we are awaiting the next key.
    pub awaiting_macro_key: bool,
    /// When `awaiting_macro_key` was last set, so the TUI tick loop can time
    /// it out (reusing the 1.5 s success-message timeout) rather than
    /// stranding the prefix state across an idle period.
    pub awaiting_macro_key_since: Option<Instant>,
    /// Item IDs that exec_on_new has already fired for (persisted).
    pub seen_items: HashSet<String>,
    /// Feed URLs whose first successful fetch has been observed (persisted).
    pub feeds_seeded: HashSet<String>,
    /// FIFO queue of macro steps awaiting execution. Filled by the macro
    /// engine in `events.rs`; drained in order by the TUI run loop after
    /// each event-handling pass. Steps include Actions (which mutate app
    /// state) and PipeTo/Exec (which spawn children). Draining happens at
    /// the loop level because PipeTo needs the terminal handle to suspend
    /// the TUI, which `handle_events` does not have access to.
    pub pending_macro_steps: VecDeque<crate::keybindings::MacroStep>,
    /// Pre-tokenized `[hooks].exec_on_new` template. `None` if the hook is
    /// disabled or the template failed to tokenize at startup (in which case
    /// a startup warning is surfaced).
    pub exec_on_new_template: Option<Vec<String>>,
    /// Per-item full-text extraction cache. Keyed by the same item_id scheme
    /// as `read_items` / `starred_items`. In-memory only; not persisted.
    pub extracted: HashMap<String, ExtractionState>,
    /// Insertion order for `extracted`, used to drop the oldest entry when
    /// the cache hits `EXTRACTED_CACHE_CAP`.
    pub extracted_order: VecDeque<String>,
    /// Feed URLs that have `fulltext = true` in config and should trigger
    /// auto-extraction for newly-seen items on each refresh. Same derivation
    /// pattern as `feed_headers` / `feed_refresh_intervals`.
    pub fulltext_feeds: HashSet<String>,
    /// When the focused article has a `Ready` extraction, this flag controls
    /// whether the detail view renders the extracted text or the original
    /// summary. Single global toggle (not per-item) — simplest UX and the
    /// detail view only ever shows one article at a time.
    pub show_extracted: bool,
    /// `(feed_idx, item_idx)` of the article the detail view rendered last
    /// frame. When the focus moves to a different article, `show_extracted`
    /// is reset to `true` so the user doesn't carry over a "view summary"
    /// toggle from one article to another (and end up staring at a summary
    /// while the title bar labels the body `Full-text`). Only the detail
    /// view writes to / consults this — other views resolve focus through
    /// `current_article_indices` directly.
    pub last_detail_focus: Option<(usize, usize)>,
    /// Queue of extraction requests for the TUI loop to spawn worker threads
    /// for. Decouples request-time (event handler) from spawn-time (loop
    /// drain) so events.rs doesn't need the thread-handle / channel.
    pub pending_extraction_requests: VecDeque<ExtractionRequest>,
    /// Monotonic generation counter for extraction requests. Each new
    /// `Pending` slot gets a fresh value so a worker for a previously
    /// evicted-and-recreated slot can't accidentally satisfy the new
    /// request — `record_extraction_result` matches the generation.
    pub next_extraction_generation: u64,
    /// In-article find query. Persists across Enter/Esc transitions until
    /// the user clears it (Esc while in ArticleSearch input mode) or moves
    /// focus to a different article.
    pub article_search_query: String,
    /// Index of the currently-focused match within `article_search_matches`,
    /// or `None` when there are no matches.
    pub article_search_current: Option<usize>,
    /// Matches cached by the detail renderer each frame for use by `n`/`N`
    /// in event handling. Always re-derived from `article_body_cache` +
    /// `article_search_query` at render time, so this list is consistent
    /// with what's drawn on screen.
    pub article_search_matches: Vec<ArticleMatch>,
    /// Cached body text fed to the detail view's `Paragraph`. Written by
    /// the renderer every frame, read by `next_article_match` /
    /// `prev_article_match` to compute wrapped-row scroll targets.
    pub article_body_cache: String,
    /// Cached content width (post-border, post-padding) used to compute
    /// wrapped-row positions when jumping to a match.
    pub article_body_width_cache: usize,
    /// Inline-image cache + Kitty protocol driver. Owned by `App` so the
    /// detail-view renderer can call `start_fetch` from inside ratatui's
    /// draw closure (it has `&mut App`). The actual escape emission lives
    /// in `run_app` after `terminal.draw()` returns — see
    /// `pending_image_render`.
    pub image_cache: crate::image::ImageCache,
    /// Set by the detail-view renderer each frame when an image is loaded
    /// and ready to draw. Consumed once by `run_app` immediately after
    /// `terminal.draw()` returns — the run loop emits Kitty placement
    /// escapes for this rect, then clears the slot. None on every frame
    /// where no image should be shown (no thumbnail, not on detail view,
    /// fetch in-flight, fetch failed).
    pub pending_image_render: Option<PendingImage>,
    /// True iff the previous frame *was* showing an inline image. Used by
    /// `run_app` to detect view-change → emit Kitty delete-all so the
    /// image doesn't bleed onto the next view's cells.
    pub last_frame_had_image: bool,
    /// When true in the detail view, hides the title bar, article header,
    /// image strip, and help bar so the article content occupies the full screen.
    pub read_mode: bool,
}

/// Detail-view → run_app message: "after the next `terminal.draw()` returns,
/// place image `url` inside the rect `(col, row, cols, rows)`." Coordinates
/// are 0-based terminal cells (matching ratatui's `Rect` semantics).
#[derive(Clone, Debug)]
pub struct PendingImage {
    pub url: String,
    pub col: u16,
    pub row: u16,
    pub cols: u16,
    pub rows: u16,
}

/// A single in-article search match, indexed against the rendered body's
/// `'\n'`-separated logical lines (the same split the renderer uses to
/// build the `Vec<Line>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArticleMatch {
    /// 0-based index into `body.split('\n')`.
    pub line: usize,
    /// Byte offset of the match start within that line.
    pub start: usize,
    /// Byte offset of the match end (exclusive) within that line.
    pub end: usize,
}

/// Snapshot of the focused article passed to template-expansion / pipe-payload helpers.
#[derive(Clone, Debug)]
pub struct ArticleContext<'a> {
    pub title: &'a str,
    pub url: Option<&'a str>,
    pub author: Option<&'a str>,
    pub formatted_date: Option<&'a str>,
    pub feed_title: &'a str,
    pub feed_url: &'a str,
    pub plain_text: Option<&'a str>,
    /// Primary media URL — picked by `FeedItem::primary_media`. None if the
    /// item has no media attachments. Exposed to macros as `%m`.
    pub media_url: Option<&'a str>,
    /// MIME type of the primary media attachment, if known. `%M`.
    pub media_mime: Option<&'a str>,
    /// First `<media:thumbnail>` URI on the item, if any. `%i`.
    pub thumbnail_url: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub enum LinkType {
    Link,
    Image,
}

#[derive(Clone, Debug)]
pub struct ExtractedLink {
    pub url: String,
    pub text: String,
    pub link_type: LinkType,
}

#[derive(Clone, Debug)]
pub enum TreeItem {
    Category(usize),            // index into self.categories
    Feed(usize, Option<usize>), // feed index, optional parent category index
}

#[derive(Clone, Debug)]
pub enum CategoryAction {
    Create,
    Rename(usize),
    AddFeedToCategory(String), // Feed URL to add
}

pub enum AddFeedResult {
    Added,
    DiscoveredFeeds {
        feeds: Vec<crate::feed::DiscoveredFeed>,
        page_url: String,
    },
}

/// State of a per-item full-text extraction. Lives only in-memory: a
/// restart re-fetches on demand or via the next refresh for fulltext-flagged
/// feeds. No on-disk persistence by design — saves us from cache-invalidation
/// logic and bounds storage growth.
#[derive(Clone, Debug)]
pub enum ExtractionState {
    /// In-flight. `generation` matches the generation stamped on the
    /// `ExtractionRequest` that produced this slot; `started_at` drives the
    /// stale-Pending watchdog. The generation closes a TOCTOU window where
    /// a Pending slot is evicted from the LRU and re-requested for the same
    /// id — without the tag, the old worker's eventual result would land on
    /// the new slot.
    ///
    /// `spawned` flips to true only after the spawn loop has both bumped
    /// the inflight counter AND successfully called `thread::Builder::spawn`.
    /// A request that ages out while still queued (rate-limited behind a
    /// backlog longer than the watchdog window) is `spawned == false`, and
    /// the watchdog MUST NOT release an inflight slot for it — there was no
    /// permit to release, and a spurious release would shrink the effective
    /// pool below `EXTRACTION_MAX_INFLIGHT`.
    Pending {
        generation: u64,
        started_at: Instant,
        spawned: bool,
    },
    Ready(ExtractedArticle),
    Failed(String),
}

/// A queued request for the TUI loop to spawn a worker thread for. Filled
/// in `request_extraction_for_current` and `enqueue_auto_extractions`,
/// drained in `tui.rs` each tick (same shape as `pending_macro_steps`).
#[derive(Clone, Debug)]
pub struct ExtractionRequest {
    pub id: String,
    pub url: String,
    /// When true, the worker must use a client whose redirect policy re-runs
    /// `is_safe_auto_url` on every hop. Set for the auto-fulltext path
    /// (URLs come from feed XML, untrusted) and cleared for manual `Shift+F`
    /// (user already chose the article — same trust model as opening it in
    /// a browser).
    pub safe_redirects: bool,
    /// Generation tag matching the `Pending` slot this request was created
    /// for. Worker echoes it back through the result channel so the receiver
    /// can drop late results from stale (evicted-and-recreated) slots.
    pub generation: u64,
}

/// Cap on the number of cached extracted articles. Auto-fetch over a big
/// feed could otherwise accumulate state monotonically across a long
/// session. Insertion-order LRU eviction (oldest cache entries are dropped
/// first) — `extracted_order` tracks the insertion sequence.
///
/// Soft cap, not hard: `insert_extraction` refuses to evict
/// `Pending { spawned: true }` slots — evicting one would hide it from the
/// stale-Pending watchdog (which only looks at `extracted`) and leak the
/// worker's inflight permit if the worker is dead. The number of such slots
/// at any moment is bounded by the worker-pool size (see
/// `EXTRACTION_MAX_INFLIGHT` in `tui.rs`), so the effective cap grows by at
/// most that constant.
pub const EXTRACTED_CACHE_CAP: usize = 500;

/// Stale-`Pending` watchdog timeout, as a multiple of `http_timeout`. A worker
/// whose Pending slot is older than `http_timeout * PENDING_WATCHDOG_MULTIPLIER`
/// is assumed dead (OS kill / SIGSEGV in a native dep) and flipped to
/// `Failed("timed out")` so the user can retry. Multiplier covers slow DNS,
/// TLS handshakes, and Readability CPU time on top of the network timeout.
pub const PENDING_WATCHDOG_MULTIPLIER: u32 = 3;

#[derive(Serialize, Deserialize)]
struct SavedData {
    bookmarks: Vec<String>,
    categories: Vec<FeedCategory>,
    read_items: HashSet<String>,
    #[serde(default)]
    starred_items: HashSet<String>,
    #[serde(default)]
    last_session_time: Option<String>,
    /// Item IDs (same scheme as `read_items`) that exec_on_new has already fired for.
    #[serde(default)]
    seen_items: HashSet<String>,
    /// Feed URLs whose first successful fetch has been observed. Items from a
    /// feed not in this set are seeded into `seen_items` silently.
    #[serde(default)]
    feeds_seeded: HashSet<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        // Load configuration
        let config = Config::load().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to load config, using defaults: {}", e);
            Config::default()
        });

        let saved_data = Self::load_saved_data().unwrap_or_else(|_| SavedData {
            bookmarks: vec![],
            categories: vec![],
            read_items: HashSet::new(),
            starred_items: HashSet::new(),
            last_session_time: None,
            seen_items: HashSet::new(),
            feeds_seeded: HashSet::new(),
        });

        // Seed bookmarks from default_feeds if no saved bookmarks exist
        let mut bookmarks = saved_data.bookmarks;
        if bookmarks.is_empty() && !config.default_feeds.is_empty() {
            bookmarks = config.default_feeds.iter().map(|f| f.url.clone()).collect();
        }

        let has_bookmarks = !bookmarks.is_empty();
        let color_scheme = ColorScheme::from_theme(&config.ui.theme);

        // Build per-URL headers lookup from config
        let feed_headers: HashMap<String, HashMap<String, String>> = config
            .default_feeds
            .iter()
            .filter_map(|f| f.headers.as_ref().map(|h| (f.url.clone(), h.clone())))
            .collect();

        // Build per-feed refresh intervals from config
        let feed_refresh_intervals: HashMap<String, u64> = config
            .default_feeds
            .iter()
            .filter_map(|f| f.refresh_interval.map(|interval| (f.url.clone(), interval)))
            .collect();

        // Build the set of URLs that opted into full-text auto-extraction
        let fulltext_feeds: HashSet<String> = config
            .default_feeds
            .iter()
            .filter(|f| f.fulltext.unwrap_or(false))
            .map(|f| f.url.clone())
            .collect();

        // Parse last session time from saved data
        let last_session_time = saved_data
            .last_session_time
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let show_summary = last_session_time.is_some() && has_bookmarks;

        let (keybindings, kb_warnings) = crate::keybindings::build_keybindings(&config.keybindings);
        let (macro_options, mo_warnings) = crate::keybindings::resolve_macro_options(
            &config.macro_options.prefix,
            &config.macro_options.pipe_default_stdin,
        );
        let (macros, mac_warnings) =
            crate::keybindings::build_macros(&config.macros, &macro_options);

        // Pre-tokenize the exec_on_new template once at startup so we don't
        // re-shlex on every feed arrival, and surface tokenization errors as a
        // startup warning instead of an unexpected runtime failure later.
        let mut hook_warnings: Vec<String> = Vec::new();
        let exec_on_new_template = config
            .hooks
            .exec_on_new
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|t| match shlex::split(t) {
                Some(toks) if !toks.is_empty() => Some(toks),
                _ => {
                    hook_warnings.push(format!("exec_on_new: could not tokenize: {}", t));
                    None
                }
            });

        let show_preview = config.ui.show_preview;

        let mut app = Self {
            config,
            feeds: Vec::new(),
            bookmarks,
            categories: saved_data.categories,
            selected_category: None,
            input: String::new(),
            input_mode: InputMode::Normal,
            selected_feed: None,
            selected_item: None,
            view: View::Dashboard,
            error: None,
            success_message: None,
            success_message_time: None,
            search_query: String::new(),
            is_searching: false,
            filtered_items: Vec::new(),
            dashboard_items: Vec::new(),
            is_loading: has_bookmarks,
            loading_indicator: 0,
            filter_options: FilterOptions::new(),
            filter_mode: false,
            read_items: saved_data.read_items,
            starred_items: saved_data.starred_items,
            filtered_dashboard_items: Vec::new(),
            category_action: None,
            detail_vertical_scroll: 0,
            detail_max_scroll: 0,
            last_refresh: None,
            refresh_in_progress: false,
            refresh_requested: false,
            last_domain_fetch: HashMap::new(),
            color_scheme,
            last_session_time,
            show_summary,
            preview_pane: show_preview,
            preview_scroll: 0,
            preview_max_scroll: 0,
            feed_headers,
            compact: false,
            discovered_feeds: Vec::new(),
            discovered_feed_selection: 0,
            feed_refresh_intervals,
            last_feed_refresh: HashMap::new(),
            show_help_overlay: false,
            help_overlay_scroll: 0,
            extracted_links: Vec::new(),
            show_link_overlay: false,
            selected_link: 0,
            feed_tree: Vec::new(),
            selected_tree_item: None,
            keybindings,
            macros,
            macro_options,
            awaiting_macro_key: false,
            awaiting_macro_key_since: None,
            seen_items: saved_data.seen_items,
            feeds_seeded: saved_data.feeds_seeded,
            pending_macro_steps: VecDeque::new(),
            exec_on_new_template,
            extracted: HashMap::new(),
            extracted_order: VecDeque::new(),
            fulltext_feeds,
            show_extracted: true,
            last_detail_focus: None,
            pending_extraction_requests: VecDeque::new(),
            next_extraction_generation: 0,
            article_search_query: String::new(),
            article_search_current: None,
            article_search_matches: Vec::new(),
            article_body_cache: String::new(),
            article_body_width_cache: 0,
            image_cache: crate::image::ImageCache::new(),
            pending_image_render: None,
            last_frame_had_image: false,
            read_mode: false,
        };

        app.update_dashboard();
        app.rebuild_feed_tree();

        let mut all_warnings = kb_warnings;
        all_warnings.extend(mo_warnings);
        all_warnings.extend(mac_warnings);
        all_warnings.extend(hook_warnings);
        if !all_warnings.is_empty() {
            app.error = Some(format!("Config: {}", all_warnings.join("; ")));
        }

        app
    }

    pub fn key_matches(
        &self,
        action: crate::keybindings::KeyAction,
        key: &crossterm::event::KeyEvent,
    ) -> bool {
        if let Some(bindings) = self.keybindings.get(&action) {
            bindings.iter().any(|b| b.matches(key))
        } else {
            false
        }
    }

    pub fn update_compact_mode(&mut self, terminal_height: u16) {
        self.compact = match self.config.ui.compact_mode {
            CompactMode::Always => true,
            CompactMode::Never => false,
            CompactMode::Auto => terminal_height <= 30,
        };
    }

    pub fn load_bookmarked_feeds(&mut self) {
        self.feeds.clear();
        if self.bookmarks.is_empty() {
            return;
        }

        let timeout = self.config.network.http_timeout;
        let user_agent = self.config.network.user_agent.clone();

        let client = match Feed::build_client(timeout) {
            Ok(c) => c,
            Err(_) => return,
        };

        // Spawn a thread per bookmark URL for parallel fetching
        let handles: Vec<_> = self
            .bookmarks
            .iter()
            .map(|url| {
                let client = client.clone();
                let url = url.clone();
                let ua = user_agent.clone();
                let hdrs = self.feed_headers.get(&url).cloned();
                std::thread::spawn(move || {
                    Feed::fetch_url(&url, &client, Some(&ua), hdrs.as_ref())
                        .and_then(|r| r.into_feed())
                })
            })
            .collect();

        // Collect results in original bookmark order
        for handle in handles {
            if let Ok(Ok(feed)) = handle.join() {
                self.feeds.push(feed);
            }
        }
    }

    fn load_saved_data() -> Result<SavedData> {
        let path = Self::data_path();
        if !path.exists() {
            return Ok(SavedData {
                bookmarks: Vec::new(),
                categories: Vec::new(),
                read_items: HashSet::new(),
                starred_items: HashSet::new(),
                last_session_time: None,
                seen_items: HashSet::new(),
                feeds_seeded: HashSet::new(),
            });
        }

        let data = fs::read_to_string(path)?;
        let saved_data: SavedData = serde_json::from_str(&data)?;
        Ok(saved_data)
    }

    pub fn save_data(&self) -> Result<()> {
        let path = Self::data_path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        let saved_data = SavedData {
            bookmarks: self.bookmarks.clone(),
            categories: self.categories.clone(),
            read_items: self.read_items.clone(),
            starred_items: self.starred_items.clone(),
            last_session_time: Some(Utc::now().to_rfc3339()),
            seen_items: self.seen_items.clone(),
            feeds_seeded: self.feeds_seeded.clone(),
        };

        let json = serde_json::to_string(&saved_data)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Get the data file path with XDG support and backwards compatibility
    fn data_path() -> PathBuf {
        // New XDG-compliant location
        let xdg_path = Self::xdg_data_path();

        // Old location for backwards compatibility
        let old_path = Self::legacy_data_path();

        // If old location exists and new doesn't, migrate
        if old_path.exists() && !xdg_path.exists() {
            if let Err(e) = Self::migrate_data_file(&old_path, &xdg_path) {
                eprintln!("Warning: Failed to migrate data file: {}", e);
                // Fall back to old path if migration fails
                return old_path;
            }
            eprintln!(
                "Data file migrated from {} to {}",
                old_path.display(),
                xdg_path.display()
            );
        }

        xdg_path
    }

    /// Get the XDG-compliant data path (~/.local/share/feedr/feedr_data.json)
    fn xdg_data_path() -> PathBuf {
        let mut path = dirs::data_local_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
        path.push("feedr");
        path.push("feedr_data.json");
        path
    }

    /// Get the legacy data path for backwards compatibility
    fn legacy_data_path() -> PathBuf {
        let mut path = dirs::data_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
        path.push("feedr");
        path.push("feedr_data.json");
        path
    }

    /// Migrate data from old location to new XDG location
    fn migrate_data_file(old_path: &Path, new_path: &Path) -> Result<()> {
        // Ensure the target directory exists
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Copy the file to the new location
        fs::copy(old_path, new_path)?;

        // Optionally, remove the old file after successful migration
        // Commenting this out for extra safety - users can manually delete if desired
        // fs::remove_file(old_path)?;

        Ok(())
    }

    pub fn apply_filters(&mut self) {
        // First update the dashboard items normally
        if !self.filter_options.is_active() {
            // No filters active, so filtered items are the same as dashboard items
            // Use clone_from to reuse existing allocation
            self.filtered_dashboard_items
                .clone_from(&self.dashboard_items);
        } else {
            self.filtered_dashboard_items = self
                .dashboard_items
                .iter()
                .filter(|&&(feed_idx, item_idx)| self.item_matches_filter(feed_idx, item_idx))
                .cloned()
                .collect();
        }

        self.clamp_dashboard_selection();
    }

    /// Returns the item list currently visible on the dashboard,
    /// accounting for search mode and active filters.
    pub fn active_dashboard_items(&self) -> &[(usize, usize)] {
        if self.is_searching {
            &self.filtered_items
        } else if self.filter_options.is_active() {
            &self.filtered_dashboard_items
        } else {
            &self.dashboard_items
        }
    }

    /// Returns the `(Feed, FeedItem)` at the given position in the active (possibly filtered) list.
    pub fn active_dashboard_item(&self, idx: usize) -> Option<(&Feed, &FeedItem)> {
        let items = self.active_dashboard_items();
        if idx < items.len() {
            let (feed_idx, item_idx) = items[idx];
            if let Some(feed) = self.feeds.get(feed_idx) {
                if let Some(item) = feed.items.get(item_idx) {
                    return Some((feed, item));
                }
            }
        }
        None
    }

    /// Clamp `selected_item` to stay within the active dashboard item list bounds.
    pub fn clamp_dashboard_selection(&mut self) {
        let len = self.active_dashboard_items().len();
        if len == 0 {
            self.selected_item = None;
        } else if let Some(sel) = self.selected_item {
            if sel >= len {
                self.selected_item = Some(len - 1);
            }
        }
    }

    fn item_matches_filter(&self, feed_idx: usize, item_idx: usize) -> bool {
        let feed = match self.feeds.get(feed_idx) {
            Some(f) => f,
            None => return false,
        };

        let item = match feed.items.get(item_idx) {
            Some(i) => i,
            None => return false,
        };

        // Check category filter
        if let Some(category_name) = &self.filter_options.category {
            let feed_in_category = self
                .categories
                .iter()
                .any(|c| c.name == *category_name && c.contains_feed(&feed.url));
            if !feed_in_category {
                return false;
            }
        }

        // Check age filter using cached parsed_date (avoids re-parsing RFC3339 strings)
        if let Some(age_filter) = &self.filter_options.age {
            if let Some(date) = &item.parsed_date {
                let now = chrono::Utc::now();
                let duration = now.signed_duration_since(*date);

                match age_filter {
                    TimeFilter::Today => {
                        if duration.num_hours() > 24 {
                            return false;
                        }
                    }
                    TimeFilter::ThisWeek => {
                        if duration.num_days() > 7 {
                            return false;
                        }
                    }
                    TimeFilter::ThisMonth => {
                        if duration.num_days() > 30 {
                            return false;
                        }
                    }
                    TimeFilter::Older => {
                        if duration.num_days() <= 30 {
                            return false;
                        }
                    }
                }
            } else {
                // No date, so filter out if age filter is active
                return false;
            }
        }

        // Check author filter
        if let Some(has_author) = self.filter_options.has_author {
            let item_has_author =
                item.author.is_some() && !item.author.as_ref().unwrap().is_empty();
            if has_author != item_has_author {
                return false;
            }
        }

        // Check read status filter
        if let Some(is_read) = self.filter_options.read_status {
            let item_id = self.get_item_id(feed_idx, item_idx);
            let item_is_read = self.read_items.contains(&item_id);
            if is_read != item_is_read {
                return false;
            }
        }

        // Check starred status filter
        if let Some(is_starred) = self.filter_options.starred_only {
            let item_id = self.get_item_id(feed_idx, item_idx);
            let item_is_starred = self.starred_items.contains(&item_id);
            if is_starred != item_is_starred {
                return false;
            }
        }

        // Check content length filter using cached plain_text (avoids HTML parsing)
        if let Some(min_length) = self.filter_options.min_length {
            if let Some(plain_text) = &item.plain_text {
                if plain_text.len() < min_length {
                    return false;
                }
            } else {
                // No description, so it doesn't meet length requirement
                return false;
            }
        }

        true
    }

    // Generate a unique ID for an item to track read status
    pub(crate) fn get_item_id(&self, feed_idx: usize, item_idx: usize) -> String {
        if let Some(feed) = self.feeds.get(feed_idx) {
            if let Some(item) = feed.items.get(item_idx) {
                return make_item_id(feed, item);
            }
        }
        String::new()
    }

    // Mark an item as read
    pub fn mark_item_as_read(&mut self, feed_idx: usize, item_idx: usize) -> Result<()> {
        let item_id = self.get_item_id(feed_idx, item_idx);
        if !item_id.is_empty() && self.read_items.insert(item_id) {
            self.save_data()?;
        }
        Ok(())
    }

    // Toggle an item's read status and return whether it's now read
    pub fn toggle_item_read(&mut self, feed_idx: usize, item_idx: usize) -> Result<bool> {
        let item_id = self.get_item_id(feed_idx, item_idx);
        if !item_id.is_empty() {
            let is_now_read = if self.read_items.contains(&item_id) {
                // Item is read, mark as unread
                self.read_items.remove(&item_id);
                false
            } else {
                // Item is unread, mark as read
                self.read_items.insert(item_id);
                true
            };
            self.save_data()?;
            Ok(is_now_read)
        } else {
            Ok(false)
        }
    }

    // Check if an item is read
    pub fn is_item_read(&self, feed_idx: usize, item_idx: usize) -> bool {
        let item_id = self.get_item_id(feed_idx, item_idx);
        self.read_items.contains(&item_id)
    }

    /// Mark all currently visible dashboard items as read, returns count marked.
    pub fn mark_all_dashboard_read(&mut self) -> Result<usize> {
        let items: Vec<(usize, usize)> = self.active_dashboard_items().to_vec();
        let mut count = 0;
        for (feed_idx, item_idx) in &items {
            let item_id = self.get_item_id(*feed_idx, *item_idx);
            if !item_id.is_empty() && self.read_items.insert(item_id) {
                count += 1;
            }
        }
        if count > 0 {
            self.save_data()?;
        }
        Ok(count)
    }

    /// Mark all currently starred items as read, returns count marked.
    pub fn mark_all_starred_read(&mut self) -> Result<usize> {
        let starred = self.get_starred_dashboard_items();
        let mut count = 0;
        for (feed_idx, item_idx) in &starred {
            let item_id = self.get_item_id(*feed_idx, *item_idx);
            if !item_id.is_empty() && self.read_items.insert(item_id) {
                count += 1;
            }
        }
        if count > 0 {
            self.save_data()?;
        }
        Ok(count)
    }

    /// Mark all items in a specific feed as read, returns count marked.
    pub fn mark_all_feed_read(&mut self, feed_idx: usize) -> Result<usize> {
        let mut count = 0;
        if let Some(feed) = self.feeds.get(feed_idx) {
            for item_idx in 0..feed.items.len() {
                let item_id = self.get_item_id(feed_idx, item_idx);
                if !item_id.is_empty() && self.read_items.insert(item_id) {
                    count += 1;
                }
            }
        }
        if count > 0 {
            self.save_data()?;
        }
        Ok(count)
    }

    // Toggle an item's starred status and return whether it's now starred
    pub fn toggle_item_starred(&mut self, feed_idx: usize, item_idx: usize) -> Result<bool> {
        let item_id = self.get_item_id(feed_idx, item_idx);
        if !item_id.is_empty() {
            let is_now_starred = if self.starred_items.contains(&item_id) {
                self.starred_items.remove(&item_id);
                false
            } else {
                self.starred_items.insert(item_id);
                true
            };
            self.save_data()?;
            Ok(is_now_starred)
        } else {
            Ok(false)
        }
    }

    // Check if an item is starred
    pub fn is_item_starred(&self, feed_idx: usize, item_idx: usize) -> bool {
        let item_id = self.get_item_id(feed_idx, item_idx);
        self.starred_items.contains(&item_id)
    }

    // Get starred items from dashboard_items for the Starred view
    pub fn get_starred_dashboard_items(&self) -> Vec<(usize, usize)> {
        self.dashboard_items
            .iter()
            .filter(|&&(feed_idx, item_idx)| self.is_item_starred(feed_idx, item_idx))
            .cloned()
            .collect()
    }

    pub fn update_dashboard(&mut self) {
        // Clear existing dashboard items
        self.dashboard_items.clear();

        // Get all feeds and sort by most recent first
        let mut all_items = Vec::new();

        for (feed_idx, feed) in self.feeds.iter().enumerate() {
            for (item_idx, item) in feed.items.iter().enumerate() {
                all_items.push((feed_idx, item_idx, item.parsed_date));
            }
        }

        // Sort by date (most recent first)
        all_items.sort_by(|a, b| match (&a.2, &b.2) {
            (Some(a_date), Some(b_date)) => b_date.cmp(a_date),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        // Add items to dashboard (limited by config)
        let max_items = self.config.general.max_dashboard_items;
        for (feed_idx, item_idx, _) in all_items.into_iter().take(max_items) {
            self.dashboard_items.push((feed_idx, item_idx));
        }

        // Apply any active filters
        self.apply_filters();
    }

    pub fn add_feed(&mut self, url: &str) -> Result<AddFeedResult> {
        let timeout = self.config.network.http_timeout;
        let user_agent = &self.config.network.user_agent;
        let headers = self.feed_headers.get(url);
        let client = Feed::build_client(timeout)?;
        let result = Feed::fetch_url(url, &client, Some(user_agent), headers)?;

        match result {
            crate::feed::FeedFetchResult::Feed(feed) => {
                self.feeds.push(feed);
                if !self.bookmarks.contains(&url.to_string()) {
                    self.bookmarks.push(url.to_string());
                }
                self.update_dashboard();
                self.rebuild_feed_tree();
                self.save_data()?;
                Ok(AddFeedResult::Added)
            }
            crate::feed::FeedFetchResult::DiscoveredFeeds { feeds, page_url } => {
                Ok(AddFeedResult::DiscoveredFeeds { feeds, page_url })
            }
        }
    }

    fn opml_dfs(outline: &opml::Outline) -> Vec<String> {
        let mut urls = Vec::<String>::new();
        if let Some(url) = &outline.xml_url {
            urls.push(url.to_string())
        }
        for o in &outline.outlines {
            urls.append(&mut Self::opml_dfs(o));
        }
        urls
    }

    pub fn import_opml(&mut self, file_path: &str) -> Result<()> {
        let mut opml_file = match std::fs::File::open(file_path) {
            Ok(f) => f,
            Err(e) => return Err(anyhow::anyhow!("Opening file {}. {}", file_path, e)),
        };
        let opml_data = match opml::OPML::from_reader(&mut opml_file) {
            Ok(opml) => opml,
            Err(e) => return Err(anyhow::anyhow!("OPML decode error. {}", e)),
        };
        for feed_list in opml_data.body.outlines {
            for feed in Self::opml_dfs(&feed_list) {
                match self.add_feed(&feed) {
                    Ok(AddFeedResult::Added) => println!("Feed {} added", feed),
                    Ok(AddFeedResult::DiscoveredFeeds { feeds, .. }) => {
                        eprintln!(
                            "Skipping {}: HTML page ({} feed links found, use TUI to select)",
                            feed,
                            feeds.len()
                        );
                    }
                    Err(e) => eprintln!("Error adding {}: {}", feed, e),
                }
            }
        }
        Ok(())
    }

    pub fn remove_current_feed(&mut self) -> Result<()> {
        if let Some(idx) = self.selected_feed {
            if idx < self.feeds.len() {
                let url = self.feeds[idx].url.clone();

                // Drop this feed's exec_on_new tracking state before removing
                // the feed itself — otherwise the URL would stay in
                // feeds_seeded and its item IDs would stay in seen_items
                // forever, growing the persisted JSON file monotonically.
                self.feeds_seeded.remove(&url);
                let mut removed_ids: Vec<String> = Vec::new();
                for item in &self.feeds[idx].items {
                    let id = make_item_id(&self.feeds[idx], item);
                    self.seen_items.remove(&id);
                    removed_ids.push(id);
                }
                // Drop in-memory extracted full-text for the removed items
                // too — keeping them would never expire (no persistence)
                // but they're keyed on item ids that no longer resolve.
                for id in &removed_ids {
                    self.remove_extraction(id);
                }
                self.fulltext_feeds.remove(&url);

                // Remove from feeds
                self.feeds.remove(idx);

                // Remove from bookmarks
                if let Some(pos) = self.bookmarks.iter().position(|x| x == &url) {
                    self.bookmarks.remove(pos);
                }

                // Remove from all categories
                for category in &mut self.categories {
                    category.remove_feed(&url);
                }

                // Update selected feed
                if !self.feeds.is_empty() {
                    if idx >= self.feeds.len() {
                        self.selected_feed = Some(self.feeds.len() - 1);
                    }
                } else {
                    self.selected_feed = None;
                    self.view = View::Dashboard;
                }

                // Update dashboard
                self.update_dashboard();
                self.rebuild_feed_tree();

                // Save changes
                self.save_data()?;
            }
        }

        Ok(())
    }

    pub fn current_feed(&self) -> Option<&Feed> {
        self.selected_feed.and_then(|idx| self.feeds.get(idx))
    }

    pub fn current_item(&self) -> Option<&FeedItem> {
        self.current_feed()
            .and_then(|feed| self.selected_item.and_then(|idx| feed.items.get(idx)))
    }

    pub fn open_current_item_in_browser(&self) -> Result<()> {
        if let Some(item) = self.current_item() {
            if let Some(link) = &item.link {
                open::that(link)?;
            }
        }
        Ok(())
    }

    pub fn live_search(&mut self, query: &str) {
        self.search_feeds(query);
        self.view = View::Dashboard;
        let count = self.filtered_items.len();
        match self.selected_item {
            Some(sel) if count > 0 && sel >= count => self.selected_item = Some(count - 1),
            None if count > 0 => self.selected_item = Some(0),
            _ => {}
        }
    }

    pub fn search_feeds(&mut self, query: &str) {
        self.search_query = query.to_lowercase();
        self.is_searching = !query.is_empty();

        if !self.is_searching {
            return;
        }

        self.filtered_items.clear();
        for (feed_idx, feed) in self.feeds.iter().enumerate() {
            if feed.title_lower.contains(&self.search_query) {
                // Add all items from matching feed
                for item_idx in 0..feed.items.len() {
                    self.filtered_items.push((feed_idx, item_idx));
                }
            } else {
                // Check individual items using pre-lowercased title and cached plain_text
                for (item_idx, item) in feed.items.iter().enumerate() {
                    if item.title_lower.contains(&self.search_query)
                        || item
                            .plain_text
                            .as_ref()
                            .is_some_and(|pt| pt.to_lowercase().contains(&self.search_query))
                    {
                        self.filtered_items.push((feed_idx, item_idx));
                    }
                }
            }
        }
    }

    pub fn refresh_feeds(&mut self) -> Result<()> {
        self.is_loading = true;
        self.refresh_in_progress = true;

        // Group feeds by domain for rate limiting
        let mut domain_groups: HashMap<String, Vec<String>> = HashMap::new();
        for url in &self.bookmarks {
            let domain = Self::extract_domain_from_url(url);
            domain_groups.entry(domain).or_default().push(url.clone());
        }

        let timeout = self.config.network.http_timeout;
        let user_agent = self.config.network.user_agent.clone();
        let rate_limit_delay = self.config.general.refresh_rate_limit_delay;

        // Pre-calculate per-domain delays before spawning threads
        let domain_delays: HashMap<String, std::time::Duration> = domain_groups
            .keys()
            .map(|domain| (domain.clone(), self.calculate_required_delay(domain)))
            .collect();

        self.feeds.clear();

        let client = match Feed::build_client(timeout) {
            Ok(c) => c,
            Err(e) => {
                self.is_loading = false;
                self.refresh_in_progress = false;
                return Err(e);
            }
        };

        // Clone headers map for use in threads
        let all_headers = self.feed_headers.clone();

        // Spawn one thread per domain group — domains fetch in parallel,
        // feeds within the same domain fetch sequentially (rate limiting)
        let handles: Vec<_> = domain_groups
            .into_iter()
            .map(|(domain, urls)| {
                let client = client.clone();
                let ua = user_agent.clone();
                let delay = domain_delays.get(&domain).copied().unwrap_or_default();
                let rate_limit = std::time::Duration::from_millis(rate_limit_delay);
                let hdrs = all_headers.clone();

                std::thread::spawn(move || {
                    if !delay.is_zero() {
                        std::thread::sleep(delay);
                    }

                    let mut results = Vec::new();
                    for (i, url) in urls.iter().enumerate() {
                        // Rate-limit between feeds on the same domain
                        if i > 0 && !rate_limit.is_zero() {
                            std::thread::sleep(rate_limit);
                        }
                        results.push((
                            url.clone(),
                            Feed::fetch_url(url, &client, Some(&ua), hdrs.get(url))
                                .and_then(|r| r.into_feed()),
                        ));
                    }
                    (domain, results)
                })
            })
            .collect();

        // Collect results and update domain fetch times
        let mut errors = Vec::new();
        for handle in handles {
            if let Ok((domain, results)) = handle.join() {
                for (url, result) in results {
                    match result {
                        Ok(feed) => self.feeds.push(feed),
                        Err(e) => {
                            errors.push(format!("Failed to refresh feed {}: {}", url, e));
                        }
                    }
                }
                self.last_domain_fetch.insert(domain, Instant::now());
            }
        }

        if let Some(last_error) = errors.last() {
            self.error = Some(last_error.clone());
        }

        self.update_dashboard();
        self.is_loading = false;
        self.refresh_in_progress = false;
        self.last_refresh = Some(Instant::now());

        Ok(())
    }

    pub fn update_loading_indicator(&mut self) {
        self.loading_indicator = (self.loading_indicator + 1) % 10;
    }

    pub fn get_available_categories(&self) -> Vec<String> {
        let mut result: Vec<String> = self.categories.iter().map(|c| c.name.clone()).collect();
        result.sort();
        result
    }

    pub fn get_filter_stats(&self) -> (usize, usize, usize) {
        let active_count = [
            self.filter_options.category.is_some(),
            self.filter_options.age.is_some(),
            self.filter_options.has_author.is_some(),
            self.filter_options.read_status.is_some(),
            self.filter_options.min_length.is_some(),
            self.filter_options.starred_only.is_some(),
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        let filtered_count = if self.is_searching {
            self.filtered_items.len()
        } else {
            self.filtered_dashboard_items.len()
        };

        let total_count = if self.is_searching {
            self.filtered_items.len()
        } else {
            self.dashboard_items.len()
        };

        (active_count, filtered_count, total_count)
    }

    pub fn get_filter_summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(category) = &self.filter_options.category {
            parts.push(format!("Category: {}", category));
        }

        if let Some(age) = &self.filter_options.age {
            let age_str = match age {
                TimeFilter::Today => "Today",
                TimeFilter::ThisWeek => "This Week",
                TimeFilter::ThisMonth => "This Month",
                TimeFilter::Older => "Older than a month",
            };
            parts.push(format!("Age: {}", age_str));
        }

        if let Some(has_author) = self.filter_options.has_author {
            parts.push(format!(
                "Author: {}",
                if has_author {
                    "With author"
                } else {
                    "No author"
                }
            ));
        }

        if let Some(is_read) = self.filter_options.read_status {
            parts.push(format!(
                "Status: {}",
                if is_read { "Read" } else { "Unread" }
            ));
        }

        if let Some(length) = self.filter_options.min_length {
            let length_str = match length {
                100 => "Short",
                500 => "Medium",
                1000 => "Long",
                _ => "Custom",
            };
            parts.push(format!("Length: {}", length_str));
        }

        if let Some(is_starred) = self.filter_options.starred_only {
            parts.push(format!(
                "Starred: {}",
                if is_starred { "Yes" } else { "No" }
            ));
        }

        if parts.is_empty() {
            "No filters active".to_string()
        } else {
            parts.join(" | ")
        }
    }

    // Category management functions
    pub fn create_category(&mut self, name: &str) -> Result<()> {
        // Trim the name and check if it's empty
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow::anyhow!("Category name cannot be empty"));
        }

        // Check if category with same name exists
        if self.categories.iter().any(|c| c.name == name) {
            return Err(anyhow::anyhow!("Category with this name already exists"));
        }

        // Create and add the new category
        let category = FeedCategory::new(name);
        self.categories.push(category);
        self.selected_category = Some(self.categories.len() - 1);

        // Save categories
        self.save_data()?;
        self.rebuild_feed_tree();

        Ok(())
    }

    pub fn delete_category(&mut self, idx: usize) -> Result<()> {
        if idx >= self.categories.len() {
            return Err(anyhow::anyhow!("Invalid category index"));
        }

        self.categories.remove(idx);
        if !self.categories.is_empty() && self.selected_category.is_some() {
            if self.selected_category.unwrap() >= self.categories.len() {
                self.selected_category = Some(self.categories.len() - 1);
            }
        } else {
            self.selected_category = None;
        }

        // Save categories
        self.save_data()?;
        self.rebuild_feed_tree();

        Ok(())
    }

    pub fn rename_category(&mut self, idx: usize, new_name: &str) -> Result<()> {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err(anyhow::anyhow!("Category name cannot be empty"));
        }

        // Check if another category already has this name
        if self
            .categories
            .iter()
            .enumerate()
            .any(|(i, c)| i != idx && c.name == new_name)
        {
            return Err(anyhow::anyhow!("Category with this name already exists"));
        }

        if idx < self.categories.len() {
            self.categories[idx].rename(new_name);
            self.save_data()?;
            self.rebuild_feed_tree();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Invalid category index"))
        }
    }

    pub fn assign_feed_to_category(&mut self, feed_url: &str, category_idx: usize) -> Result<()> {
        if category_idx >= self.categories.len() {
            return Err(anyhow::anyhow!("Invalid category index"));
        }

        // Add feed to the selected category
        self.categories[category_idx].add_feed(feed_url);

        // Save the updated categories
        self.save_data()?;
        self.rebuild_feed_tree();

        Ok(())
    }

    pub fn remove_feed_from_category(&mut self, feed_url: &str, category_idx: usize) -> Result<()> {
        if category_idx >= self.categories.len() {
            return Err(anyhow::anyhow!("Invalid category index"));
        }

        let removed = self.categories[category_idx].remove_feed(feed_url);
        if removed {
            self.save_data()?;
            self.rebuild_feed_tree();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Feed not found in category"))
        }
    }

    pub fn toggle_category_expanded(&mut self, idx: usize) -> Result<()> {
        if idx < self.categories.len() {
            self.categories[idx].toggle_expanded();
            self.rebuild_feed_tree();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Invalid category index"))
        }
    }

    pub fn rebuild_feed_tree(&mut self) {
        self.feed_tree.clear();
        let mut categorized_feeds: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // Add categories and their feeds
        for (cat_idx, category) in self.categories.iter().enumerate() {
            self.feed_tree.push(TreeItem::Category(cat_idx));
            if category.expanded {
                for (feed_idx, feed) in self.feeds.iter().enumerate() {
                    if category.feeds.contains(&feed.url) {
                        self.feed_tree.push(TreeItem::Feed(feed_idx, Some(cat_idx)));
                        categorized_feeds.insert(feed.url.clone());
                    }
                }
            } else {
                // Still track which feeds are categorized even when collapsed
                for feed in &self.feeds {
                    if category.feeds.contains(&feed.url) {
                        categorized_feeds.insert(feed.url.clone());
                    }
                }
            }
        }

        // Add uncategorized feeds at the bottom
        for (feed_idx, feed) in self.feeds.iter().enumerate() {
            if !categorized_feeds.contains(&feed.url) {
                self.feed_tree.push(TreeItem::Feed(feed_idx, None));
            }
        }

        // Clamp selection
        if let Some(selected) = self.selected_tree_item {
            if selected >= self.feed_tree.len() {
                self.selected_tree_item = if self.feed_tree.is_empty() {
                    None
                } else {
                    Some(self.feed_tree.len() - 1)
                };
            }
        }
    }

    // When managing categories in UI
    pub fn get_category_for_feed(&self, feed_url: &str) -> Option<usize> {
        self.categories
            .iter()
            .position(|c| c.contains_feed(feed_url))
    }

    /// Update the maximum scroll value based on content height and viewport height
    pub fn update_detail_max_scroll(&mut self, content_lines: u16, viewport_height: u16) {
        // Maximum scroll is the content lines minus the viewport height
        // If content fits in viewport, max scroll is 0
        self.detail_max_scroll = content_lines.saturating_sub(viewport_height);
    }

    /// Clamp the current scroll position to valid bounds
    pub fn clamp_detail_scroll(&mut self) {
        if self.detail_vertical_scroll > self.detail_max_scroll {
            self.detail_vertical_scroll = self.detail_max_scroll;
        }
    }

    /// Update the maximum scroll value for preview pane based on content height and viewport height
    pub fn update_preview_max_scroll(&mut self, content_lines: u16, viewport_height: u16) {
        self.preview_max_scroll = content_lines.saturating_sub(viewport_height);
    }

    /// Clamp the preview scroll position to valid bounds
    pub fn clamp_preview_scroll(&mut self) {
        if self.preview_scroll > self.preview_max_scroll {
            self.preview_scroll = self.preview_max_scroll;
        }
    }

    /// Exit the detail view and reset scroll position. Also resets
    /// `input_mode` so an in-progress `ArticleSearch` session can't strand
    /// the user in an input mode that no other view renders a cue for.
    pub fn exit_detail_view(&mut self, new_view: View) {
        self.detail_vertical_scroll = 0;
        self.read_mode = false;
        self.clear_article_search();
        self.input_mode = InputMode::Normal;
        self.view = new_view;
    }

    pub fn toggle_read_mode(&mut self) {
        self.read_mode = !self.read_mode;
    }

    /// Drop any in-article search state. Called when the focused article
    /// changes, when the user presses `Esc` while typing a query, or when
    /// the detail view is exited. Also drops the cached body snapshot so a
    /// stale buffer can't feed a stray `n`/`N` press across an article
    /// change — the renderer re-populates on the next frame if a query is
    /// entered. `String::clear` keeps the allocation for reuse.
    pub fn clear_article_search(&mut self) {
        self.article_search_query.clear();
        self.article_search_current = None;
        self.article_search_matches.clear();
        self.article_body_cache.clear();
        self.article_body_width_cache = 0;
    }

    /// Advance the in-article match cursor by one and scroll the detail
    /// viewport so the new match is visible. No-op when there are no
    /// matches. Wraps around to 0 past the last match.
    pub fn next_article_match(&mut self) {
        if self.article_search_matches.is_empty() {
            return;
        }
        let cur = self.article_search_current.unwrap_or(0);
        let next = (cur + 1) % self.article_search_matches.len();
        self.article_search_current = Some(next);
        self.scroll_to_current_article_match();
    }

    /// Move the in-article match cursor back by one. Wraps from 0 to the
    /// last match. No-op when there are no matches.
    pub fn prev_article_match(&mut self) {
        if self.article_search_matches.is_empty() {
            return;
        }
        let len = self.article_search_matches.len();
        let cur = self.article_search_current.unwrap_or(0);
        let prev = if cur == 0 { len - 1 } else { cur - 1 };
        self.article_search_current = Some(prev);
        self.scroll_to_current_article_match();
    }

    fn scroll_to_current_article_match(&mut self) {
        let Some(idx) = self.article_search_current else {
            return;
        };
        let Some(m) = self.article_search_matches.get(idx) else {
            return;
        };
        // Leave two rows of context above the match so the user can read
        // the paragraph it lives in, not just the match itself.
        let row = wrapped_row_of_line(
            &self.article_body_cache,
            m.line,
            self.article_body_width_cache,
        );
        self.detail_vertical_scroll = row.saturating_sub(2);
        // Final clamp happens on the next render via `clamp_detail_scroll`.
    }

    /// Check if auto-refresh should trigger
    pub fn should_auto_refresh(&self) -> bool {
        if self.refresh_in_progress {
            return false;
        }

        // Check global refresh
        if self.config.general.refresh_enabled && self.config.general.auto_refresh_interval > 0 {
            if let Some(last_refresh) = self.last_refresh {
                if last_refresh.elapsed().as_secs() >= self.config.general.auto_refresh_interval {
                    return true;
                }
            } else {
                // Never refreshed, so we should refresh
                return true;
            }
        }

        // Check per-feed intervals — these act as trigger thresholds for a
        // global refresh (all feeds are refreshed together). True selective
        // per-feed refresh is not yet implemented.
        for (url, &interval) in &self.feed_refresh_intervals {
            if let Some(last) = self.last_feed_refresh.get(url) {
                if last.elapsed().as_secs() >= interval {
                    return true;
                }
            } else {
                // Never refreshed this feed individually — check global last refresh
                if let Some(last) = self.last_refresh {
                    if last.elapsed().as_secs() >= interval {
                        return true;
                    }
                } else {
                    // Never refreshed at all, trigger refresh
                    return true;
                }
            }
        }

        false
    }

    /// Extract domain from URL (e.g., "reddit.com" from "https://www.reddit.com/r/rust/.rss")
    pub(crate) fn extract_domain_from_url(url: &str) -> String {
        // Simple domain extraction
        if let Some(domain_start) = url.find("://") {
            let after_protocol = &url[domain_start + 3..];
            let domain_end = after_protocol.find('/').unwrap_or(after_protocol.len());
            let domain = &after_protocol[..domain_end];

            // Remove www. prefix if present
            domain.strip_prefix("www.").unwrap_or(domain).to_string()
        } else {
            // No protocol, try to extract domain directly
            let domain_end = url.find('/').unwrap_or(url.len());
            let domain = &url[..domain_end];

            domain.strip_prefix("www.").unwrap_or(domain).to_string()
        }
    }

    /// Calculate required delay before fetching from a domain
    fn calculate_required_delay(&self, domain: &str) -> std::time::Duration {
        if let Some(last_fetch) = self.last_domain_fetch.get(domain) {
            let elapsed = last_fetch.elapsed();
            let required_delay =
                std::time::Duration::from_millis(self.config.general.refresh_rate_limit_delay);

            if elapsed < required_delay {
                required_delay - elapsed
            } else {
                std::time::Duration::from_secs(0)
            }
        } else {
            std::time::Duration::from_secs(0)
        }
    }

    pub fn toggle_preview_pane(&mut self) {
        self.preview_pane = !self.preview_pane;
        self.preview_scroll = 0;
        self.preview_max_scroll = 0;
    }

    pub fn reset_preview_scroll(&mut self) {
        self.preview_scroll = 0;
        self.preview_max_scroll = 0;
    }

    /// Get new items since last session, returns (feed_idx, item_idx, feed_title)
    pub fn get_new_items_since_session(&self) -> Vec<(usize, usize, &str)> {
        let session_time = match self.last_session_time {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut items = Vec::new();
        for (feed_idx, feed) in self.feeds.iter().enumerate() {
            for (item_idx, item) in feed.items.iter().enumerate() {
                if let Some(parsed_date) = item.parsed_date {
                    if parsed_date > session_time {
                        items.push((feed_idx, item_idx, feed.title.as_str()));
                    }
                }
            }
        }
        items
    }

    /// Get summary stats for the "What's New" view
    pub fn get_summary_stats(&self) -> (usize, Vec<(String, usize)>) {
        let new_items = self.get_new_items_since_session();
        let total = new_items.len();

        // Count items per feed
        let mut feed_counts: HashMap<&str, usize> = HashMap::new();
        for &(_, _, feed_title) in &new_items {
            *feed_counts.entry(feed_title).or_insert(0) += 1;
        }

        let mut feeds_with_counts: Vec<(String, usize)> = feed_counts
            .into_iter()
            .map(|(name, count)| (name.to_string(), count))
            .collect();
        feeds_with_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        (total, feeds_with_counts)
    }

    /// Toggle between light and dark themes
    pub fn toggle_theme(&mut self) -> Result<()> {
        use crate::config::Theme;

        self.config.ui.theme = match self.config.ui.theme {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        };

        // Update cached color scheme
        self.color_scheme = ColorScheme::from_theme(&self.config.ui.theme);

        // Save the updated config
        self.config.save()?;

        Ok(())
    }

    pub fn extract_links_from_current_item(&mut self) {
        use scraper::{Html, Selector};

        self.extracted_links.clear();
        self.selected_link = 0;

        let description = if let Some(feed_idx) = self.selected_feed {
            if let Some(item_idx) = self.selected_item {
                self.feeds
                    .get(feed_idx)
                    .and_then(|f| f.items.get(item_idx))
                    .and_then(|item| item.description.as_ref())
                    .cloned()
            } else {
                None
            }
        } else {
            None
        };

        let Some(html_content) = description else {
            return;
        };

        let base_url = self
            .selected_feed
            .and_then(|fi| {
                self.selected_item.and_then(|ii| {
                    self.feeds
                        .get(fi)
                        .and_then(|f| f.items.get(ii).and_then(|item| item.link.as_ref()))
                })
            })
            .and_then(|link| url::Url::parse(link).ok());

        let document = Html::parse_document(&html_content);
        let mut seen_urls = std::collections::HashSet::new();

        // Extract links
        if let Ok(selector) = Selector::parse("a[href]") {
            for element in document.select(&selector) {
                if let Some(href) = element.value().attr("href") {
                    let resolved = if let Some(base) = &base_url {
                        base.join(href)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| href.to_string())
                    } else {
                        href.to_string()
                    };
                    if seen_urls.insert(resolved.clone()) {
                        let text = element.text().collect::<String>().trim().to_string();
                        self.extracted_links.push(ExtractedLink {
                            url: resolved,
                            text: if text.is_empty() {
                                "(no text)".to_string()
                            } else {
                                text
                            },
                            link_type: LinkType::Link,
                        });
                    }
                }
            }
        }

        // Extract images
        if let Ok(selector) = Selector::parse("img[src]") {
            for element in document.select(&selector) {
                if let Some(src) = element.value().attr("src") {
                    let resolved = if let Some(base) = &base_url {
                        base.join(src)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| src.to_string())
                    } else {
                        src.to_string()
                    };
                    if seen_urls.insert(resolved.clone()) {
                        let alt = element.value().attr("alt").unwrap_or("").trim().to_string();
                        self.extracted_links.push(ExtractedLink {
                            url: resolved,
                            text: if alt.is_empty() {
                                "(image)".to_string()
                            } else {
                                alt
                            },
                            link_type: LinkType::Image,
                        });
                    }
                }
            }
        }

        self.show_link_overlay = !self.extracted_links.is_empty();
        if self.extracted_links.is_empty() {
            self.error = Some("No links or images found in this article".to_string());
        }
    }

    /// Resolve `(feed_idx, item_idx)` for the focused article in the current
    /// view. Dashboard and Starred views derive the indices from their derived
    /// item list rather than from `selected_feed` (which is only populated
    /// once the user drills into FeedItems / FeedItemDetail). This is the
    /// single source of truth for "which item is focused right now".
    pub fn current_article_indices(&self) -> Option<(usize, usize)> {
        match self.view {
            View::Dashboard => {
                let sel = self.selected_item?;
                let active = self.active_dashboard_items();
                if sel >= active.len() {
                    return None;
                }
                Some(active[sel])
            }
            View::FeedItems | View::FeedItemDetail => {
                Some((self.selected_feed?, self.selected_item?))
            }
            View::Starred => {
                let sel = self.selected_item?;
                let starred = self.get_starred_dashboard_items();
                if sel >= starred.len() {
                    return None;
                }
                Some(starred[sel])
            }
            _ => None,
        }
    }

    /// Resolve the focused article based on the current view + selection.
    /// Returns None if nothing is selected or the indices don't resolve.
    pub fn current_article_context(&self) -> Option<ArticleContext<'_>> {
        let (feed_idx, item_idx) = self.current_article_indices()?;
        let feed = self.feeds.get(feed_idx)?;
        let item = feed.items.get(item_idx)?;
        let primary = item.primary_media();
        Some(ArticleContext {
            title: &item.title,
            url: item.link.as_deref(),
            author: item.author.as_deref(),
            formatted_date: item.formatted_date.as_deref(),
            feed_title: &feed.title,
            feed_url: &feed.url,
            plain_text: item.plain_text.as_deref(),
            media_url: primary.map(|m| m.url.as_str()),
            media_mime: primary.and_then(|m| m.mime.as_deref()),
            thumbnail_url: item.thumbnail.as_deref(),
        })
    }

    /// Look up a macro whose trigger matches the given key event.
    pub fn macro_for_key(
        &self,
        key: &crossterm::event::KeyEvent,
    ) -> Option<&crate::keybindings::MacroBinding> {
        self.macros.iter().find(|m| m.trigger.matches(key))
    }

    /// Look up extraction state for an item by its `(feed_idx, item_idx)`.
    /// Returns `None` if the indices don't resolve or no extraction has been
    /// requested for that item.
    pub fn extraction_state_for(
        &self,
        feed_idx: usize,
        item_idx: usize,
    ) -> Option<&ExtractionState> {
        let id = self.get_item_id(feed_idx, item_idx);
        if id.is_empty() {
            None
        } else {
            self.extracted.get(&id)
        }
    }

    /// Land a worker result for `id`. Drops the result if the slot has been
    /// evicted, removed (feed deleted), already taken to a non-Pending state,
    /// or if the request's generation no longer matches the current Pending
    /// slot — we never want to resurrect a dead cache entry or let a stale
    /// worker satisfy a re-requested slot.
    pub fn record_extraction_result(
        &mut self,
        id: String,
        generation: u64,
        result: Result<ExtractedArticle>,
    ) {
        if id.is_empty() {
            return;
        }
        // Must be Pending AND the generation must match. The generation
        // check closes the TOCTOU window where an LRU-evicted Pending slot
        // gets re-requested: the old worker's result would otherwise land
        // on the new Pending slot of the same id.
        match self.extracted.get(&id) {
            Some(ExtractionState::Pending { generation: g, .. }) if *g == generation => {}
            _ => return,
        }
        let state = match result {
            // `{:#}` flattens the anyhow cause chain into one line
            // ("outer: caused by: inner"), preserving context that the
            // default `{}` formatter drops. The user only ever sees the
            // top message in the detail view, but a chained message helps
            // when the failure is logged or reported.
            Ok(article) => ExtractionState::Ready(article),
            Err(e) => ExtractionState::Failed(format!("{:#}", e)),
        };
        self.insert_extraction(id, state);
    }

    /// Mark an item as `Pending` and append the request to the spawn queue.
    /// No-op if the item already has any state (Pending / Ready / Failed) —
    /// callers should clear or re-request explicitly. `safe_redirects` selects
    /// whether the worker uses the safe-redirect client (auto path) or the
    /// permissive one (manual path); see `ExtractionRequest::safe_redirects`.
    /// `priority_front=true` jumps the queue: an explicit user click should
    /// not wait behind a refresh's auto-extractions to the same host.
    pub fn request_extraction(
        &mut self,
        id: String,
        url: String,
        safe_redirects: bool,
        priority_front: bool,
    ) {
        if id.is_empty() || url.is_empty() {
            return;
        }
        if self.extracted.contains_key(&id) {
            return;
        }
        let generation = self.next_extraction_generation;
        self.next_extraction_generation = self.next_extraction_generation.wrapping_add(1);
        self.insert_extraction(
            id.clone(),
            ExtractionState::Pending {
                generation,
                started_at: Instant::now(),
                spawned: false,
            },
        );
        let req = ExtractionRequest {
            id,
            url,
            safe_redirects,
            generation,
        };
        if priority_front {
            self.pending_extraction_requests.push_front(req);
        } else {
            self.pending_extraction_requests.push_back(req);
        }
    }

    /// Sweep Pending slots that have been in-flight longer than
    /// `http_timeout * PENDING_WATCHDOG_MULTIPLIER`, flip them to
    /// `Failed("timed out")`, and return the count of pruned slots that
    /// were actually *spawned* — so the caller can release exactly that
    /// many inflight budget slots.
    ///
    /// A normally-completing worker (success, error, even a Rust panic caught
    /// by `catch_unwind`) always sends a result back through the channel. A
    /// worker that silently disappears — OS kill, SIGSEGV in a native dep,
    /// parent thread crash — leaves its slot stuck on Pending forever, which
    /// blocks `toggle_or_request_fulltext`'s `contains_key` guard. The
    /// returned count is what `run_app` passes to a saturating decrement of
    /// the shared `extract_inflight` counter so the pool doesn't leak budget
    /// slots across a session.
    ///
    /// Critically, we only count pruned slots whose `spawned` flag is true:
    /// a request that aged out while still queued never claimed an inflight
    /// permit, so releasing one for it would silently widen the effective
    /// worker cap. Both the watchdog path and the worker-exit path use
    /// saturating decrement, so a slow worker that completes AFTER the
    /// watchdog already released its slot can't underflow the counter.
    pub fn prune_stale_pending_extractions(&mut self, http_timeout: std::time::Duration) -> usize {
        let watchdog = http_timeout.saturating_mul(PENDING_WATCHDOG_MULTIPLIER);
        if watchdog.is_zero() {
            return 0;
        }
        let now = Instant::now();
        let mut to_fail: Vec<(String, bool)> = Vec::new();
        for (id, state) in self.extracted.iter() {
            if let ExtractionState::Pending {
                started_at,
                spawned,
                ..
            } = state
            {
                if now.duration_since(*started_at) > watchdog {
                    to_fail.push((id.clone(), *spawned));
                }
            }
        }
        let mut spawned_pruned = 0usize;
        for (id, spawned) in to_fail {
            if spawned {
                spawned_pruned += 1;
            }
            // Mutate in place — must NOT route through `insert_extraction`,
            // which promotes the entry to MRU. A watchdog-failed slot is
            // strictly less useful than newer Ready entries, so it should
            // keep its original LRU position and be evicted ahead of them.
            if let Some(state) = self.extracted.get_mut(&id) {
                *state = ExtractionState::Failed("timed out".to_string());
            }
        }
        spawned_pruned
    }

    /// Mark a Pending slot as spawned (the spawn loop has bumped the inflight
    /// counter and successfully called `thread::Builder::spawn`). Idempotent
    /// — a no-op for any other state, so race conditions where the worker
    /// already landed a result before this is called are harmless.
    pub(crate) fn mark_extraction_spawned(&mut self, id: &str, generation: u64) {
        if let Some(ExtractionState::Pending {
            generation: g,
            spawned,
            ..
        }) = self.extracted.get_mut(id)
        {
            if *g == generation {
                *spawned = true;
            }
        }
    }

    /// Forget a single extraction slot (both the state and its LRU
    /// position). The deque is kept in sync with the map.
    pub(crate) fn remove_extraction(&mut self, id: &str) {
        self.extracted.remove(id);
        if let Some(pos) = self.extracted_order.iter().position(|x| x.as_str() == id) {
            self.extracted_order.remove(pos);
        }
    }

    fn insert_extraction(&mut self, id: String, state: ExtractionState) {
        // If id is already present, move its LRU position to most-recent and
        // overwrite the state.
        if let Some(existing) = self.extracted.get_mut(&id) {
            *existing = state;
            if let Some(pos) = self.extracted_order.iter().position(|x| x == &id) {
                self.extracted_order.remove(pos);
            }
            self.extracted_order.push_back(id);
            return;
        }
        // LRU eviction with a soft-protect for `Pending { spawned: true }`
        // slots. Evicting a spawned Pending would hide the slot from the
        // stale-Pending watchdog (it iterates `extracted`), leaving a dead
        // worker's inflight permit leaked for the rest of the session.
        // Skip those slots and evict the next-oldest evictable entry.
        //
        // The protected-slot count is bounded by the worker pool (see
        // `EXTRACTION_MAX_INFLIGHT` in `tui.rs`), so the cache size is
        // bounded at `EXTRACTED_CACHE_CAP + EXTRACTION_MAX_INFLIGHT` — a
        // soft cap, not hard, but still strictly bounded. If every entry
        // happens to be a spawned Pending (would mean the entire pool has
        // been stuck for a long time), the cache transiently grows past
        // CAP; the watchdog will eventually flip those slots to Failed,
        // restoring evictability on the next insert.
        while self.extracted_order.len() >= EXTRACTED_CACHE_CAP {
            let evict_idx = self.extracted_order.iter().position(|id| {
                !matches!(
                    self.extracted.get(id),
                    Some(ExtractionState::Pending { spawned: true, .. })
                )
            });
            match evict_idx {
                Some(idx) => {
                    // `VecDeque::remove` is O(N) for arbitrary indices,
                    // but N ≤ ~500 and eviction only fires at the cap.
                    if let Some(evict) = self.extracted_order.remove(idx) {
                        self.extracted.remove(&evict);
                    }
                }
                None => break,
            }
        }
        self.extracted.insert(id.clone(), state);
        self.extracted_order.push_back(id);
    }

    /// Handler for the FetchFullText keybinding. Resolves the focused
    /// article, then either toggles the view (if extraction is Ready) or
    /// queues a new extraction request. Re-queues on `Failed` so the user
    /// can retry. No-op on `Pending` (extraction already in flight).
    pub fn toggle_or_request_fulltext(&mut self) {
        let Some((feed_idx, item_idx)) = self.current_article_indices() else {
            self.error = Some("No article in focus".to_string());
            return;
        };
        let id = self.get_item_id(feed_idx, item_idx);
        if id.is_empty() {
            self.error = Some("No article in focus".to_string());
            return;
        }
        let url = match self
            .feeds
            .get(feed_idx)
            .and_then(|f| f.items.get(item_idx))
            .and_then(|i| i.link.clone())
        {
            Some(u) => u,
            None => {
                self.error = Some("Article has no URL".to_string());
                return;
            }
        };
        // Track whether this invocation actually queued / unqueued a
        // request so we only show the off-view hint when something happened.
        let mut queued_or_toggled = false;
        match self.extracted.get(&id) {
            Some(ExtractionState::Ready(_)) => {
                self.show_extracted = !self.show_extracted;
                queued_or_toggled = true;
            }
            Some(ExtractionState::Pending { .. }) => {
                // already in flight — leave it
            }
            Some(ExtractionState::Failed(_)) => {
                self.remove_extraction(&id);
                self.show_extracted = true;
                // Manual retry: jump the queue ahead of any auto-extractions
                // a refresh may have enqueued.
                self.request_extraction(id, url, false, true);
                queued_or_toggled = true;
            }
            None => {
                self.show_extracted = true;
                // Manual request: priority over auto-path enqueues.
                self.request_extraction(id, url, false, true);
                queued_or_toggled = true;
            }
        }
        // The detail view is what renders extracted state; when the action
        // fires from anywhere else (macro path on Dashboard / FeedItems /
        // Starred) the user gets no visible feedback. Surface a hint so the
        // request doesn't feel like a no-op.
        if queued_or_toggled && self.view != View::FeedItemDetail {
            self.success_message = Some("Extracting full text (open article to view)".to_string());
            self.success_message_time = Some(std::time::Instant::now());
        }
    }

    /// Called from the detail renderer each frame. If the focused article
    /// changed since the last frame, reset the global `show_extracted`
    /// toggle to `true` so the user doesn't carry a "view summary" choice
    /// from one article to another. Idempotent on unchanged focus.
    pub fn sync_detail_focus_anchor(&mut self) {
        let current_focus = self.current_article_indices();
        if self.last_detail_focus != current_focus {
            self.show_extracted = true;
            // A `/foo` left over from the previous article would be
            // misleading on a brand new body — drop it on focus change.
            self.clear_article_search();
            self.last_detail_focus = current_focus;
        }
    }
}

/// Find all occurrences of `query` in `body` using ripgrep-style smart-case
/// matching: case-sensitive when `query` contains any uppercase character
/// (Unicode-aware), case-insensitive (ASCII fold) otherwise. Returns an
/// empty `Vec` for an empty query. Lines are indexed by `body.split('\n')`
/// so the result aligns with what the detail renderer draws.
///
/// INVARIANT: the case fold used here MUST be byte-length-preserving so
/// that byte offsets within the folded haystack remain valid byte offsets
/// (and valid UTF-8 boundaries) within the original line — the detail
/// renderer slices the *original* line at `[start..end]` to build the
/// highlight span. `str::to_ascii_lowercase` satisfies this (it only
/// flips bit 5 of bytes in `0x41..=0x5A`, never altering byte length or
/// touching multi-byte UTF-8 sequences). Full Unicode case folding via
/// `str::to_lowercase` does NOT — e.g. `"İ".to_lowercase() == "i\u{307}"`
/// changes the byte length. Do not swap the fold without redesigning the
/// offset model (e.g. by maintaining a parallel folded→original byte map).
/// The side effect is that non-ASCII characters do not fold in
/// case-insensitive mode (e.g. `"café"` will not match `"Café"`); that's
/// the price of the invariant.
pub fn find_article_matches(body: &str, query: &str) -> Vec<ArticleMatch> {
    if query.is_empty() {
        return Vec::new();
    }
    let case_sensitive = query.chars().any(|c| c.is_uppercase());
    let needle = if case_sensitive {
        query.to_string()
    } else {
        query.to_ascii_lowercase()
    };
    let needle_len = needle.len();
    let mut out = Vec::new();
    for (line_idx, line) in body.split('\n').enumerate() {
        if case_sensitive {
            scan_line(line, &needle, needle_len, line_idx, &mut out);
        } else {
            let folded = line.to_ascii_lowercase();
            scan_line(&folded, &needle, needle_len, line_idx, &mut out);
        }
    }
    out
}

fn scan_line(
    haystack: &str,
    needle: &str,
    needle_len: usize,
    line_idx: usize,
    out: &mut Vec<ArticleMatch>,
) {
    let mut start = 0;
    while start <= haystack.len() {
        let rest = &haystack[start..];
        match rest.find(needle) {
            Some(rel) => {
                let pos = start + rel;
                let end = pos + needle_len;
                out.push(ArticleMatch {
                    line: line_idx,
                    start: pos,
                    end,
                });
                // Step past the match (guard against zero-length needle —
                // we already filter that, but belt-and-braces).
                start = end.max(pos + 1);
            }
            None => break,
        }
    }
}

/// Cumulative wrapped-row count for source lines `0..target_line_idx`.
/// Matches the per-line walk in `count_wrapped_lines` so scroll positions
/// remain consistent with how `Paragraph` lays out the body. Returns 0 for
/// `target_line_idx == 0` or a zero `width`.
pub fn wrapped_row_of_line(body: &str, target_line_idx: usize, width: usize) -> u16 {
    if width == 0 || target_line_idx == 0 {
        return 0;
    }
    let mut row: u16 = 0;
    for (i, line) in body.split('\n').enumerate() {
        if i >= target_line_idx {
            break;
        }
        if line.is_empty() {
            row = row.saturating_add(1);
        } else {
            let lw = unicode_width::UnicodeWidthStr::width(line);
            if lw == 0 {
                row = row.saturating_add(1);
            } else {
                let wrapped = lw.div_ceil(width).max(1);
                row = row.saturating_add(wrapped as u16);
            }
        }
    }
    row
}

/// Build an `ArticleContext` directly from a `Feed` + `FeedItem` pair —
/// the path used by `exec_on_new`, where the new item may not yet live
/// inside `App::feeds`.
pub fn article_context_from<'a>(feed: &'a Feed, item: &'a FeedItem) -> ArticleContext<'a> {
    let primary = item.primary_media();
    ArticleContext {
        title: &item.title,
        url: item.link.as_deref(),
        author: item.author.as_deref(),
        formatted_date: item.formatted_date.as_deref(),
        feed_title: &feed.title,
        feed_url: &feed.url,
        plain_text: item.plain_text.as_deref(),
        media_url: primary.map(|m| m.url.as_str()),
        media_mime: primary.and_then(|m| m.mime.as_deref()),
        thumbnail_url: item.thumbnail.as_deref(),
    }
}

/// Build the same identifier scheme used by `read_items`/`starred_items` from
/// a feed + item that may not yet be in `App::feeds`.
pub fn make_item_id(feed: &Feed, item: &FeedItem) -> String {
    if let Some(link) = &item.link {
        return link.clone();
    }
    format!("{}_{}", feed.url, item.title)
}

/// Update `seen_items` and `feeds_seeded` to reflect this feed having been
/// successfully fetched. Returns the indices into `feed.items` of items
/// that are newly seen *and should fire exec_on_new*. On the first
/// successful fetch of a feed (URL not yet in `feeds_seeded`), the seen
/// set is populated silently and an empty Vec is returned.
pub fn mark_feed_seen(app: &mut App, feed: &Feed) -> Vec<usize> {
    let is_first_fetch = !app.feeds_seeded.contains(&feed.url);
    let mut newly_seen = Vec::new();
    for (idx, item) in feed.items.iter().enumerate() {
        let id = make_item_id(feed, item);
        let inserted = app.seen_items.insert(id);
        if inserted && !is_first_fetch {
            newly_seen.push(idx);
        }
    }
    // Only flip the seeded bit on a fetch that actually returned items. A
    // transiently-empty first fetch (server hiccup, parser edge case) would
    // otherwise mark the feed as seeded with no items recorded; the next
    // non-empty fetch would then treat every item as new and firehose
    // exec_on_new for the entire backlog.
    if is_first_fetch && !feed.items.is_empty() {
        app.feeds_seeded.insert(feed.url.clone());
    }
    newly_seen
}

/// Substitute newsboat-style %X placeholders into each argv token. Untrusted
/// content is never interpreted by a shell — placeholders are inserted
/// verbatim into individual argv elements.
///
/// Variables: %t title, %u url, %a author, %d formatted-date,
///            %f feed-title, %F feed-url,
///            %m primary-media-url, %M primary-media-mime, %i thumbnail-url,
///            %% literal %.
///
/// Placeholders that resolve to no value (e.g. `%m` on a plain text item)
/// expand to the empty string, matching how `%u` behaves on items missing
/// `link`. `%m` and `%u` are intentionally orthogonal — no automatic
/// fallback — so users can compose them explicitly in their macros.
pub fn expand_argv_template(template: &[String], ctx: &ArticleContext<'_>) -> Vec<String> {
    template
        .iter()
        .map(|tok| expand_one_token(tok, ctx))
        .collect()
}

fn expand_one_token(tok: &str, ctx: &ArticleContext<'_>) -> String {
    let mut out = String::with_capacity(tok.len());
    let mut chars = tok.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('t') => out.push_str(ctx.title),
            Some('u') => out.push_str(ctx.url.unwrap_or("")),
            Some('a') => out.push_str(ctx.author.unwrap_or("")),
            Some('d') => out.push_str(ctx.formatted_date.unwrap_or("")),
            Some('f') => out.push_str(ctx.feed_title),
            Some('F') => out.push_str(ctx.feed_url),
            Some('m') => out.push_str(ctx.media_url.unwrap_or("")),
            Some('M') => out.push_str(ctx.media_mime.unwrap_or("")),
            Some('i') => out.push_str(ctx.thumbnail_url.unwrap_or("")),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}

/// Build the byte payload that gets piped to a `pipe-to` child's stdin.
pub fn make_pipe_payload(ctx: &ArticleContext<'_>, kind: crate::keybindings::StdinKind) -> Vec<u8> {
    use crate::keybindings::StdinKind;
    match kind {
        StdinKind::None => Vec::new(),
        StdinKind::Title => ctx.title.as_bytes().to_vec(),
        StdinKind::Url => ctx.url.unwrap_or("").as_bytes().to_vec(),
        StdinKind::Body => ctx
            .plain_text
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default(),
        StdinKind::Metadata => {
            let mut out = String::new();
            out.push_str("Title: ");
            out.push_str(ctx.title);
            out.push('\n');
            if let Some(u) = ctx.url {
                out.push_str("URL: ");
                out.push_str(u);
                out.push('\n');
            }
            if let Some(a) = ctx.author {
                out.push_str("Author: ");
                out.push_str(a);
                out.push('\n');
            }
            if let Some(d) = ctx.formatted_date {
                out.push_str("Date: ");
                out.push_str(d);
                out.push('\n');
            }
            out.push_str("Feed: ");
            out.push_str(ctx.feed_title);
            out.push('\n');
            out.push('\n');
            if let Some(body) = ctx.plain_text {
                out.push_str(body);
            }
            out.into_bytes()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_domain_from_url() {
        assert_eq!(
            App::extract_domain_from_url("https://www.reddit.com/r/rust/.rss"),
            "reddit.com"
        );
        assert_eq!(
            App::extract_domain_from_url("https://news.ycombinator.com/rss"),
            "news.ycombinator.com"
        );
        assert_eq!(
            App::extract_domain_from_url("http://example.com/feed.xml"),
            "example.com"
        );
        assert_eq!(
            App::extract_domain_from_url("https://www.example.com/"),
            "example.com"
        );
    }

    /// Helper to create a minimal App with test feeds, avoiding filesystem I/O
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
                        media: Vec::new(),
                        thumbnail: None,
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
                        media: Vec::new(),
                        thumbnail: None,
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
                    media: Vec::new(),
                    thumbnail: None,
                }],
            },
        ];
        app.update_dashboard();
        app
    }

    #[test]
    fn test_get_new_items_since_session_no_session_time() {
        let mut app = make_test_app();
        app.last_session_time = None;
        let items = app.get_new_items_since_session();
        assert!(items.is_empty(), "No items when no session time is set");
    }

    #[test]
    fn test_get_new_items_since_session_filters_by_date() {
        let mut app = make_test_app();
        // Set session time to 12 hours ago — should pick up items from last few hours
        app.last_session_time = Some(Utc::now() - chrono::Duration::hours(12));
        let items = app.get_new_items_since_session();
        // "New Article" (1h ago) and "Another New" (2h ago) are newer than 12h ago
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_get_new_items_since_session_skips_no_date() {
        let mut app = make_test_app();
        app.last_session_time = Some(Utc::now() - chrono::Duration::hours(12));
        // Remove parsed_date from one item
        app.feeds[0].items[1].parsed_date = None;
        let items = app.get_new_items_since_session();
        // Only "Another New" from Feed Two should match
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].2, "Feed Two");
    }

    #[test]
    fn test_get_summary_stats() {
        let mut app = make_test_app();
        app.last_session_time = Some(Utc::now() - chrono::Duration::hours(12));
        let (total, feeds) = app.get_summary_stats();
        assert_eq!(total, 2);
        // Both feeds have 1 new item each
        assert_eq!(feeds.len(), 2);
        // Sorted by count descending (both have 1, so order is stable but either is fine)
        assert!(feeds.iter().all(|(_, count)| *count == 1));
    }

    #[test]
    fn test_get_summary_stats_sorting() {
        let mut app = make_test_app();
        // Set session time far in the past so all items with dates are "new"
        app.last_session_time = Some(Utc::now() - chrono::Duration::days(365));
        let (total, feeds) = app.get_summary_stats();
        assert_eq!(total, 3);
        // Feed One has 2 items, Feed Two has 1 — Feed One should be first
        assert_eq!(feeds[0].0, "Feed One");
        assert_eq!(feeds[0].1, 2);
        assert_eq!(feeds[1].0, "Feed Two");
        assert_eq!(feeds[1].1, 1);
    }

    #[test]
    fn test_get_summary_stats_tie_break_by_name() {
        let mut app = make_test_app();
        // Add a third feed whose name sorts before "Feed Two" but ties on count
        app.feeds.push(Feed {
            url: "https://example.com/feed3".to_string(),
            title: "Alpha Feed".to_string(),
            title_lower: "alpha feed".to_string(),
            items: vec![FeedItem {
                title: "Alpha New".to_string(),
                title_lower: "alpha new".to_string(),
                link: Some("https://example.com/alpha".to_string()),
                description: Some("Alpha content".to_string()),
                pub_date: None,
                author: None,
                formatted_date: None,
                parsed_date: Some(Utc::now() - chrono::Duration::hours(3)),
                plain_text: Some("Alpha content".to_string()),
                media: Vec::new(),
                thumbnail: None,
            }],
        });
        app.last_session_time = Some(Utc::now() - chrono::Duration::days(365));

        // Run twice — order must be identical across calls regardless of HashMap iteration.
        let (_, first) = app.get_summary_stats();
        let (_, second) = app.get_summary_stats();
        assert_eq!(first, second);

        // Feed One leads on count; "Alpha Feed" beats "Feed Two" alphabetically on the tie.
        assert_eq!(first[0], ("Feed One".to_string(), 2));
        assert_eq!(first[1], ("Alpha Feed".to_string(), 1));
        assert_eq!(first[2], ("Feed Two".to_string(), 1));
    }

    #[test]
    fn test_live_search_clamps_selection() {
        let mut app = make_test_app();
        // Start with selection beyond what search will return
        app.selected_item = Some(100);
        app.live_search("new");
        // Should clamp to last result, not reset to 0
        assert!(app.selected_item.is_some());
        let sel = app.selected_item.unwrap();
        assert!(sel < app.filtered_items.len());
        assert_eq!(sel, app.filtered_items.len() - 1);
    }

    #[test]
    fn test_live_search_preserves_valid_selection() {
        let mut app = make_test_app();
        app.selected_item = Some(0);
        app.live_search("new");
        // Selection 0 is valid, should stay
        assert_eq!(app.selected_item, Some(0));
    }

    #[test]
    fn test_live_search_sets_selection_when_none() {
        let mut app = make_test_app();
        app.selected_item = None;
        app.live_search("new");
        // Should set to 0 when there are results
        assert_eq!(app.selected_item, Some(0));
    }

    #[test]
    fn test_live_search_no_results() {
        let mut app = make_test_app();
        app.selected_item = Some(1);
        app.live_search("zzzznonexistent");
        // No results, selection should remain as-is (count == 0, match arm falls through)
        assert_eq!(app.selected_item, Some(1));
    }

    #[test]
    fn test_toggle_preview_pane() {
        let mut app = make_test_app();
        // Initial state depends on ui.show_preview in the loaded config, so set
        // it explicitly — this test asserts toggle behavior, not the default.
        app.preview_pane = false;
        app.preview_scroll = 5;
        app.preview_max_scroll = 10;

        app.toggle_preview_pane();
        assert!(app.preview_pane);
        assert_eq!(app.preview_scroll, 0);
        assert_eq!(app.preview_max_scroll, 0);

        app.toggle_preview_pane();
        assert!(!app.preview_pane);
    }

    #[test]
    fn test_reset_preview_scroll() {
        let mut app = make_test_app();
        app.preview_scroll = 10;
        app.preview_max_scroll = 20;

        app.reset_preview_scroll();
        assert_eq!(app.preview_scroll, 0);
        assert_eq!(app.preview_max_scroll, 0);
    }

    #[test]
    fn test_update_preview_max_scroll() {
        let mut app = make_test_app();
        app.update_preview_max_scroll(50, 20);
        assert_eq!(app.preview_max_scroll, 30);

        // Content fits in viewport
        app.update_preview_max_scroll(10, 20);
        assert_eq!(app.preview_max_scroll, 0);
    }

    #[test]
    fn test_clamp_preview_scroll() {
        let mut app = make_test_app();
        app.preview_max_scroll = 10;
        app.preview_scroll = 15;
        app.clamp_preview_scroll();
        assert_eq!(app.preview_scroll, 10);

        // Already valid
        app.preview_scroll = 5;
        app.clamp_preview_scroll();
        assert_eq!(app.preview_scroll, 5);
    }

    #[test]
    fn test_should_auto_refresh() {
        let mut app = App::new();

        // Should not refresh when disabled
        app.config.general.refresh_enabled = false;
        app.config.general.auto_refresh_interval = 300;
        assert!(!app.should_auto_refresh());

        // Should not refresh when interval is 0
        app.config.general.refresh_enabled = true;
        app.config.general.auto_refresh_interval = 0;
        assert!(!app.should_auto_refresh());

        // Should refresh when enabled and never refreshed before
        app.config.general.refresh_enabled = true;
        app.config.general.auto_refresh_interval = 300;
        app.last_refresh = None;
        assert!(app.should_auto_refresh());

        // Should not refresh when in progress
        app.refresh_in_progress = true;
        assert!(!app.should_auto_refresh());
    }

    #[test]
    fn test_should_auto_refresh_per_feed_interval() {
        let mut app = App::new();
        // Disable global refresh
        app.config.general.refresh_enabled = false;
        app.config.general.auto_refresh_interval = 0;

        // Add a per-feed interval
        app.feed_refresh_intervals
            .insert("https://example.com/feed".to_string(), 60);

        // Never refreshed — should trigger
        assert!(app.should_auto_refresh());

        // Mark as recently refreshed globally
        app.last_refresh = Some(std::time::Instant::now());
        assert!(!app.should_auto_refresh());
    }

    #[test]
    fn test_rebuild_feed_tree_uncategorized_only() {
        let mut app = make_test_app();
        app.categories.clear();
        app.rebuild_feed_tree();

        // Should have exactly 2 uncategorized feed entries
        assert_eq!(app.feed_tree.len(), 2);
        assert!(matches!(app.feed_tree[0], TreeItem::Feed(0, None)));
        assert!(matches!(app.feed_tree[1], TreeItem::Feed(1, None)));
    }

    #[test]
    fn test_rebuild_feed_tree_with_category() {
        let mut app = make_test_app();
        app.categories.clear();
        let mut cat = FeedCategory::new("Tech");
        cat.add_feed("https://example.com/feed1");
        app.categories.push(cat);
        app.rebuild_feed_tree();

        // Category node + expanded feed + 1 uncategorized feed = 3
        assert_eq!(app.feed_tree.len(), 3);
        assert!(matches!(app.feed_tree[0], TreeItem::Category(0)));
        assert!(matches!(app.feed_tree[1], TreeItem::Feed(0, Some(0))));
        assert!(matches!(app.feed_tree[2], TreeItem::Feed(1, None)));
    }

    #[test]
    fn test_rebuild_feed_tree_collapsed_category() {
        let mut app = make_test_app();
        app.categories.clear();
        let mut cat = FeedCategory::new("Tech");
        cat.add_feed("https://example.com/feed1");
        cat.expanded = false;
        app.categories.push(cat);
        app.rebuild_feed_tree();

        // Category node + 1 uncategorized (feed1 is hidden, feed2 uncategorized)
        assert_eq!(app.feed_tree.len(), 2);
        assert!(matches!(app.feed_tree[0], TreeItem::Category(0)));
        assert!(matches!(app.feed_tree[1], TreeItem::Feed(1, None)));
    }

    #[test]
    fn test_rebuild_feed_tree_clamps_selection() {
        let mut app = make_test_app();
        app.categories.clear();
        app.rebuild_feed_tree();
        // Set selection beyond bounds
        app.selected_tree_item = Some(100);
        app.rebuild_feed_tree();
        assert_eq!(app.selected_tree_item, Some(app.feed_tree.len() - 1));
    }

    #[test]
    fn test_rebuild_feed_tree_empty() {
        let mut app = App::new();
        app.feeds.clear();
        app.categories.clear();
        app.selected_tree_item = Some(5);
        app.rebuild_feed_tree();
        assert!(app.feed_tree.is_empty());
        assert_eq!(app.selected_tree_item, None);
    }

    #[test]
    fn test_mark_all_dashboard_read() {
        let mut app = make_test_app();
        app.read_items.clear();
        // All 3 items should be unread initially
        assert!(!app.is_item_read(0, 0));
        assert!(!app.is_item_read(0, 1));
        assert!(!app.is_item_read(1, 0));

        let count = app.mark_all_dashboard_read().unwrap();
        assert_eq!(count, 3);

        assert!(app.is_item_read(0, 0));
        assert!(app.is_item_read(0, 1));
        assert!(app.is_item_read(1, 0));

        // Calling again should mark 0 new items
        let count = app.mark_all_dashboard_read().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_mark_all_feed_read() {
        let mut app = make_test_app();
        app.read_items.clear();
        let count = app.mark_all_feed_read(0).unwrap();
        assert_eq!(count, 2); // Feed 0 has 2 items

        assert!(app.is_item_read(0, 0));
        assert!(app.is_item_read(0, 1));
        assert!(!app.is_item_read(1, 0)); // Feed 1 untouched
    }

    #[test]
    fn test_mark_all_feed_read_invalid_index() {
        let mut app = make_test_app();
        let count = app.mark_all_feed_read(999).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_mark_all_starred_read() {
        let mut app = make_test_app();
        app.read_items.clear();
        app.starred_items.clear();
        // Star two items
        app.toggle_item_starred(0, 0).unwrap();
        app.toggle_item_starred(1, 0).unwrap();

        let count = app.mark_all_starred_read().unwrap();
        assert_eq!(count, 2);
        assert!(app.is_item_read(0, 0));
        assert!(app.is_item_read(1, 0));
        assert!(!app.is_item_read(0, 1)); // Unstarred item untouched
    }

    #[test]
    fn test_extract_links_from_html() {
        let mut app = make_test_app();
        app.selected_feed = Some(0);
        app.selected_item = Some(0);

        // Set HTML content with links and images
        app.feeds[0].items[0].description = Some(
            r#"<p>Check out <a href="https://example.com/page">this page</a>
            and <a href="https://example.com/other">another link</a></p>
            <img src="https://example.com/image.png" alt="test image">"#
                .to_string(),
        );

        app.extract_links_from_current_item();

        assert!(app.show_link_overlay);
        assert_eq!(app.extracted_links.len(), 3);
        assert_eq!(app.extracted_links[0].url, "https://example.com/page");
        assert_eq!(app.extracted_links[0].text, "this page");
        assert!(matches!(app.extracted_links[0].link_type, LinkType::Link));
        assert_eq!(app.extracted_links[2].url, "https://example.com/image.png");
        assert!(matches!(app.extracted_links[2].link_type, LinkType::Image));
    }

    #[test]
    fn test_extract_links_deduplicates() {
        let mut app = make_test_app();
        app.selected_feed = Some(0);
        app.selected_item = Some(0);

        app.feeds[0].items[0].description = Some(
            r#"<a href="https://example.com">link1</a>
            <a href="https://example.com">link2</a>"#
                .to_string(),
        );

        app.extract_links_from_current_item();

        assert_eq!(app.extracted_links.len(), 1);
    }

    #[test]
    fn test_extract_links_no_content() {
        let mut app = make_test_app();
        app.selected_feed = Some(0);
        app.selected_item = Some(0);
        app.feeds[0].items[0].description = None;

        app.extract_links_from_current_item();

        assert!(!app.show_link_overlay);
        assert!(app.extracted_links.is_empty());
    }

    #[test]
    fn test_extract_links_resolves_relative_urls() {
        let mut app = make_test_app();
        app.selected_feed = Some(0);
        app.selected_item = Some(0);
        // Item has a base URL via its link
        app.feeds[0].items[0].link = Some("https://example.com/article/123".to_string());
        app.feeds[0].items[0].description = Some(r#"<a href="/about">About</a>"#.to_string());

        app.extract_links_from_current_item();

        assert_eq!(app.extracted_links.len(), 1);
        assert_eq!(app.extracted_links[0].url, "https://example.com/about");
    }

    fn synthetic_ctx() -> ArticleContext<'static> {
        ArticleContext {
            title: "Hello %u world",
            url: Some("https://ex.com/a"),
            author: Some("Alice"),
            formatted_date: Some("2 hours ago"),
            feed_title: "Example",
            feed_url: "https://ex.com/feed.xml",
            plain_text: Some("body text"),
            media_url: Some("https://ex.com/v.mp4"),
            media_mime: Some("video/mp4"),
            thumbnail_url: Some("https://ex.com/thumb.jpg"),
        }
    }

    fn synthetic_ctx_no_media() -> ArticleContext<'static> {
        ArticleContext {
            title: "Plain",
            url: Some("https://ex.com/p"),
            author: None,
            formatted_date: None,
            feed_title: "Example",
            feed_url: "https://ex.com/feed.xml",
            plain_text: None,
            media_url: None,
            media_mime: None,
            thumbnail_url: None,
        }
    }

    #[test]
    fn test_expand_argv_template_per_token() {
        let tmpl = vec![
            "yt-dlp".to_string(),
            "%u".to_string(),
            "--title=%t".to_string(),
        ];
        let argv = expand_argv_template(&tmpl, &synthetic_ctx());
        assert_eq!(argv[0], "yt-dlp");
        // The url replaces %u literally — but the title contains a %u which
        // must NOT be re-expanded (we only expand once, in the template, not
        // in the substituted content).
        assert_eq!(argv[1], "https://ex.com/a");
        assert_eq!(argv[2], "--title=Hello %u world");
    }

    #[test]
    fn test_expand_argv_template_percent_literal_and_unknown() {
        let tmpl = vec!["a%%b".to_string(), "x%zy".to_string()];
        let argv = expand_argv_template(&tmpl, &synthetic_ctx());
        assert_eq!(argv[0], "a%b");
        // Unknown escapes are passed through verbatim.
        assert_eq!(argv[1], "x%zy");
    }

    #[test]
    fn test_expand_media_placeholders() {
        let tmpl = vec![
            "%m".to_string(),
            "--mime=%M".to_string(),
            "--thumb=%i".to_string(),
        ];
        let argv = expand_argv_template(&tmpl, &synthetic_ctx());
        assert_eq!(argv[0], "https://ex.com/v.mp4");
        assert_eq!(argv[1], "--mime=video/mp4");
        assert_eq!(argv[2], "--thumb=https://ex.com/thumb.jpg");
    }

    #[test]
    fn test_expand_media_placeholders_empty_when_absent() {
        // `%m` / `%M` / `%i` on an item without media expand to "", not to
        // the page URL — placeholders are orthogonal to `%u`.
        let tmpl = vec![
            "%m".to_string(),
            "%M".to_string(),
            "%i".to_string(),
            "%u".to_string(),
        ];
        let argv = expand_argv_template(&tmpl, &synthetic_ctx_no_media());
        assert_eq!(argv[0], "");
        assert_eq!(argv[1], "");
        assert_eq!(argv[2], "");
        assert_eq!(argv[3], "https://ex.com/p");
    }

    #[test]
    fn test_make_pipe_payload_kinds() {
        use crate::keybindings::StdinKind;
        let ctx = synthetic_ctx();
        assert_eq!(make_pipe_payload(&ctx, StdinKind::None), Vec::<u8>::new());
        assert_eq!(make_pipe_payload(&ctx, StdinKind::Title), b"Hello %u world");
        assert_eq!(make_pipe_payload(&ctx, StdinKind::Url), b"https://ex.com/a");
        assert_eq!(make_pipe_payload(&ctx, StdinKind::Body), b"body text");
        let metadata = make_pipe_payload(&ctx, StdinKind::Metadata);
        let s = String::from_utf8(metadata).unwrap();
        assert!(s.contains("Title: Hello %u world"));
        assert!(s.contains("URL: https://ex.com/a"));
        assert!(s.contains("Author: Alice"));
        assert!(s.contains("Feed: Example"));
        assert!(s.ends_with("body text"));
    }

    fn synthetic_feed(url: &str, item_titles: &[&str]) -> Feed {
        let items = item_titles
            .iter()
            .map(|t| FeedItem {
                title: t.to_string(),
                link: Some(format!("https://ex.com/a/{}", t)),
                description: None,
                pub_date: None,
                author: None,
                formatted_date: None,
                parsed_date: None,
                plain_text: None,
                title_lower: t.to_lowercase(),
                media: Vec::new(),
                thumbnail: None,
            })
            .collect();
        Feed {
            url: url.to_string(),
            title: "Example".to_string(),
            items,
            title_lower: "example".to_string(),
        }
    }

    #[test]
    fn test_mark_feed_seen_first_fetch_seeds_silently() {
        let mut app = App::new();
        let feed = synthetic_feed("https://ex.com/feed.xml", &["A", "B", "C"]);

        let newly = mark_feed_seen(&mut app, &feed);
        assert!(
            newly.is_empty(),
            "first fetch must return no newly-seen items"
        );
        assert_eq!(app.seen_items.len(), 3);
        assert!(app.feeds_seeded.contains("https://ex.com/feed.xml"));
    }

    #[test]
    fn test_mark_feed_seen_second_fetch_returns_only_new() {
        let mut app = App::new();
        let feed_v1 = synthetic_feed("https://ex.com/feed.xml", &["A", "B"]);
        mark_feed_seen(&mut app, &feed_v1);

        let feed_v2 = synthetic_feed("https://ex.com/feed.xml", &["A", "B", "C", "D"]);
        let newly = mark_feed_seen(&mut app, &feed_v2);
        // Indices 2 and 3 (C, D) are new; A and B are already seen.
        assert_eq!(newly, vec![2, 3]);
        assert_eq!(app.seen_items.len(), 4);
    }

    #[test]
    fn test_mark_feed_seen_no_changes_returns_empty() {
        let mut app = App::new();
        let feed = synthetic_feed("https://ex.com/feed.xml", &["A", "B"]);
        mark_feed_seen(&mut app, &feed);
        let newly = mark_feed_seen(&mut app, &feed);
        assert!(newly.is_empty());
    }

    #[test]
    fn test_mark_feed_seen_distinct_feeds_independent() {
        let mut app = App::new();
        let feed_a = synthetic_feed("https://ex.com/a.xml", &["A1"]);
        let feed_b = synthetic_feed("https://ex.com/b.xml", &["B1"]);
        // First fetches both seed silently.
        assert!(mark_feed_seen(&mut app, &feed_a).is_empty());
        assert!(mark_feed_seen(&mut app, &feed_b).is_empty());
        assert!(app.feeds_seeded.contains("https://ex.com/a.xml"));
        assert!(app.feeds_seeded.contains("https://ex.com/b.xml"));
    }

    #[test]
    fn test_mark_feed_seen_empty_first_fetch_does_not_seed() {
        // Regression: a transiently-empty first fetch must not flip
        // feeds_seeded. If it did, the next non-empty fetch would be treated
        // as "subsequent" and fire exec_on_new for every backlog item — the
        // exact firehose the first-fetch suppression exists to prevent.
        let mut app = App::new();
        let empty = synthetic_feed("https://ex.com/feed.xml", &[]);

        let newly = mark_feed_seen(&mut app, &empty);
        assert!(newly.is_empty());
        assert!(
            !app.feeds_seeded.contains("https://ex.com/feed.xml"),
            "empty fetch must NOT mark feed as seeded"
        );

        // The next fetch — now with items — is still the first real fetch
        // and must seed silently.
        let with_items = synthetic_feed("https://ex.com/feed.xml", &["A", "B", "C"]);
        let newly = mark_feed_seen(&mut app, &with_items);
        assert!(
            newly.is_empty(),
            "first non-empty fetch must seed silently, not fire"
        );
        assert!(app.feeds_seeded.contains("https://ex.com/feed.xml"));
        assert_eq!(app.seen_items.len(), 3);
    }

    #[test]
    fn test_remove_current_feed_cleans_up_tracking_state() {
        // Regression: feed removal used to leave the URL in feeds_seeded and
        // every item ID in seen_items forever, growing the persisted JSON
        // file monotonically across feed churn.
        let mut app = App::new();
        let feed = synthetic_feed("https://ex.com/feed.xml", &["A", "B"]);
        let item_ids: Vec<String> = feed.items.iter().map(|i| make_item_id(&feed, i)).collect();

        // Seed the tracking state as if exec_on_new had run for this feed.
        mark_feed_seen(&mut app, &feed);
        let second = synthetic_feed("https://ex.com/feed.xml", &["A", "B", "C"]);
        mark_feed_seen(&mut app, &second);
        assert!(app.feeds_seeded.contains("https://ex.com/feed.xml"));
        for id in &item_ids {
            assert!(app.seen_items.contains(id), "precondition: {} seen", id);
        }

        // Install the feed and trigger removal via the public path.
        app.bookmarks.push("https://ex.com/feed.xml".to_string());
        app.feeds.push(second);
        app.selected_feed = Some(0);
        app.remove_current_feed().unwrap();

        assert!(
            !app.feeds_seeded.contains("https://ex.com/feed.xml"),
            "URL must be dropped from feeds_seeded"
        );
        for id in &item_ids {
            assert!(
                !app.seen_items.contains(id),
                "item id {} must be dropped from seen_items",
                id
            );
        }
    }

    #[test]
    fn test_make_item_id_prefers_link() {
        let mut feed = Feed {
            url: "https://ex.com/feed.xml".to_string(),
            title: "Example".to_string(),
            items: vec![],
            title_lower: "example".to_string(),
        };
        let item = FeedItem {
            title: "T".to_string(),
            link: Some("https://ex.com/a".to_string()),
            description: None,
            pub_date: None,
            author: None,
            formatted_date: None,
            parsed_date: None,
            plain_text: None,
            title_lower: "t".to_string(),
            media: Vec::new(),
            thumbnail: None,
        };
        feed.items.push(item.clone());
        assert_eq!(make_item_id(&feed, &item), "https://ex.com/a");
        let mut item2 = item.clone();
        item2.link = None;
        item2.title = "T2".to_string();
        assert_eq!(make_item_id(&feed, &item2), "https://ex.com/feed.xml_T2");
    }

    #[test]
    fn test_saved_data_seen_items_feeds_seeded_roundtrip() {
        // Verify that the new persistence fields survive a JSON round-trip
        // with the same shape the App expects.
        let mut seen_items = HashSet::new();
        seen_items.insert("https://ex.com/a".to_string());
        seen_items.insert("https://ex.com/feed.xml_T2".to_string());
        let mut feeds_seeded = HashSet::new();
        feeds_seeded.insert("https://ex.com/feed.xml".to_string());

        let original = SavedData {
            bookmarks: vec!["https://ex.com/feed.xml".to_string()],
            categories: vec![],
            read_items: HashSet::new(),
            starred_items: HashSet::new(),
            last_session_time: None,
            seen_items: seen_items.clone(),
            feeds_seeded: feeds_seeded.clone(),
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: SavedData = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.seen_items, seen_items);
        assert_eq!(parsed.feeds_seeded, feeds_seeded);
    }

    #[test]
    fn test_saved_data_back_compat_without_new_fields() {
        // An older feedr_data.json with no seen_items / feeds_seeded must
        // still deserialize (defaults to empty sets), so users who upgrade
        // mid-flight don't lose their bookmarks.
        let json = r#"{
            "bookmarks": ["https://ex.com/feed.xml"],
            "categories": [],
            "read_items": [],
            "starred_items": []
        }"#;
        let parsed: SavedData = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed.bookmarks,
            vec!["https://ex.com/feed.xml".to_string()]
        );
        assert!(parsed.seen_items.is_empty());
        assert!(parsed.feeds_seeded.is_empty());
    }

    fn dummy_extracted(text: &str) -> ExtractedArticle {
        ExtractedArticle {
            title: "T".to_string(),
            plain_text: text.to_string(),
            byline: None,
            site_name: None,
            source_url: "https://ex.com/a".to_string(),
        }
    }

    /// Look up the generation stamped on a `Pending` slot, so tests can
    /// echo it back through `record_extraction_result` the same way a
    /// real worker would.
    fn pending_generation(app: &App, id: &str) -> u64 {
        match app.extracted.get(id) {
            Some(ExtractionState::Pending { generation, .. }) => *generation,
            other => panic!("expected Pending slot for {}, got {:?}", id, other),
        }
    }

    #[test]
    fn test_request_extraction_creates_pending_and_enqueues() {
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        assert!(matches!(
            app.extracted.get("id1"),
            Some(ExtractionState::Pending { .. })
        ));
        assert_eq!(app.pending_extraction_requests.len(), 1);
    }

    #[test]
    fn test_request_extraction_stores_safe_redirects_flag() {
        let mut app = App::new();
        app.request_extraction(
            "id_auto".to_string(),
            "https://ex.com/a".to_string(),
            true,
            false,
        );
        app.request_extraction(
            "id_manual".to_string(),
            "https://ex.com/b".to_string(),
            false,
            false,
        );
        let q: Vec<_> = app.pending_extraction_requests.iter().collect();
        let auto = q.iter().find(|r| r.id == "id_auto").unwrap();
        let manual = q.iter().find(|r| r.id == "id_manual").unwrap();
        assert!(
            auto.safe_redirects,
            "auto-path request must carry safe_redirects=true"
        );
        assert!(
            !manual.safe_redirects,
            "manual-path request must carry safe_redirects=false"
        );
    }

    #[test]
    fn test_request_extraction_priority_front_jumps_queue() {
        // Auto requests go to the back; a manual (`priority_front=true`)
        // request must land at the head so an explicit Shift+F isn't
        // starved behind a refresh's auto-extraction batch.
        let mut app = App::new();
        for i in 0..3 {
            app.request_extraction(
                format!("auto{}", i),
                "https://ex.com/a".to_string(),
                true,
                false,
            );
        }
        app.request_extraction(
            "manual".to_string(),
            "https://ex.com/m".to_string(),
            false,
            true,
        );
        let order: Vec<&str> = app
            .pending_extraction_requests
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(
            order,
            vec!["manual", "auto0", "auto1", "auto2"],
            "manual request must jump to the front"
        );
    }

    #[test]
    fn test_request_extraction_idempotent_for_already_pending() {
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        // No double-queue: second call must be a no-op because state is Pending.
        assert_eq!(app.pending_extraction_requests.len(), 1);
    }

    #[test]
    fn test_record_extraction_result_transitions_to_ready() {
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let gen = pending_generation(&app, "id1");
        app.record_extraction_result("id1".to_string(), gen, Ok(dummy_extracted("body")));
        match app.extracted.get("id1") {
            Some(ExtractionState::Ready(a)) => assert_eq!(a.plain_text, "body"),
            other => panic!("expected Ready, got {:?}", other),
        }
    }

    #[test]
    fn test_record_extraction_result_transitions_to_failed() {
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let gen = pending_generation(&app, "id1");
        app.record_extraction_result("id1".to_string(), gen, Err(anyhow::anyhow!("boom")));
        match app.extracted.get("id1") {
            Some(ExtractionState::Failed(msg)) => assert!(msg.contains("boom")),
            other => panic!("expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn test_lru_evicts_oldest_at_cap() {
        let mut app = App::new();
        // Insert one more than the cap; the first id must be evicted. Real
        // callers always Pending → Ready, and `record_extraction_result`
        // refuses results for slots without a Pending predecessor, so do the
        // same here.
        for i in 0..EXTRACTED_CACHE_CAP + 1 {
            let id = format!("id{}", i);
            app.request_extraction(id.clone(), "https://ex.com/a".to_string(), false, false);
            let gen = pending_generation(&app, &id);
            app.record_extraction_result(id, gen, Ok(dummy_extracted("x")));
        }
        assert!(!app.extracted.contains_key("id0"), "oldest must be evicted");
        assert_eq!(app.extracted.len(), EXTRACTED_CACHE_CAP);
    }

    #[test]
    fn test_remove_current_feed_prunes_extracted_cache() {
        let mut app = make_test_app();
        // Seed extraction state for items in the first feed via the real
        // request → result transition.
        let id_old = app.get_item_id(0, 0);
        let id_new = app.get_item_id(0, 1);
        app.request_extraction(id_old.clone(), "https://ex.com/a".to_string(), false, false);
        let gen_old = pending_generation(&app, &id_old);
        app.record_extraction_result(id_old.clone(), gen_old, Ok(dummy_extracted("a")));
        app.request_extraction(id_new.clone(), "https://ex.com/b".to_string(), false, false);
        let gen_new = pending_generation(&app, &id_new);
        app.record_extraction_result(id_new.clone(), gen_new, Ok(dummy_extracted("b")));
        assert_eq!(app.extracted.len(), 2);

        // Remove the first feed.
        app.selected_feed = Some(0);
        app.remove_current_feed().ok();

        assert!(
            !app.extracted.contains_key(&id_old) && !app.extracted.contains_key(&id_new),
            "extracted entries for removed feed must be pruned"
        );
        assert!(
            app.extracted_order.is_empty(),
            "LRU order must be pruned in sync with the map"
        );
    }

    #[test]
    fn test_record_extraction_result_drops_late_result_for_evicted_id() {
        // Simulates the race where a worker thread completes after its slot
        // was evicted (or its feed was removed). We must not resurrect the
        // slot — otherwise dead state pins memory and confuses the UI.
        let mut app = App::new();
        // Any generation — slot doesn't exist, so result is dropped before
        // generation is even consulted.
        app.record_extraction_result("ghost".to_string(), 0, Ok(dummy_extracted("body")));
        assert!(
            !app.extracted.contains_key("ghost"),
            "result for an id with no Pending slot must be dropped"
        );
    }

    #[test]
    fn test_record_extraction_result_drops_late_result_for_ready_slot() {
        // If the slot is already `Ready` (e.g. user manually retried and the
        // retry landed first), a second late worker must not clobber it.
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let gen = pending_generation(&app, "id1");
        app.record_extraction_result("id1".to_string(), gen, Ok(dummy_extracted("first")));
        // Late second worker — slot is Ready, so this must be a no-op.
        // Whatever generation it claims is irrelevant; the state check fails.
        app.record_extraction_result("id1".to_string(), gen, Ok(dummy_extracted("late")));
        match app.extracted.get("id1") {
            Some(ExtractionState::Ready(a)) => {
                assert_eq!(a.plain_text, "first", "late result must not clobber Ready")
            }
            other => panic!("expected Ready, got {:?}", other),
        }
    }

    #[test]
    fn test_record_extraction_result_drops_stale_generation_on_re_requested_slot() {
        // The TOCTOU race the generation tag closes: a Pending slot is
        // evicted by LRU, then re-requested for the same id. The OLD
        // worker (still running) eventually delivers its result. Without
        // the generation check, that stale result would land on the new
        // Pending slot — silently satisfying a "manual retry" with a
        // stale fetch.
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let stale_gen = pending_generation(&app, "id1");
        // Evict + re-request manually (faster than filling 500 entries).
        app.remove_extraction("id1");
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let fresh_gen = pending_generation(&app, "id1");
        assert_ne!(
            stale_gen, fresh_gen,
            "re-requested slot must carry a fresh generation"
        );
        // Old worker lands a result tagged with the stale generation.
        app.record_extraction_result("id1".to_string(), stale_gen, Ok(dummy_extracted("stale")));
        // Slot must still be Pending — the stale result was discarded.
        assert!(
            matches!(
                app.extracted.get("id1"),
                Some(ExtractionState::Pending { generation: g, .. }) if *g == fresh_gen
            ),
            "stale-generation result must not satisfy the re-requested slot"
        );
    }

    #[test]
    fn test_prune_stale_pending_extractions_flips_old_pending_to_failed() {
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        // Forcibly age the Pending slot's started_at past the watchdog.
        // Mark it `spawned=true` so this test exercises the genuine
        // stuck-worker case (a worker that bumped inflight and never
        // released it) — that's what the watchdog is meant to recover.
        let id = "id1".to_string();
        let gen = pending_generation(&app, &id);
        let aged = Instant::now()
            .checked_sub(std::time::Duration::from_secs(3600))
            .expect("Instant::now() - 1h must be representable on a freshly-booted machine");
        app.extracted.insert(
            id.clone(),
            ExtractionState::Pending {
                generation: gen,
                started_at: aged,
                spawned: true,
            },
        );
        // 30 s × 3 = 90 s watchdog; aged 3600 s exceeds it.
        app.prune_stale_pending_extractions(std::time::Duration::from_secs(30));
        match app.extracted.get(&id) {
            Some(ExtractionState::Failed(msg)) => {
                assert!(
                    msg.contains("timed out"),
                    "expected timeout reason: {}",
                    msg
                )
            }
            other => panic!("expected Failed after watchdog, got {:?}", other),
        }
    }

    #[test]
    fn test_prune_returns_zero_for_aged_but_unspawned_pending() {
        // Regression test for the inflight-counter drift: a request that
        // aged out while still queued (rate-limited behind a long backlog)
        // has `spawned=false`, so the watchdog must NOT count it toward the
        // inflight-release total. Counting it would silently widen the
        // effective worker cap because no permit was ever claimed.
        let mut app = App::new();
        app.request_extraction(
            "stranded".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let gen = pending_generation(&app, "stranded");
        // Age the slot but leave `spawned=false` — the request never made
        // it through the spawn loop.
        let aged = Instant::now()
            .checked_sub(std::time::Duration::from_secs(3600))
            .expect("Instant::now() - 1h must be representable");
        app.extracted.insert(
            "stranded".to_string(),
            ExtractionState::Pending {
                generation: gen,
                started_at: aged,
                spawned: false,
            },
        );
        let pruned = app.prune_stale_pending_extractions(std::time::Duration::from_secs(30));
        assert_eq!(
            pruned, 0,
            "watchdog must return zero for queue-stranded prunes — no inflight permit was ever claimed"
        );
        // The slot still flips to Failed so the user can retry; only the
        // inflight-release count is gated on `spawned`.
        assert!(
            matches!(
                app.extracted.get("stranded"),
                Some(ExtractionState::Failed(_))
            ),
            "queue-stranded slot must still be flipped to Failed so a manual retry works"
        );
    }

    #[test]
    fn test_prune_counts_only_spawned_slots_when_mixed() {
        // Mixed batch: one spawned (real stuck worker) and one queue-stranded.
        // Only the spawned one should be counted toward inflight release.
        let mut app = App::new();
        app.request_extraction(
            "spawned".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        app.request_extraction(
            "queued".to_string(),
            "https://ex.com/b".to_string(),
            false,
            false,
        );
        let aged = Instant::now()
            .checked_sub(std::time::Duration::from_secs(3600))
            .expect("Instant::now() - 1h must be representable");
        let g1 = pending_generation(&app, "spawned");
        let g2 = pending_generation(&app, "queued");
        app.extracted.insert(
            "spawned".to_string(),
            ExtractionState::Pending {
                generation: g1,
                started_at: aged,
                spawned: true,
            },
        );
        app.extracted.insert(
            "queued".to_string(),
            ExtractionState::Pending {
                generation: g2,
                started_at: aged,
                spawned: false,
            },
        );
        let pruned = app.prune_stale_pending_extractions(std::time::Duration::from_secs(30));
        assert_eq!(pruned, 1, "only the spawned slot must be counted");
    }

    #[test]
    fn test_mark_extraction_spawned_flips_flag_on_matching_generation() {
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let gen = pending_generation(&app, "id1");
        // Before: spawned=false from request_extraction.
        assert!(matches!(
            app.extracted.get("id1"),
            Some(ExtractionState::Pending { spawned: false, .. })
        ));
        app.mark_extraction_spawned("id1", gen);
        assert!(matches!(
            app.extracted.get("id1"),
            Some(ExtractionState::Pending { spawned: true, .. })
        ));
    }

    #[test]
    fn test_mark_extraction_spawned_is_noop_on_generation_mismatch() {
        // If the slot was evicted and re-requested between spawn() and
        // mark_extraction_spawned, the old generation must not flip the
        // flag on the fresh slot. The stale worker's record_extraction_result
        // would also be dropped by the generation check, so leaving
        // spawned=false on the fresh slot is the right semantic.
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let stale_gen = pending_generation(&app, "id1");
        app.remove_extraction("id1");
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let fresh_gen = pending_generation(&app, "id1");
        assert_ne!(stale_gen, fresh_gen);
        app.mark_extraction_spawned("id1", stale_gen);
        assert!(matches!(
            app.extracted.get("id1"),
            Some(ExtractionState::Pending { spawned: false, .. })
        ));
    }

    #[test]
    fn test_prune_stale_pending_extractions_leaves_fresh_pending_alone() {
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        // Freshly Pending — well within the watchdog window.
        app.prune_stale_pending_extractions(std::time::Duration::from_secs(30));
        assert!(matches!(
            app.extracted.get("id1"),
            Some(ExtractionState::Pending { .. })
        ));
    }

    #[test]
    fn test_lru_hard_cap_when_oldest_is_pending() {
        // Stress the soft-cap edge case: queue CAP+1 requests as Pending
        // (no result returned). The cache size must stay at the cap, not
        // grow with the number of in-flight requests.
        let mut app = App::new();
        for i in 0..EXTRACTED_CACHE_CAP + 5 {
            let id = format!("id{}", i);
            app.request_extraction(id, "https://ex.com/a".to_string(), false, false);
        }
        assert_eq!(
            app.extracted.len(),
            EXTRACTED_CACHE_CAP,
            "Pending entries must not let the cache exceed the hard cap"
        );
        assert_eq!(app.extracted_order.len(), EXTRACTED_CACHE_CAP);
    }

    #[test]
    fn test_remove_extraction_helper_prunes_both_map_and_order() {
        let mut app = App::new();
        app.request_extraction(
            "id1".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let gen = pending_generation(&app, "id1");
        app.record_extraction_result("id1".to_string(), gen, Ok(dummy_extracted("body")));
        assert_eq!(app.extracted.len(), 1);
        assert_eq!(app.extracted_order.len(), 1);
        app.remove_extraction("id1");
        assert!(app.extracted.is_empty());
        assert!(app.extracted_order.is_empty());
    }

    #[test]
    fn test_sync_detail_focus_anchor_resets_show_extracted_on_focus_change() {
        // Regression test for the cross-article show_extracted bug: the
        // global toggle must reset to `true` whenever the detail view's
        // focused article changes, so a "view summary" choice on article A
        // doesn't silently carry over to article B.
        let mut app = make_test_app();
        app.view = View::FeedItemDetail;
        app.selected_feed = Some(0);
        app.selected_item = Some(0);
        // First sync — establishes the anchor at (0, 0).
        app.sync_detail_focus_anchor();
        assert!(app.show_extracted);
        assert_eq!(app.last_detail_focus, Some((0, 0)));

        // Simulate the user pressing Shift+F to toggle off on article (0, 0).
        app.show_extracted = false;
        // Re-sync on the same article — must NOT clobber the user's toggle.
        app.sync_detail_focus_anchor();
        assert!(
            !app.show_extracted,
            "anchor unchanged → user's toggle preserved"
        );

        // User navigates to article (0, 1). Sync must reset the toggle.
        app.selected_item = Some(1);
        app.sync_detail_focus_anchor();
        assert!(
            app.show_extracted,
            "focus changed → show_extracted must reset to true"
        );
        assert_eq!(app.last_detail_focus, Some((0, 1)));
    }

    #[test]
    fn test_sync_detail_focus_anchor_is_noop_when_focus_unchanged() {
        // Per-frame call must be idempotent — otherwise a toggled-off state
        // would be wiped on the very next frame and the toggle would feel
        // unresponsive.
        let mut app = make_test_app();
        app.view = View::FeedItemDetail;
        app.selected_feed = Some(0);
        app.selected_item = Some(0);
        app.sync_detail_focus_anchor();
        app.show_extracted = false;
        for _ in 0..10 {
            app.sync_detail_focus_anchor();
        }
        assert!(
            !app.show_extracted,
            "repeated syncs must not wipe the toggle"
        );
    }

    /// Regression lock for the LRU-vs-watchdog leak: evicting a Pending slot
    /// whose `spawned` flag is true would hide it from
    /// `prune_stale_pending_extractions` (which only iterates `extracted`),
    /// so a silently-dead worker's inflight permit would leak for the rest
    /// of the session. `insert_extraction` must skip such slots during
    /// eviction and evict the next-oldest evictable entry.
    #[test]
    fn test_lru_eviction_skips_spawned_pending_slots() {
        let mut app = App::new();
        // Seed one spawned-Pending slot at the head of the LRU. Real callers
        // would have inserted via `request_extraction` then flipped `spawned`
        // via `mark_extraction_spawned`; we do the same here.
        app.request_extraction(
            "spawned_head".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        let gen = pending_generation(&app, "spawned_head");
        app.mark_extraction_spawned("spawned_head", gen);
        // Fill the rest of the cap with Ready entries (insertion-order LRU
        // puts them all behind the spawned head).
        for i in 0..EXTRACTED_CACHE_CAP - 1 {
            let id = format!("ready{}", i);
            app.request_extraction(id.clone(), "https://ex.com/x".to_string(), false, false);
            let g = pending_generation(&app, &id);
            app.record_extraction_result(id, g, Ok(dummy_extracted("body")));
        }
        assert_eq!(app.extracted.len(), EXTRACTED_CACHE_CAP);

        // Insert one more entry — eviction must skip the spawned-Pending
        // head and instead evict the next-oldest (ready0).
        app.request_extraction(
            "newcomer".to_string(),
            "https://ex.com/n".to_string(),
            false,
            false,
        );
        let g = pending_generation(&app, "newcomer");
        app.record_extraction_result("newcomer".to_string(), g, Ok(dummy_extracted("body")));

        assert!(
            app.extracted.contains_key("spawned_head"),
            "spawned-Pending slot must NOT be evicted — would hide it from the watchdog"
        );
        assert!(
            !app.extracted.contains_key("ready0"),
            "the next-oldest evictable entry should have been evicted instead"
        );
        // Size still bounded — cap +/- the one we skipped (no extra growth
        // here because we only protected one slot and evicted one entry).
        assert_eq!(app.extracted.len(), EXTRACTED_CACHE_CAP);
    }

    /// Watchdog must NOT promote a stale Pending → Failed transition to the
    /// most-recently-used end of the LRU deque. A timed-out entry should be
    /// strictly less useful than newer Ready entries and evicted ahead of
    /// them — promoting it would invert that order.
    #[test]
    fn test_watchdog_failure_does_not_promote_lru_position() {
        let mut app = App::new();
        app.request_extraction(
            "first".to_string(),
            "https://ex.com/a".to_string(),
            false,
            false,
        );
        // Insert another entry so we can observe LRU order before/after.
        app.request_extraction(
            "second".to_string(),
            "https://ex.com/b".to_string(),
            false,
            false,
        );
        // Order should be [first, second] in the deque.
        let order_before: Vec<String> = app.extracted_order.iter().cloned().collect();
        assert_eq!(
            order_before,
            vec!["first".to_string(), "second".to_string()]
        );

        // Age `first` past the watchdog window and run the prune.
        let gen = pending_generation(&app, "first");
        let aged = Instant::now()
            .checked_sub(std::time::Duration::from_secs(3600))
            .expect("Instant::now() - 1h must be representable");
        app.extracted.insert(
            "first".to_string(),
            ExtractionState::Pending {
                generation: gen,
                started_at: aged,
                spawned: true,
            },
        );
        app.prune_stale_pending_extractions(std::time::Duration::from_secs(30));

        // State flipped to Failed, but LRU position preserved at the head.
        assert!(matches!(
            app.extracted.get("first"),
            Some(ExtractionState::Failed(_))
        ));
        let order_after: Vec<String> = app.extracted_order.iter().cloned().collect();
        assert_eq!(
            order_after,
            vec!["first".to_string(), "second".to_string()],
            "watchdog must NOT promote a Failed entry to MRU — would let it outlive newer Ready slots"
        );
    }

    // ── In-article search helpers ─────────────────────────────────────

    #[test]
    fn test_find_article_matches_empty_query_returns_empty() {
        assert!(find_article_matches("anything goes here", "").is_empty());
        assert!(find_article_matches("", "").is_empty());
    }

    #[test]
    fn test_find_article_matches_case_insensitive_when_query_all_lowercase() {
        let body = "Foo bar FOO baz foo";
        let matches = find_article_matches(body, "foo");
        assert_eq!(matches.len(), 3);
        // All on line 0; starts at byte offsets 0, 8, 16.
        assert_eq!(
            matches[0],
            ArticleMatch {
                line: 0,
                start: 0,
                end: 3
            }
        );
        assert_eq!(
            matches[1],
            ArticleMatch {
                line: 0,
                start: 8,
                end: 11
            }
        );
        assert_eq!(
            matches[2],
            ArticleMatch {
                line: 0,
                start: 16,
                end: 19
            }
        );
    }

    #[test]
    fn test_find_article_matches_case_sensitive_when_query_has_uppercase() {
        // Smart-case: "Foo" has uppercase → case-sensitive, so only the
        // first occurrence matches (the FOO and foo variants are skipped).
        let body = "Foo bar FOO baz foo";
        let matches = find_article_matches(body, "Foo");
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0],
            ArticleMatch {
                line: 0,
                start: 0,
                end: 3
            }
        );
    }

    #[test]
    fn test_find_article_matches_across_multiple_lines() {
        let body = "first line\nsecond foo line\nthird foo and foo again";
        let matches = find_article_matches(body, "foo");
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[0].start, 7);
        assert_eq!(matches[1].line, 2);
        assert_eq!(matches[1].start, 6);
        assert_eq!(matches[2].line, 2);
        assert_eq!(matches[2].start, 14);
    }

    #[test]
    fn test_find_article_matches_no_matches_returns_empty() {
        assert!(find_article_matches("hello world", "xyz").is_empty());
    }

    #[test]
    fn test_wrapped_row_of_line_zero_returns_zero() {
        let body = "one\ntwo\nthree";
        assert_eq!(wrapped_row_of_line(body, 0, 80), 0);
    }

    #[test]
    fn test_wrapped_row_of_line_short_lines_one_row_each() {
        let body = "one\ntwo\nthree\nfour";
        // Line 2 ("three") starts at row 2 — preceded by "one" (1 row) and
        // "two" (1 row).
        assert_eq!(wrapped_row_of_line(body, 2, 80), 2);
        assert_eq!(wrapped_row_of_line(body, 3, 80), 3);
    }

    #[test]
    fn test_wrapped_row_of_line_accounts_for_wrap() {
        // 12-char line in a 5-wide viewport wraps to ceil(12/5) = 3 rows.
        let body = "aaaaaaaaaaaa\nshort";
        assert_eq!(wrapped_row_of_line(body, 1, 5), 3);
    }

    #[test]
    fn test_wrapped_row_of_line_empty_width_returns_zero() {
        assert_eq!(wrapped_row_of_line("foo\nbar", 1, 0), 0);
    }

    #[test]
    fn test_next_article_match_wraps_and_advances() {
        let mut app = App::new();
        app.article_body_cache = "alpha\nbeta\nalpha".to_string();
        app.article_body_width_cache = 80;
        app.article_search_query = "alpha".to_string();
        app.article_search_matches =
            find_article_matches(&app.article_body_cache, &app.article_search_query);
        assert_eq!(app.article_search_matches.len(), 2);
        app.article_search_current = Some(0);

        app.next_article_match();
        assert_eq!(app.article_search_current, Some(1));
        // Wraps to 0 past the end.
        app.next_article_match();
        assert_eq!(app.article_search_current, Some(0));
    }

    #[test]
    fn test_prev_article_match_wraps_to_last() {
        let mut app = App::new();
        app.article_body_cache = "alpha\nalpha\nalpha".to_string();
        app.article_body_width_cache = 80;
        app.article_search_query = "alpha".to_string();
        app.article_search_matches =
            find_article_matches(&app.article_body_cache, &app.article_search_query);
        app.article_search_current = Some(0);

        app.prev_article_match();
        assert_eq!(app.article_search_current, Some(2));
        app.prev_article_match();
        assert_eq!(app.article_search_current, Some(1));
    }

    #[test]
    fn test_next_article_match_noop_when_empty() {
        let mut app = App::new();
        app.article_search_current = None;
        app.next_article_match();
        assert_eq!(app.article_search_current, None);
        app.prev_article_match();
        assert_eq!(app.article_search_current, None);
    }

    #[test]
    fn test_clear_article_search_resets_all_fields() {
        let mut app = App::new();
        app.article_search_query = "foo".to_string();
        app.article_search_current = Some(2);
        app.article_search_matches = vec![ArticleMatch {
            line: 0,
            start: 0,
            end: 3,
        }];
        app.article_body_cache = "stale body content".to_string();
        app.article_body_width_cache = 80;
        app.clear_article_search();
        assert!(app.article_search_query.is_empty());
        assert_eq!(app.article_search_current, None);
        assert!(app.article_search_matches.is_empty());
        // Body cache must be dropped too — otherwise a stray n/N after the
        // user clears (then re-typed something on a different article) could
        // scroll-target against the previous article's text.
        assert!(
            app.article_body_cache.is_empty(),
            "clear_article_search must drop the body cache"
        );
        assert_eq!(app.article_body_width_cache, 0);
    }

    #[test]
    fn test_exit_detail_view_clears_article_search() {
        let mut app = App::new();
        app.article_search_query = "foo".to_string();
        app.detail_vertical_scroll = 42;
        app.exit_detail_view(View::FeedItems);
        assert!(app.article_search_query.is_empty());
        assert_eq!(app.detail_vertical_scroll, 0);
        assert_eq!(app.view, View::FeedItems);
    }

    #[test]
    fn test_exit_detail_view_resets_input_mode() {
        // A user typing in the search footer when something else triggers
        // exit_detail_view (e.g. a macro/remote, or any future code path)
        // must not be stranded in `ArticleSearch` mode in a different view
        // that has no footer to surface their input.
        let mut app = App::new();
        app.input_mode = InputMode::ArticleSearch;
        app.article_search_query = "foo".to_string();
        app.exit_detail_view(View::FeedItems);
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_find_article_matches_non_ascii_case_insensitive_does_not_fold() {
        // INVARIANT: the ASCII fold leaves non-ASCII bytes untouched, so
        // an uppercase non-ASCII letter in the body cannot be matched by
        // the equivalent lowercase non-ASCII letter in the query. Here
        // body is "É" (U+00C9, bytes 0xC3 0x89) and query is "é" (U+00E9,
        // bytes 0xC3 0xA9): different bytes, no match. (Sanity-check the
        // ASCII path still folds in the same scenario — `C` vs `c` *does*
        // match because both bytes are in the A-Z fold table.)
        //
        // This test pins the v1 behavior so a future change to
        // Unicode-aware folding is intentional — `str::to_lowercase` is
        // NOT byte-length-preserving and would silently invalidate the
        // byte-offset-into-original-line model the highlight render relies
        // on. See the doc comment on `find_article_matches`.
        let matches = find_article_matches("É", "é");
        assert!(
            matches.is_empty(),
            "ASCII-fold smart-case must not match non-ASCII case pairs — \
             changing this requires redesigning the byte-offset model"
        );

        // ASCII counterpart — same shape (uppercase in body, lowercase in
        // query, no uppercase in query so case-insensitive mode applies) —
        // *does* match. Demonstrates the fold is doing its ASCII job; what
        // doesn't fold above is exclusively the non-ASCII range.
        let matches = find_article_matches("C", "c");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start, 0);
        assert_eq!(matches[0].end, 1);

        // And — to lock in the *positive* path for the non-ASCII byte we
        // care about — a query whose bytes already match the body matches
        // normally regardless of fold mode.
        let matches = find_article_matches("Café", "café");
        assert_eq!(
            matches.len(),
            1,
            "lowercase non-ASCII in both needle and folded haystack must \
             still match — only the cross-case pair is the gap"
        );
        assert_eq!(matches[0].start, 0);
        assert_eq!(matches[0].end, "Café".len());
    }

    #[test]
    fn test_find_article_matches_offsets_valid_with_multibyte_chars() {
        // Multi-byte char ("é" = 2 bytes in UTF-8) sits in the line before
        // the ASCII match. The reported byte offsets must land on UTF-8
        // boundaries in the *original* line, not just the folded one, so
        // that `&line[start..end]` slicing in `build_styled_body` is safe.
        let body = "café bar foo baz";
        // bytes: c(0) a(1) f(2) é(3..5) ' '(5) b(6) a(7) r(8) ' '(9) f(10) o(11) o(12)
        let matches = find_article_matches(body, "foo");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 0);
        assert_eq!(matches[0].start, 10);
        assert_eq!(matches[0].end, 13);

        // Sanity: slicing the original line at the reported offsets must
        // succeed and yield exactly "foo". If `to_ascii_lowercase` ever
        // disturbed multi-byte sequences (it doesn't, per docs) this would
        // panic with a UTF-8 boundary error.
        assert_eq!(&body[matches[0].start..matches[0].end], "foo");
    }

    #[test]
    fn test_find_article_matches_overlapping_needle_advances_past_match() {
        // Needle "aa" in "aaaa" yields 2 non-overlapping matches at [0..2]
        // and [2..4] — `scan_line` must step past `end` so we never report
        // [0..2], [1..3], [2..4] (which would render as nested highlights).
        let matches = find_article_matches("aaaa", "aa");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].start, 0);
        assert_eq!(matches[0].end, 2);
        assert_eq!(matches[1].start, 2);
        assert_eq!(matches[1].end, 4);
    }

    #[test]
    fn test_toggle_read_mode() {
        let mut app = App::new();
        assert!(!app.read_mode);
        app.toggle_read_mode();
        assert!(app.read_mode);
        app.toggle_read_mode();
        assert!(!app.read_mode);
    }

    #[test]
    fn test_exit_detail_view_resets_read_mode() {
        let mut app = App::new();
        app.view = View::FeedItemDetail;
        app.read_mode = true;
        app.exit_detail_view(View::FeedItems);
        assert!(!app.read_mode);
        assert_eq!(app.view, View::FeedItems);
    }
}

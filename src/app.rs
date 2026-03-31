use crate::config::{CompactMode, Config};
use crate::feed::{Feed, FeedCategory, FeedItem};
use crate::ui::ColorScheme;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

#[derive(Clone, Debug)]
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

#[derive(Serialize, Deserialize)]
struct SavedData {
    bookmarks: Vec<String>,
    categories: Vec<FeedCategory>,
    read_items: HashSet<String>,
    #[serde(default)]
    starred_items: HashSet<String>,
    #[serde(default)]
    last_session_time: Option<String>,
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

        // Parse last session time from saved data
        let last_session_time = saved_data
            .last_session_time
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let show_summary = last_session_time.is_some() && has_bookmarks;

        let keybindings = crate::keybindings::build_keybindings(&config.keybindings);

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
            preview_pane: false,
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
        };

        app.update_dashboard();
        app.rebuild_feed_tree();

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
                if let Some(link) = &item.link {
                    return link.clone();
                }
                return format!("{}_{}", feed.url, item.title);
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

    /// Exit the detail view and reset scroll position
    pub fn exit_detail_view(&mut self, new_view: View) {
        self.detail_vertical_scroll = 0;
        self.view = new_view;
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

        // Check per-feed intervals
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
    fn extract_domain_from_url(url: &str) -> String {
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
        feeds_with_counts.sort_by(|a, b| b.1.cmp(&a.1));

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
        assert!(!app.preview_pane);
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
}

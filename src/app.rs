use crate::feed::{Feed, FeedItem};
use crate::ui::extract_domain;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct FilterOptions {
    pub category: Option<String>,  // Filter by feed category
    pub age: Option<TimeFilter>,   // Filter by content age
    pub has_author: Option<bool>,  // Filter for items with/without author
    pub read_status: Option<bool>, // Filter for read/unread items
    pub min_length: Option<usize>, // Filter by content length
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
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug)]
pub enum InputMode {
    Normal,
    InsertUrl,
    SearchMode,
    FilterMode,
}

#[derive(Clone, Debug)]
pub enum View {
    Dashboard,
    FeedList,
    FeedItems,
    FeedItemDetail,
}

#[derive(Clone, Debug)]
pub struct App {
    pub feeds: Vec<Feed>,
    pub bookmarks: Vec<String>,
    pub input: String,
    pub input_mode: InputMode,
    pub selected_feed: Option<usize>,
    pub selected_item: Option<usize>,
    pub view: View,
    pub error: Option<String>,
    pub search_query: String,
    pub is_searching: bool,
    pub filtered_items: Vec<(usize, usize)>, // (feed_idx, item_idx) for search results
    pub dashboard_items: Vec<(usize, usize)>, // (feed_idx, item_idx) for dashboard
    pub is_loading: bool,                    // Flag to indicate loading/refreshing state
    pub loading_indicator: usize,            // For animated loading indicator
    pub filter_options: FilterOptions,
    pub filter_mode: bool,       // Whether we're in filter selection mode
    pub read_items: Vec<String>, // Track read item IDs
    pub filtered_dashboard_items: Vec<(usize, usize)>, // Filtered items for dashboard
}

#[derive(Serialize, Deserialize)]
struct SavedData {
    bookmarks: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        let bookmarks = Self::load_bookmarks().unwrap_or_default();
        let read_items = Self::load_read_items().unwrap_or_default();
        let mut app = Self {
            feeds: Vec::new(),
            bookmarks,
            input: String::new(),
            input_mode: InputMode::Normal,
            selected_feed: None,
            selected_item: None,
            view: View::Dashboard,
            error: None,
            search_query: String::new(),
            is_searching: false,
            filtered_items: Vec::new(),
            dashboard_items: Vec::new(),
            is_loading: false,
            loading_indicator: 0,
            filter_options: FilterOptions::new(),
            filter_mode: false,
            read_items,
            filtered_dashboard_items: Vec::new(),
        };

        // Load bookmarked feeds
        app.load_bookmarked_feeds();
        app.update_dashboard();

        app
    }

    pub fn load_bookmarked_feeds(&mut self) {
        self.feeds.clear();
        for url in &self.bookmarks {
            match Feed::from_url(url) {
                Ok(feed) => self.feeds.push(feed),
                Err(_) => { /* Skip failed feeds */ }
            }
        }
    }

    fn load_read_items() -> Result<Vec<String>> {
        let path = Self::read_items_path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let data = fs::read_to_string(path)?;
        let items: Vec<String> = serde_json::from_str(&data)?;
        Ok(items)
    }

    fn save_read_items(&self) -> Result<()> {
        let path = Self::read_items_path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        let json = serde_json::to_string(&self.read_items)?;
        fs::write(path, json)?;
        Ok(())
    }

    fn read_items_path() -> std::path::PathBuf {
        let mut path = dirs::data_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
        path.push("feedr");
        path.push("read_items.json");
        path
    }

    pub fn apply_filters(&mut self) {
        // First update the dashboard items normally
        if !self.filter_options.is_active() {
            // No filters active, so filtered items are the same as dashboard items
            self.filtered_dashboard_items = self.dashboard_items.clone();
            return;
        }

        self.filtered_dashboard_items = self
            .dashboard_items
            .iter()
            .filter(|&&(feed_idx, item_idx)| self.item_matches_filter(feed_idx, item_idx))
            .cloned()
            .collect();
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
        if let Some(category) = &self.filter_options.category {
            // Use feed URL to infer category
            let feed_domain = extract_domain(&feed.url);
            if !feed_domain.contains(category) {
                return false;
            }
        }

        // Check age filter
        if let Some(age_filter) = &self.filter_options.age {
            if let Some(date_str) = &item.pub_date {
                if let Ok(date) = DateTime::parse_from_rfc2822(date_str) {
                    let now = Utc::now();
                    let duration = now.signed_duration_since(date.with_timezone(&Utc));

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
                    // Can't parse date, so filter out if age filter is active
                    return false;
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

        // Check content length filter
        if let Some(min_length) = self.filter_options.min_length {
            if let Some(desc) = &item.description {
                let plain_text = html2text::from_read(desc.as_bytes(), 80);
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
    fn get_item_id(&self, feed_idx: usize, item_idx: usize) -> String {
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
        if !item_id.is_empty() && !self.read_items.contains(&item_id) {
            self.read_items.push(item_id);
            self.save_read_items()?;
        }
        Ok(())
    }

    // Check if an item is read
    pub fn is_item_read(&self, feed_idx: usize, item_idx: usize) -> bool {
        let item_id = self.get_item_id(feed_idx, item_idx);
        self.read_items.contains(&item_id)
    }

    pub fn update_dashboard(&mut self) {
        self.dashboard_items.clear();

        // Collect all items from all feeds with their indices
        let mut all_items = Vec::new();
        for (feed_idx, feed) in self.feeds.iter().enumerate() {
            for (item_idx, item) in feed.items.iter().enumerate() {
                if let Some(date_str) = &item.pub_date {
                    if let Ok(date) = DateTime::parse_from_rfc2822(date_str) {
                        all_items.push((feed_idx, item_idx, date.with_timezone(&Utc)));
                    } else {
                        // If we can't parse the date, still include but with a "oldest" date
                        all_items.push((feed_idx, item_idx, Utc::now()));
                    }
                } else {
                    // If there's no date, still include but with a "oldest" date
                    all_items.push((feed_idx, item_idx, Utc::now()));
                }
            }
        }

        // Sort by date (newest first)
        all_items.sort_by(|a, b| b.2.cmp(&a.2));

        // Take the 20 newest items
        self.dashboard_items = all_items
            .into_iter()
            .take(20)
            .map(|(feed_idx, item_idx, _)| (feed_idx, item_idx))
            .collect();

        self.filtered_dashboard_items = if !self.filter_options.is_active() {
            self.dashboard_items.clone()
        } else {
            self.dashboard_items
                .iter()
                .filter(|&&(feed_idx, item_idx)| self.item_matches_filter(feed_idx, item_idx))
                .cloned()
                .collect()
        };
    }

    pub fn add_feed(&mut self, url: &str) -> Result<()> {
        let feed = Feed::from_url(url)?;
        if !self.bookmarks.contains(&url.to_string()) {
            self.bookmarks.push(url.to_string());
            self.save_bookmarks()?;
        }
        self.feeds.push(feed);
        self.update_dashboard();
        Ok(())
    }

    pub fn remove_current_feed(&mut self) -> Result<()> {
        if let Some(idx) = self.selected_feed {
            if idx < self.feeds.len() {
                let url = self.feeds[idx].url.clone();
                self.feeds.remove(idx);
                self.bookmarks.retain(|b| b != &url);
                self.save_bookmarks()?;
                if self.feeds.is_empty() {
                    self.selected_feed = None;
                } else if idx >= self.feeds.len() {
                    self.selected_feed = Some(self.feeds.len() - 1);
                }
                self.update_dashboard();
            }
        }
        Ok(())
    }

    fn load_bookmarks() -> Result<Vec<String>> {
        let path = Self::bookmarks_path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let data = fs::read_to_string(path)?;
        let saved: SavedData = serde_json::from_str(&data)?;
        Ok(saved.bookmarks)
    }

    fn save_bookmarks(&self) -> Result<()> {
        let path = Self::bookmarks_path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        let data = SavedData {
            bookmarks: self.bookmarks.clone(),
        };
        let json = serde_json::to_string_pretty(&data)?;
        fs::write(path, json)?;
        Ok(())
    }

    fn bookmarks_path() -> std::path::PathBuf {
        let mut path = dirs::data_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
        path.push("feedr");
        path.push("bookmarks.json");
        path
    }

    pub fn current_feed(&self) -> Option<&Feed> {
        self.selected_feed.and_then(|idx| self.feeds.get(idx))
    }

    pub fn current_item(&self) -> Option<&FeedItem> {
        self.current_feed()
            .and_then(|feed| self.selected_item.and_then(|idx| feed.items.get(idx)))
    }

    pub fn dashboard_item(&self, idx: usize) -> Option<(&Feed, &FeedItem)> {
        if idx < self.dashboard_items.len() {
            let (feed_idx, item_idx) = self.dashboard_items[idx];
            if let Some(feed) = self.feeds.get(feed_idx) {
                if let Some(item) = feed.items.get(item_idx) {
                    return Some((feed, item));
                }
            }
        }
        None
    }

    pub fn open_current_item_in_browser(&self) -> Result<()> {
        if let Some(item) = self.current_item() {
            if let Some(link) = &item.link {
                open::that(link)?;
            }
        }
        Ok(())
    }

    pub fn search_feeds(&mut self, query: &str) {
        self.search_query = query.to_lowercase();
        self.is_searching = !query.is_empty();

        if !self.is_searching {
            return;
        }

        self.filtered_items.clear();
        for (feed_idx, feed) in self.feeds.iter().enumerate() {
            if feed.title.to_lowercase().contains(&self.search_query) {
                // Add all items from matching feed
                for item_idx in 0..feed.items.len() {
                    self.filtered_items.push((feed_idx, item_idx));
                }
            } else {
                // Check individual items
                for (item_idx, item) in feed.items.iter().enumerate() {
                    if item.title.to_lowercase().contains(&self.search_query)
                        || item
                            .description
                            .as_ref()
                            .map_or(false, |d| d.to_lowercase().contains(&self.search_query))
                    {
                        self.filtered_items.push((feed_idx, item_idx));
                    }
                }
            }
        }
    }

    pub fn search_item(&self, idx: usize) -> Option<(&Feed, &FeedItem)> {
        if idx < self.filtered_items.len() {
            let (feed_idx, item_idx) = self.filtered_items[idx];
            if let Some(feed) = self.feeds.get(feed_idx) {
                if let Some(item) = feed.items.get(item_idx) {
                    return Some((feed, item));
                }
            }
        }
        None
    }

    pub fn refresh_feeds(&mut self) -> Result<()> {
        self.is_loading = true;

        // Instead of threading, we'll do a synchronous refresh
        // but track the loading state to show the animation
        let urls = self.bookmarks.clone();
        self.feeds.clear();

        for url in &urls {
            match Feed::from_url(url) {
                Ok(feed) => self.feeds.push(feed),
                Err(e) => self.error = Some(format!("Failed to refresh feed {}: {}", url, e)),
            }
        }

        self.update_dashboard();
        self.is_loading = false;

        Ok(())
    }

    pub fn update_loading_indicator(&mut self) {
        self.loading_indicator = (self.loading_indicator + 1) % 10;
    }

    pub fn get_available_categories(&self) -> Vec<String> {
        // Extract potential categories from feed domains
        let mut categories = std::collections::HashSet::new();

        for feed in &self.feeds {
            let domain = extract_domain(&feed.url);

            // Try to extract a category from the domain
            if domain.contains("news") || domain.contains("nytimes") || domain.contains("cnn") {
                categories.insert("news".to_string());
            } else if domain.contains("tech")
                || domain.contains("wired")
                || domain.contains("ycombinator")
            {
                categories.insert("tech".to_string());
            } else if domain.contains("science")
                || domain.contains("nature")
                || domain.contains("scientific")
            {
                categories.insert("science".to_string());
            } else if domain.contains("finance")
                || domain.contains("money")
                || domain.contains("business")
            {
                categories.insert("finance".to_string());
            } else if domain.contains("sport")
                || domain.contains("espn")
                || domain.contains("athletic")
            {
                categories.insert("sports".to_string());
            } else {
                // Use the first part of the domain as a fallback category
                if let Some(first_part) = domain.split('.').next() {
                    categories.insert(first_part.to_string());
                }
            }
        }

        let mut result: Vec<String> = categories.into_iter().collect();
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

        if parts.is_empty() {
            "No filters active".to_string()
        } else {
            parts.join(" | ")
        }
    }
}

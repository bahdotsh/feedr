use crate::feed::{Feed, FeedCategory, FeedItem};
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

#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    InsertUrl,
    SearchMode,
    FilterMode,
    CategoryNameInput, // For creating/renaming categories
}

#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Dashboard,
    FeedList,
    FeedItems,
    FeedItemDetail,
    CategoryManagement,
}

#[derive(Clone, Debug)]
pub struct App {
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
    pub category_action: Option<CategoryAction>, // For category management
    pub detail_vertical_scroll: u16, // Vertical scroll value for item detail view
    pub detail_max_scroll: u16,  // Maximum scroll value for current content
}

#[derive(Clone, Debug)]
pub enum CategoryAction {
    Create,
    Rename(usize),
    AddFeedToCategory(String), // Feed URL to add
}

#[derive(Serialize, Deserialize)]
struct SavedData {
    bookmarks: Vec<String>,
    categories: Vec<FeedCategory>,
    read_items: Vec<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let saved_data = Self::load_saved_data().unwrap_or_else(|_| SavedData {
            bookmarks: vec![],
            categories: vec![],
            read_items: vec![],
        });

        let mut app = Self {
            feeds: Vec::new(),
            bookmarks: saved_data.bookmarks,
            categories: saved_data.categories,
            selected_category: None,
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
            read_items: saved_data.read_items,
            filtered_dashboard_items: Vec::new(),
            category_action: None,
            detail_vertical_scroll: 0,
            detail_max_scroll: 0,
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

    fn load_saved_data() -> Result<SavedData> {
        let path = Self::data_path();
        if !path.exists() {
            return Ok(SavedData {
                bookmarks: Vec::new(),
                categories: Vec::new(),
                read_items: Vec::new(),
            });
        }

        let data = fs::read_to_string(path)?;
        let saved_data: SavedData = serde_json::from_str(&data)?;
        Ok(saved_data)
    }

    fn save_data(&self) -> Result<()> {
        let path = Self::data_path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        let saved_data = SavedData {
            bookmarks: self.bookmarks.clone(),
            categories: self.categories.clone(),
            read_items: self.read_items.clone(),
        };

        let json = serde_json::to_string(&saved_data)?;
        fs::write(path, json)?;
        Ok(())
    }

    fn data_path() -> std::path::PathBuf {
        let mut path = dirs::data_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
        path.push("feedr");
        path.push("feedr_data.json");
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
            self.save_data()?;
        }
        Ok(())
    }

    // Check if an item is read
    pub fn is_item_read(&self, feed_idx: usize, item_idx: usize) -> bool {
        let item_id = self.get_item_id(feed_idx, item_idx);
        self.read_items.contains(&item_id)
    }

    pub fn update_dashboard(&mut self) {
        // Clear existing dashboard items
        self.dashboard_items.clear();

        // Get all feeds and sort by most recent first
        let mut all_items = Vec::new();

        for (feed_idx, feed) in self.feeds.iter().enumerate() {
            for (item_idx, item) in feed.items.iter().enumerate() {
                let date = item
                    .pub_date
                    .as_ref()
                    .and_then(|date_str| DateTime::parse_from_rfc2822(date_str).ok());

                all_items.push((feed_idx, item_idx, date));
            }
        }

        // Sort by date (most recent first)
        all_items.sort_by(|a, b| match (&a.2, &b.2) {
            (Some(a_date), Some(b_date)) => b_date.cmp(a_date),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        // Add items to dashboard (limited to most recent 100 for performance)
        for (feed_idx, item_idx, _) in all_items.into_iter().take(100) {
            self.dashboard_items.push((feed_idx, item_idx));
        }

        // Apply any active filters
        self.apply_filters();
    }

    pub fn add_feed(&mut self, url: &str) -> Result<()> {
        let feed = Feed::from_url(url)?;
        self.feeds.push(feed);
        if !self.bookmarks.contains(&url.to_string()) {
            self.bookmarks.push(url.to_string());
        }
        self.update_dashboard();

        // Save data
        self.save_data()?;

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
                            .is_some_and(|d| d.to_lowercase().contains(&self.search_query))
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

        Ok(())
    }

    pub fn remove_feed_from_category(&mut self, feed_url: &str, category_idx: usize) -> Result<()> {
        if category_idx >= self.categories.len() {
            return Err(anyhow::anyhow!("Invalid category index"));
        }

        let removed = self.categories[category_idx].remove_feed(feed_url);
        if removed {
            self.save_data()?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Feed not found in category"))
        }
    }

    pub fn toggle_category_expanded(&mut self, idx: usize) -> Result<()> {
        if idx < self.categories.len() {
            self.categories[idx].toggle_expanded();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Invalid category index"))
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

    /// Exit the detail view and reset scroll position
    pub fn exit_detail_view(&mut self, new_view: View) {
        self.detail_vertical_scroll = 0;
        self.view = new_view;
    }
}

use crate::feed::{Feed, FeedItem};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub enum InputMode {
    Normal,
    InsertUrl,
    SearchMode,
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
}

#[derive(Serialize, Deserialize)]
struct SavedData {
    bookmarks: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        let bookmarks = Self::load_bookmarks().unwrap_or_default();
        let mut app = Self {
            feeds: Vec::new(),
            bookmarks,
            input: String::new(),
            input_mode: InputMode::Normal,
            selected_feed: None,
            selected_item: None,
            view: View::Dashboard, // Start with dashboard view
            error: None,
            search_query: String::new(),
            is_searching: false,
            filtered_items: Vec::new(),
            dashboard_items: Vec::new(),
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
        let urls = self.bookmarks.clone();
        self.feeds.clear();

        for url in &urls {
            match Feed::from_url(url) {
                Ok(feed) => self.feeds.push(feed),
                Err(e) => self.error = Some(format!("Failed to refresh feed {}: {}", url, e)),
            }
        }

        self.update_dashboard();
        Ok(())
    }
}

use crate::feed::{Feed, FeedItem};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub enum InputMode {
    Normal,
    InsertUrl,
}

#[derive(Clone, Debug)]
pub enum View {
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
            view: View::FeedList,
            error: None,
        };

        // Load bookmarked feeds
        app.load_bookmarked_feeds();

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

    pub fn add_feed(&mut self, url: &str) -> Result<()> {
        let feed = Feed::from_url(url)?;
        if !self.bookmarks.contains(&url.to_string()) {
            self.bookmarks.push(url.to_string());
            self.save_bookmarks()?;
        }
        self.feeds.push(feed);
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
}

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rss::{Channel, Item};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feed {
    pub url: String,
    pub title: String,
    pub items: Vec<FeedItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeedItem {
    pub title: String,
    pub link: Option<String>,
    pub description: Option<String>,
    pub pub_date: Option<String>,
    pub author: Option<String>,
    pub formatted_date: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedCategory {
    pub id: String,
    pub name: String,
    pub feeds: HashSet<String>, // URLs of feeds in this category, using HashSet for faster lookup
    pub expanded: bool,         // UI state: whether the category is expanded in the UI
}

impl FeedCategory {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            feeds: HashSet::new(),
            expanded: true,
        }
    }

    pub fn add_feed(&mut self, url: &str) {
        self.feeds.insert(url.to_string());
    }

    pub fn remove_feed(&mut self, url: &str) -> bool {
        self.feeds.remove(url)
    }

    pub fn contains_feed(&self, url: &str) -> bool {
        self.feeds.contains(url)
    }

    pub fn is_empty(&self) -> bool {
        self.feeds.is_empty()
    }

    pub fn feed_count(&self) -> usize {
        self.feeds.len()
    }

    pub fn rename(&mut self, new_name: &str) {
        self.name = new_name.to_string();
    }

    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }
}

impl Feed {
    pub fn from_url(url: &str) -> Result<Self> {
        let content = reqwest::blocking::Client::new()
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .context("Failed to fetch feed")?
            .bytes()
            .context("Failed to read response body")?;

        let channel = Channel::read_from(&content[..]).context("Failed to parse RSS feed")?;

        let items = channel
            .items()
            .iter()
            .map(|item| FeedItem::from_rss_item(item))
            .collect();

        Ok(Feed {
            url: url.to_string(),
            title: channel.title().to_string(),
            items,
        })
    }
}

impl FeedItem {
    fn from_rss_item(item: &Item) -> Self {
        // Format the date for better display
        let formatted_date = item.pub_date().and_then(|date_str| {
            DateTime::parse_from_rfc2822(date_str)
                .ok()
                .map(|dt| format_date(dt.with_timezone(&Utc)))
        });

        FeedItem {
            title: item.title().unwrap_or("Untitled").to_string(),
            // Using map for Option<&str> to Option<String> conversion
            link: item.link().map(ToString::to_string),
            description: item.description().map(ToString::to_string),
            pub_date: item.pub_date().map(ToString::to_string),
            author: item.author().map(ToString::to_string),
            formatted_date,
        }
    }
}

fn format_date(dt: DateTime<Utc>) -> String {
    // Calculate how long ago the item was published
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);

    if diff.num_minutes() < 60 {
        format!("{} minutes ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{} hours ago", diff.num_hours())
    } else if diff.num_days() < 7 {
        format!("{} days ago", diff.num_days())
    } else {
        // For older items, show the actual date
        dt.format("%B %d, %Y").to_string()
    }
}

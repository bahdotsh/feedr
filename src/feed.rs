use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use feed_rs::parser;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
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
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(Duration::from_secs(15))
            .build()
            .context("Failed to create HTTP client")?;

        let response = client
            .get(url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (compatible; Feedr/1.0; +https://github.com/bahdotsh/feedr)",
            )
            .header(
                "Accept",
                "application/rss+xml, application/atom+xml, application/xml, text/xml, */*",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "gzip, deflate")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .send()
            .context("Failed to fetch feed")?;

        // Check if we got redirected or have an unusual status
        let final_url = response.url().clone();
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|ct| ct.to_str().ok())
            .unwrap_or("unknown")
            .to_lowercase();

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "HTTP error {}: Failed to fetch feed from {}",
                status,
                url
            ));
        }

        let content = response.bytes().context("Failed to read response body")?;

        // Debug: Check if we got HTML instead of XML
        if content.len() < 100 {
            return Err(anyhow::anyhow!(
                "Response too short ({} bytes), might be empty or an error page",
                content.len()
            ));
        }

        let content_start = String::from_utf8_lossy(&content[..std::cmp::min(200, content.len())]);
        if content_start.trim_start().starts_with("<!DOCTYPE html")
            || content_start.trim_start().starts_with("<html")
        {
            return Err(anyhow::anyhow!(
                "Received HTML page instead of RSS/Atom feed. URL might be incorrect or require authentication. Final URL: {}",
                final_url
            ));
        }

        let feed = parser::parse(&content[..])
            .with_context(|| {
                let content_preview = String::from_utf8_lossy(&content[..std::cmp::min(300, content.len())]);
                format!(
                    "Failed to parse feed (RSS/Atom) from URL: {} (final URL: {}, {} bytes, content-type: {}, preview: {})",
                    url, final_url, content.len(), content_type, content_preview.trim()
                )
            })?;

        let items = feed.entries.iter().map(FeedItem::from_feed_entry).collect();

        Ok(Feed {
            url: url.to_string(),
            title: feed
                .title
                .map(|t| t.content)
                .unwrap_or_else(|| "Untitled Feed".to_string()),
            items,
        })
    }
}

impl FeedItem {
    fn from_feed_entry(entry: &feed_rs::model::Entry) -> Self {
        // Extract publication date - try multiple date formats
        let (pub_date_string, formatted_date) = if let Some(published) = &entry.published {
            let pub_string = published.to_rfc3339();
            let formatted = format_date(*published);
            (Some(pub_string), Some(formatted))
        } else if let Some(updated) = &entry.updated {
            let pub_string = updated.to_rfc3339();
            let formatted = format_date(*updated);
            (Some(pub_string), Some(formatted))
        } else {
            (None, None)
        };

        // Extract author information
        let author = entry.authors.first().map(|author| {
            if !author.name.is_empty() {
                author.name.clone()
            } else if let Some(email) = &author.email {
                email.clone()
            } else {
                "Unknown".to_string()
            }
        });

        // Extract content/description - prefer content over summary
        let description = if let Some(content) = entry.content.as_ref() {
            Some(content.body.clone().unwrap_or_default())
        } else if let Some(summary) = entry.summary.as_ref() {
            Some(summary.content.clone())
        } else {
            None
        };

        // Extract the primary link
        let link = entry.links.first().map(|link| link.href.clone());

        FeedItem {
            title: entry
                .title
                .as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_else(|| "Untitled".to_string()),
            link,
            description,
            pub_date: pub_date_string,
            author,
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

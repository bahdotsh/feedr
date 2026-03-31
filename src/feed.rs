use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use feed_rs::parser;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feed {
    pub url: String,
    pub title: String,
    pub items: Vec<FeedItem>,
    #[serde(skip)]
    pub title_lower: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeedItem {
    pub title: String,
    pub link: Option<String>,
    pub description: Option<String>,
    pub pub_date: Option<String>,
    pub author: Option<String>,
    pub formatted_date: Option<String>,
    #[serde(skip)]
    pub parsed_date: Option<DateTime<Utc>>,
    #[serde(skip)]
    pub plain_text: Option<String>,
    #[serde(skip)]
    pub title_lower: String,
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

#[derive(Clone, Debug)]
pub struct DiscoveredFeed {
    pub url: String,
    pub title: String,
    pub feed_type: String, // "RSS" or "Atom"
}

#[derive(Debug, Clone)]
pub struct HtmlWithFeedsError {
    pub discovered: Vec<DiscoveredFeed>,
    pub page_url: String,
}

impl std::fmt::Display for HtmlWithFeedsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.discovered.is_empty() {
            write!(
                f,
                "No RSS/Atom feed links found on this page. URL: {}",
                self.page_url
            )
        } else {
            write!(
                f,
                "HTML page with {} feed link(s) found",
                self.discovered.len()
            )
        }
    }
}

impl std::error::Error for HtmlWithFeedsError {}

pub fn discover_feeds_from_html(html: &[u8], base_url: &Url) -> Vec<DiscoveredFeed> {
    let html_str = String::from_utf8_lossy(html);
    let document = Html::parse_document(&html_str);
    let selector = Selector::parse("link[rel=alternate]").unwrap();

    let mut seen_urls = HashSet::new();
    let mut feeds = Vec::new();

    for element in document.select(&selector) {
        let link_type = match element.value().attr("type") {
            Some(t) => t.to_lowercase(),
            None => continue,
        };

        let feed_type = if link_type == "application/rss+xml" {
            "RSS"
        } else if link_type == "application/atom+xml" {
            "Atom"
        } else {
            continue;
        };

        let href = match element.value().attr("href") {
            Some(h) if !h.is_empty() => h,
            _ => continue,
        };

        let resolved = match base_url.join(href) {
            Ok(u) => u.to_string(),
            Err(_) => continue,
        };

        if !seen_urls.insert(resolved.clone()) {
            continue;
        }

        let title = element
            .value()
            .attr("title")
            .filter(|t| !t.is_empty())
            .unwrap_or(&resolved)
            .to_string();

        feeds.push(DiscoveredFeed {
            url: resolved,
            title,
            feed_type: feed_type.to_string(),
        });
    }

    feeds
}

impl Feed {
    /// Fetch and parse a feed from a URL with default timeout
    pub fn from_url(url: &str) -> Result<Self> {
        Self::from_url_with_config(url, 15, None, None)
    }

    /// Fetch and parse a feed from a URL with custom timeout
    pub fn from_url_with_timeout(url: &str, timeout_secs: u64) -> Result<Self> {
        Self::from_url_with_config(url, timeout_secs, None, None)
    }

    /// Fetch and parse a feed from a URL with custom timeout and user agent
    pub fn from_url_with_config(
        url: &str,
        timeout_secs: u64,
        user_agent: Option<&str>,
        custom_headers: Option<&HashMap<String, String>>,
    ) -> Result<Self> {
        let client = Self::build_client(timeout_secs)?;
        Self::from_url_with_client(url, &client, user_agent, custom_headers)
    }

    /// Build a shared HTTP client with the given timeout
    pub fn build_client(timeout_secs: u64) -> Result<reqwest::blocking::Client> {
        reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("Failed to create HTTP client")
    }

    /// Fetch and parse a feed from a URL using a pre-built client
    pub fn from_url_with_client(
        url: &str,
        client: &reqwest::blocking::Client,
        user_agent: Option<&str>,
        custom_headers: Option<&HashMap<String, String>>,
    ) -> Result<Self> {
        let default_user_agent =
            "Mozilla/5.0 (compatible; Feedr/1.0; +https://github.com/bahdotsh/feedr)";
        let ua = user_agent.unwrap_or(default_user_agent);

        let mut request = client
            .get(url)
            .header("User-Agent", ua)
            .header(
                "Accept",
                "application/rss+xml, application/atom+xml, application/xml, text/xml, */*",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "gzip, deflate")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive");

        if let Some(headers) = custom_headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }

        let response = request.send().context("Failed to fetch feed")?;

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
        let trimmed_lower = content_start.trim_start().to_lowercase();
        if content_type.contains("text/html")
            || trimmed_lower.starts_with("<!doctype html")
            || trimmed_lower.starts_with("<html")
        {
            let discovered = discover_feeds_from_html(&content, &final_url);
            return Err(HtmlWithFeedsError {
                discovered,
                page_url: final_url.to_string(),
            }
            .into());
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

        let title = feed
            .title
            .map(|t| t.content)
            .unwrap_or_else(|| "Untitled Feed".to_string());
        let title_lower = title.to_lowercase();

        Ok(Feed {
            url: url.to_string(),
            title,
            items,
            title_lower,
        })
    }
}

impl FeedItem {
    fn from_feed_entry(entry: &feed_rs::model::Entry) -> Self {
        // Extract publication date - try multiple date formats
        let (pub_date_string, formatted_date, parsed_date) =
            if let Some(published) = &entry.published {
                let pub_string = published.to_rfc3339();
                let formatted = format_date(*published);
                (Some(pub_string), Some(formatted), Some(*published))
            } else if let Some(updated) = &entry.updated {
                let pub_string = updated.to_rfc3339();
                let formatted = format_date(*updated);
                (Some(pub_string), Some(formatted), Some(*updated))
            } else {
                (None, None, None)
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
        } else {
            entry
                .summary
                .as_ref()
                .map(|summary| summary.content.clone())
        };

        // Cache plain text from description (avoids repeated HTML parsing)
        let plain_text = description
            .as_ref()
            .map(|desc| html2text::from_read(desc.as_bytes(), 80));

        // Extract the primary link
        let link = entry.links.first().map(|link| link.href.clone());

        let title = entry
            .title
            .as_ref()
            .map(|t| t.content.clone())
            .unwrap_or_else(|| "Untitled".to_string());
        let title_lower = title.to_lowercase();

        FeedItem {
            title,
            link,
            description,
            pub_date: pub_date_string,
            author,
            formatted_date,
            parsed_date,
            plain_text,
            title_lower,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_single_rss_feed() {
        let html = br#"<html><head>
            <link rel="alternate" type="application/rss+xml" title="My Blog" href="/feed.xml">
        </head><body></body></html>"#;
        let base = Url::parse("https://example.com/blog/").unwrap();
        let feeds = discover_feeds_from_html(html, &base);
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].url, "https://example.com/feed.xml");
        assert_eq!(feeds[0].title, "My Blog");
        assert_eq!(feeds[0].feed_type, "RSS");
    }

    #[test]
    fn test_discover_multiple_feeds() {
        let html = br#"<html><head>
            <link rel="alternate" type="application/rss+xml" title="RSS Feed" href="/rss.xml">
            <link rel="alternate" type="application/atom+xml" title="Atom Feed" href="/atom.xml">
        </head><body></body></html>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let feeds = discover_feeds_from_html(html, &base);
        assert_eq!(feeds.len(), 2);
        assert_eq!(feeds[0].feed_type, "RSS");
        assert_eq!(feeds[1].feed_type, "Atom");
    }

    #[test]
    fn test_discover_no_feeds() {
        let html = br#"<html><head><title>No feeds</title></head><body></body></html>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let feeds = discover_feeds_from_html(html, &base);
        assert!(feeds.is_empty());
    }

    #[test]
    fn test_discover_relative_url_resolution() {
        let html = br#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="feed.xml">
        </head><body></body></html>"#;
        let base = Url::parse("https://example.com/blog/page").unwrap();
        let feeds = discover_feeds_from_html(html, &base);
        assert_eq!(feeds[0].url, "https://example.com/blog/feed.xml");
    }

    #[test]
    fn test_discover_absolute_url_preserved() {
        let html = br#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="https://feeds.example.com/rss">
        </head><body></body></html>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let feeds = discover_feeds_from_html(html, &base);
        assert_eq!(feeds[0].url, "https://feeds.example.com/rss");
    }

    #[test]
    fn test_discover_missing_title_falls_back_to_url() {
        let html = br#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="/feed.xml">
        </head><body></body></html>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let feeds = discover_feeds_from_html(html, &base);
        assert_eq!(feeds[0].title, "https://example.com/feed.xml");
    }

    #[test]
    fn test_discover_deduplicates() {
        let html = br#"<html><head>
            <link rel="alternate" type="application/rss+xml" title="Feed" href="/feed.xml">
            <link rel="alternate" type="application/rss+xml" title="Same Feed" href="/feed.xml">
        </head><body></body></html>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let feeds = discover_feeds_from_html(html, &base);
        assert_eq!(feeds.len(), 1);
    }

    #[test]
    fn test_discover_case_insensitive_type() {
        let html = br#"<html><head>
            <link rel="alternate" type="Application/RSS+XML" href="/feed.xml">
        </head><body></body></html>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let feeds = discover_feeds_from_html(html, &base);
        assert_eq!(feeds.len(), 1);
    }

    #[test]
    fn test_discover_lowercase_doctype() {
        let html = br#"<!doctype html><html><head>
            <link rel="alternate" type="application/rss+xml" title="Feed" href="/feed.xml">
        </head><body></body></html>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let feeds = discover_feeds_from_html(html, &base);
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].url, "https://example.com/feed.xml");
    }
}

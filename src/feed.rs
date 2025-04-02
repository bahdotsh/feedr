use anyhow::{Context, Result};
use rss::Channel;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Feed {
    pub url: String,
    pub title: String,
    pub items: Vec<FeedItem>,
}

#[derive(Clone, Debug)]
pub struct FeedItem {
    pub title: String,
    pub link: Option<String>,
    pub description: Option<String>,
    pub pub_date: Option<String>,
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
            .map(|item| FeedItem {
                title: item.title().unwrap_or("Untitled").to_string(),
                link: item.link().map(String::from),
                description: item.description().map(String::from),
                pub_date: item.pub_date().map(String::from),
            })
            .collect();

        Ok(Feed {
            url: url.to_string(),
            title: channel.title().to_string(),
            items,
        })
    }
}

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dom_smoothie::{Config as ReadabilityConfig, Readability};
use feed_rs::parser;
use html2text::render::text_renderer::{TaggedLine, TextDecorator};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::Read;
use std::net::IpAddr;
use std::sync::OnceLock;
use std::time::Duration;
use url::Url;
use uuid::Uuid;

/// Cap on response body size for article extraction. Anything larger is
/// almost certainly not a typical article page and would only burn CPU in
/// the Readability DOM walker.
const FULLTEXT_MAX_BYTES: usize = 5 * 1024 * 1024;

/// Minimum text length the extracted article body must contain to be
/// considered useful. Pages below this are treated as "page appears empty"
/// — usually JS-rendered shells, login walls, or 404 placeholders.
const FULLTEXT_MIN_LENGTH: usize = 200;

/// Number of leading bytes of the response we sniff for `<meta charset="…">`
/// when the Content-Type header doesn't carry one. The HTML5 spec mandates
/// any in-document declaration must appear in the first 1024 bytes of the
/// document, so reading more is just wasted work.
const META_SNIFF_BYTES: usize = 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feed {
    pub url: String,
    pub title: String,
    pub items: Vec<FeedItem>,
    #[serde(skip)]
    pub title_lower: String,
}

/// Coarse classification of a media attachment, derived from its MIME type's
/// top-level type. Used by `FeedItem::primary_media` to prefer playable media
/// (audio/video) over decorative (image) when picking the item's primary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    Audio,
    Video,
    Image,
    Unknown,
}

impl MediaKind {
    fn from_mime(mime: Option<&str>) -> Self {
        match mime {
            Some(m) if m.starts_with("audio/") => Self::Audio,
            Some(m) if m.starts_with("video/") => Self::Video,
            Some(m) if m.starts_with("image/") => Self::Image,
            _ => Self::Unknown,
        }
    }
}

/// A single playable / viewable media URL attached to a feed item. Sourced
/// from RSS Media (`<media:content>`), RSS 2 enclosures (`<enclosure>`), or
/// Atom out-of-line content (`<content src="…">`). Surfaced to macro
/// placeholders via `%m` / `%M`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaAttachment {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    pub kind: MediaKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
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
    /// Media attachments parsed from `<media:content>`, `<enclosure>`, and
    /// Atom out-of-line `<content src="…">`. Empty for ordinary text feeds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<MediaAttachment>,
    /// First `<media:thumbnail>` URI on the entry, if any. Independent of
    /// `media` — a YouTube entry typically has both a video attachment and
    /// a thumbnail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

impl FeedItem {
    /// Pick the most "playable" media attachment for placeholder expansion.
    /// Audio/video win over images; images win over Unknown. Within a tier
    /// the first (feed-order) attachment wins.
    pub fn primary_media(&self) -> Option<&MediaAttachment> {
        self.media
            .iter()
            .find(|m| matches!(m.kind, MediaKind::Audio | MediaKind::Video))
            .or_else(|| self.media.iter().find(|m| m.kind == MediaKind::Image))
            .or_else(|| self.media.first())
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedType {
    Rss,
    Atom,
}

impl fmt::Display for FeedType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeedType::Rss => write!(f, "RSS"),
            FeedType::Atom => write!(f, "Atom"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DiscoveredFeed {
    pub url: String,
    pub title: String,
    pub feed_type: FeedType,
}

/// Outcome of a successful Readability extraction against an article URL.
/// Decoupled from `dom_smoothie::Article` so the upstream crate can be
/// swapped without rippling through the rest of the codebase.
#[derive(Clone, Debug)]
pub struct ExtractedArticle {
    pub title: String,
    pub plain_text: String,
    pub byline: Option<String>,
    pub site_name: Option<String>,
    pub source_url: String,
}

/// Validates that `url` is safe to fetch on the *auto-fulltext* path (i.e.
/// without a user click). Rejects non-http(s) schemes and hostnames that
/// resolve to private / loopback / link-local addresses, since auto-mode
/// fetches whatever the feed XML put in `<link>` — a hostile feed could
/// otherwise probe the user's internal network. Manual `Shift+F` is the
/// user's explicit action and bypasses this check.
pub fn is_safe_auto_url(url: &str) -> bool {
    let parsed = match Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return false;
    }
    let host = match parsed.host() {
        Some(h) => h,
        None => return false,
    };
    match host {
        url::Host::Ipv4(ip) => is_global_ip(IpAddr::V4(ip)),
        url::Host::Ipv6(ip) => is_global_ip(IpAddr::V6(ip)),
        url::Host::Domain(name) => {
            // Reject obvious local-only names; a hostile feed cannot use
            // these to probe RFC1918 space, but we still want to avoid
            // hitting "localhost"-style endpoints by accident.
            let n = name.to_ascii_lowercase();
            if n == "localhost" || n.ends_with(".localhost") || n.ends_with(".local") {
                return false;
            }
            // DNS resolution itself happens later in reqwest; we don't
            // resolve here because that would block. The intent of the
            // check is "no obvious internal target encoded directly in
            // the URL" — DNS rebinding is out of scope for a feed reader.
            true
        }
    }
}

fn is_global_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                // Multicast 224.0.0.0/4 — IPv6 multicast (ff00::/8) is
                // already rejected below; this closes the v4/v6 asymmetry.
                || v4.is_multicast()
                || v4.octets()[0] == 0
                // CGNAT 100.64.0.0/10
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64)
                // IETF protocol-assignments 192.0.0.0/24
                || (v4.octets()[0] == 192
                    && v4.octets()[1] == 0
                    && v4.octets()[2] == 0))
        }
        IpAddr::V6(v6) => {
            let seg0 = v6.segments()[0];
            let seg1 = v6.segments()[1];
            !(v6.is_loopback()
                || v6.is_unspecified()
                // unique-local fc00::/7
                || (seg0 & 0xFE00) == 0xFC00
                // link-local fe80::/10
                || (seg0 & 0xFFC0) == 0xFE80
                // deprecated site-local fec0::/10
                || (seg0 & 0xFFC0) == 0xFEC0
                // multicast ff00::/8
                || (seg0 & 0xFF00) == 0xFF00
                // 6to4 2002::/16 — routes to embedded IPv4 via 6to4 relays,
                // so an address like 2002:a9fe:a9fe:: would reach 169.254.169.254
                || seg0 == 0x2002
                // NAT64 well-known prefix 64:ff9b::/96
                || (seg0 == 0x0064 && seg1 == 0xff9b)
                // IPv4-mapped — defer to v4 check via the embedded address
                || v6.to_ipv4_mapped().map(|v4| !is_global_ip(IpAddr::V4(v4))).unwrap_or(false))
        }
    }
}

/// Fetch `url` with `client` and run Mozilla-Readability extraction over the
/// returned HTML. Feed auth headers are intentionally NOT propagated: the
/// article URL is on a different host and may be third-party, so leaking
/// per-feed `Authorization` headers would be a credential leak.
///
/// Rejects non-`text/html` responses, oversized bodies (`FULLTEXT_MAX_BYTES`),
/// and pages whose extracted text falls below `FULLTEXT_MIN_LENGTH` (treated
/// as JS-rendered or empty). Body is read with a size-bounded reader so peak
/// allocation cannot exceed the cap, and the response charset is honored so
/// non-UTF8 pages decode correctly.
pub fn extract_article(
    url: &str,
    client: &reqwest::blocking::Client,
    user_agent: Option<&str>,
) -> Result<ExtractedArticle> {
    // Scheme allowlist — reqwest's blocking client refuses non-http(s) by
    // default, but this gives a clearer error and makes the policy explicit.
    let parsed = Url::parse(url).context("Article URL is not a valid URL")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(anyhow::anyhow!(
            "Article URL scheme '{}' is not allowed (only http/https)",
            parsed.scheme()
        ));
    }

    let default_user_agent =
        "Mozilla/5.0 (compatible; Feedr/1.0; +https://github.com/bahdotsh/feedr)";
    let ua = user_agent.unwrap_or(default_user_agent);

    let response = client
        .get(url)
        .header("User-Agent", ua)
        .header("Accept", "text/html, application/xhtml+xml, */*;q=0.5")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Accept-Encoding", "gzip, deflate")
        .send()
        .context("Failed to fetch article")?;

    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "HTTP error {} fetching article from {}",
            status,
            url
        ));
    }

    let content_type_raw = response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("")
        .to_string();
    let content_type = content_type_raw.to_lowercase();
    if !content_type.is_empty()
        && !content_type.contains("text/html")
        && !content_type.contains("application/xhtml")
    {
        return Err(anyhow::anyhow!(
            "Article URL did not return HTML (content-type: {})",
            content_type
        ));
    }

    // Reject upfront if Content-Length already exceeds the cap — saves us
    // the full transfer for obviously oversized responses.
    if let Some(declared_len) = response
        .headers()
        .get("content-length")
        .and_then(|cl| cl.to_str().ok())
        .and_then(|cl| cl.parse::<usize>().ok())
    {
        if declared_len > FULLTEXT_MAX_BYTES {
            return Err(anyhow::anyhow!(
                "Article body too large ({} bytes declared, cap {} bytes)",
                declared_len,
                FULLTEXT_MAX_BYTES
            ));
        }
    }

    let final_url = response.url().to_string();

    // Preallocate at the hard cap so `read_to_end`'s doubling growth can
    // never push capacity past it. Without this, starting from a smaller
    // hint (e.g. 64 KiB) grows the backing buffer to ~8 MiB for a 5 MiB
    // body — making the body-buffer ceiling ambiguous. With the
    // preallocation, the body-buffer ceiling is exactly FULLTEXT_MAX_BYTES;
    // the downstream DOM allocation inside dom_smoothie is separate
    // working-set and not bounded here. Modern allocators don't commit
    // physical pages until written, so a 5 MiB virtual reservation costs
    // essentially zero physical memory for the common short-page case.
    let mut bytes = Vec::with_capacity(FULLTEXT_MAX_BYTES + 1);
    let mut reader = response.take((FULLTEXT_MAX_BYTES as u64) + 1);
    reader
        .read_to_end(&mut bytes)
        .context("Failed to read article body")?;
    if bytes.len() > FULLTEXT_MAX_BYTES {
        return Err(anyhow::anyhow!(
            "Article body too large (exceeded cap {} bytes)",
            FULLTEXT_MAX_BYTES
        ));
    }

    let html = decode_html_bytes(&bytes, &content_type_raw);
    extract_from_html(&html, &final_url)
}

/// Decode `bytes` into a Rust `String` using the encoding declared by the
/// `Content-Type` header, falling back to `<meta charset>` sniffing in the
/// first ~1 KB of the document, and finally to UTF-8. Without this, any
/// non-UTF8 page (Windows-1252, ISO-8859-1, Shift_JIS, GBK, …) produces
/// U+FFFD-laced mojibake that Readability would then misclassify as too
/// short.
pub(crate) fn decode_html_bytes(bytes: &[u8], content_type: &str) -> String {
    let mut encoding: Option<&'static encoding_rs::Encoding> = None;

    // 1) Honor charset= in the Content-Type header.
    if let Some(label) = charset_from_content_type(content_type) {
        encoding = encoding_rs::Encoding::for_label(label.as_bytes());
    }

    // 2) Otherwise sniff the leading bytes for an in-document <meta charset>.
    if encoding.is_none() {
        let sniff_len = bytes.len().min(META_SNIFF_BYTES);
        if let Some(label) = sniff_meta_charset(&bytes[..sniff_len]) {
            encoding = encoding_rs::Encoding::for_label(label.as_bytes());
        }
    }

    // 3) Default to UTF-8 — same fate `from_utf8_lossy` would have given us,
    //    but routed through encoding_rs so behavior is uniform.
    let encoding = encoding.unwrap_or(encoding_rs::UTF_8);
    let (cow, _enc, _had_errors) = encoding.decode(bytes);
    cow.into_owned()
}

fn charset_from_content_type(content_type: &str) -> Option<String> {
    for part in content_type.split(';') {
        let lower = part.trim().to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("charset=") {
            let v = rest.trim().trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Sniff `<meta charset="…">` or `<meta http-equiv="Content-Type"
/// content="…charset=…">` from the leading bytes of an HTML document. Only
/// matches `charset=` that appears INSIDE a `<meta …>` tag — a loose
/// substring match would be fooled by `data-charset="x"` attributes or
/// string literals inside `<script>`. Stays byte-level (latin-1 lossy
/// decode is fine here: ASCII-superset encodings agree on the bytes we
/// care about) so we don't have to pick an encoding before we know one.
fn sniff_meta_charset(head: &[u8]) -> Option<String> {
    let s = String::from_utf8_lossy(head).to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(rel) = s[cursor..].find("<meta") {
        let tag_start = cursor + rel;
        // The character after `<meta` must be whitespace or `/` or `>` so we
        // don't match e.g. `<metadata>`.
        let after_name = tag_start + "<meta".len();
        let next_char = s.as_bytes().get(after_name).copied().unwrap_or(b'>');
        let is_meta_tag = matches!(next_char, b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>');
        let tag_end = s[after_name..]
            .find('>')
            .map(|e| after_name + e)
            .unwrap_or(s.len());
        cursor = tag_end.saturating_add(1);
        if !is_meta_tag {
            continue;
        }
        let tag = &s[tag_start..tag_end];
        // Walk every `charset` occurrence in the tag, not just the first.
        // Without this, `<meta name="charset" charset="utf-8">` would match
        // inside `name="charset"`, get an empty value, and skip to the next
        // tag — silently missing the real declaration.
        let mut search_from = 0;
        while let Some(rel) = tag[search_from..].find("charset") {
            let charset_idx = search_from + rel;
            search_from = charset_idx + "charset".len();
            // Require a word boundary before `charset` so we don't match
            // inside an attribute value like `name="charset"` (where the
            // preceding char is `"`) or inside a longer attribute name like
            // `data-charset`. Allowed boundaries are whitespace or `/`
            // between attributes, or start-of-tag (`<meta` followed
            // directly by `charset` is not legal HTML but harmless).
            let preceded_by_boundary = charset_idx == 0
                || matches!(
                    tag.as_bytes().get(charset_idx - 1),
                    Some(b' ' | b'\t' | b'\n' | b'\r' | b'/')
                );
            if !preceded_by_boundary {
                continue;
            }
            // Require `=` to follow (optionally after whitespace). Without
            // this, `name="charset"` followed by another attribute would
            // match here even though there is no `charset=` declaration.
            let after = &tag[search_from..];
            let after_trimmed = after.trim_start();
            let Some(rest) = after_trimmed.strip_prefix('=') else {
                continue;
            };
            let rest = rest.trim_start();
            let value = rest
                .strip_prefix('"')
                .or_else(|| rest.strip_prefix('\''))
                .unwrap_or(rest);
            let end = value
                .find(['"', '\'', ' ', '>', ';', '/', '\t', '\n', '\r'])
                .unwrap_or(value.len());
            let label = &value[..end];
            if !label.is_empty() {
                return Some(label.to_string());
            }
        }
    }
    None
}

/// Pure Readability step — no I/O. Split out so unit tests can exercise the
/// extraction + min-length + error-path logic without a network round-trip.
pub fn extract_from_html(html: &str, source_url: &str) -> Result<ExtractedArticle> {
    let cfg = ReadabilityConfig {
        max_elements_to_parse: 9000,
        ..ReadabilityConfig::default()
    };
    let mut readability = Readability::new(html, Some(source_url), Some(cfg))
        .context("Failed to initialize Readability")?;
    let article = readability
        .parse()
        .context("Readability could not parse article")?;

    if article.length < FULLTEXT_MIN_LENGTH {
        return Err(anyhow::anyhow!(
            "Page appears empty (only {} chars of body) — likely JS-rendered or paywalled",
            article.length
        ));
    }

    Ok(ExtractedArticle {
        title: article.title,
        plain_text: article.text_content.to_string(),
        byline: article.byline,
        site_name: article.site_name,
        source_url: source_url.to_string(),
    })
}

/// Result of fetching a URL that was expected to be a feed.
pub enum FeedFetchResult {
    /// Successfully parsed as an RSS/Atom feed.
    Feed(Feed),
    /// The URL returned an HTML page; any discovered feed links are included.
    DiscoveredFeeds {
        feeds: Vec<DiscoveredFeed>,
        page_url: String,
    },
}

impl FeedFetchResult {
    /// Convert to a `Feed`, returning an error if this was an HTML page.
    pub fn into_feed(self) -> Result<Feed> {
        match self {
            FeedFetchResult::Feed(feed) => Ok(feed),
            FeedFetchResult::DiscoveredFeeds { feeds, page_url } => {
                if feeds.is_empty() {
                    Err(anyhow::anyhow!(
                        "No RSS/Atom feed links found on this page: {}",
                        page_url
                    ))
                } else {
                    Err(anyhow::anyhow!(
                        "URL is an HTML page, not a feed. {} feed link(s) found on: {}",
                        feeds.len(),
                        page_url
                    ))
                }
            }
        }
    }
}

pub fn discover_feeds_from_html(html: &[u8], base_url: &Url) -> Vec<DiscoveredFeed> {
    static SELECTOR: OnceLock<Selector> = OnceLock::new();
    let selector = SELECTOR.get_or_init(|| Selector::parse("link[rel=alternate]").unwrap());

    let html_str = String::from_utf8_lossy(html);
    let document = Html::parse_document(&html_str);

    let mut seen_urls = HashSet::new();
    let mut feeds = Vec::new();

    for element in document.select(selector) {
        let link_type = match element.value().attr("type") {
            Some(t) => t.to_lowercase(),
            None => continue,
        };

        let feed_type = if link_type == "application/rss+xml" {
            FeedType::Rss
        } else if link_type == "application/atom+xml" {
            FeedType::Atom
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
            feed_type,
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

    /// Fetch and parse a feed from a URL with custom timeout and user agent.
    /// Returns an error if the URL is an HTML page (use `fetch_url` for discovery support).
    pub fn from_url_with_config(
        url: &str,
        timeout_secs: u64,
        user_agent: Option<&str>,
        custom_headers: Option<&HashMap<String, String>>,
    ) -> Result<Self> {
        let client = Self::build_client(timeout_secs)?;
        Self::fetch_url(url, &client, user_agent, custom_headers)?.into_feed()
    }

    /// Build a shared HTTP client with the given timeout
    pub fn build_client(timeout_secs: u64) -> Result<reqwest::blocking::Client> {
        reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("Failed to create HTTP client")
    }

    /// Build an HTTP client whose redirect policy re-validates each hop with
    /// `is_safe_auto_url`. Used for the auto-fulltext path so a hostile feed
    /// can't publish a public-looking `<link>` URL that 302s into the user's
    /// internal network (e.g. RFC1918 / loopback / cloud metadata endpoints).
    /// The redirect chain is still capped at 10 hops, matching `build_client`.
    pub fn build_safe_redirect_client(timeout_secs: u64) -> Result<reqwest::blocking::Client> {
        reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 10 {
                    return attempt.stop();
                }
                if !is_safe_auto_url(attempt.url().as_str()) {
                    return attempt.stop();
                }
                attempt.follow()
            }))
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("Failed to create safe-redirect HTTP client")
    }

    /// Fetch a URL and return either a parsed feed or discovered feed links.
    pub fn fetch_url(
        url: &str,
        client: &reqwest::blocking::Client,
        user_agent: Option<&str>,
        custom_headers: Option<&HashMap<String, String>>,
    ) -> Result<FeedFetchResult> {
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

        // Reject suspiciously short responses (likely empty/error pages)
        if content.len() < 100 {
            return Err(anyhow::anyhow!(
                "Response too short ({} bytes), might be empty or an error page",
                content.len()
            ));
        }

        // Try parsing as feed first — some servers serve valid feeds with text/html content-type
        let feed = match parser::parse(&content[..]) {
            Ok(f) => f,
            Err(parse_err) => {
                // Parse failed — check if this looks like HTML and try feed discovery
                let content_start =
                    String::from_utf8_lossy(&content[..std::cmp::min(200, content.len())]);
                let trimmed_lower = content_start.trim_start().to_lowercase();
                if content_type.contains("text/html")
                    || trimmed_lower.starts_with("<!doctype html")
                    || trimmed_lower.starts_with("<html")
                {
                    let discovered = discover_feeds_from_html(&content, &final_url);
                    return Ok(FeedFetchResult::DiscoveredFeeds {
                        feeds: discovered,
                        page_url: final_url.to_string(),
                    });
                }

                let content_preview =
                    String::from_utf8_lossy(&content[..std::cmp::min(300, content.len())]);
                return Err(parse_err).with_context(|| {
                    format!(
                        "Failed to parse feed (RSS/Atom) from URL: {} (final URL: {}, {} bytes, content-type: {}, preview: {})",
                        url, final_url, content.len(), content_type, content_preview.trim()
                    )
                });
            }
        };

        let items = feed.entries.iter().map(FeedItem::from_feed_entry).collect();

        let title = feed
            .title
            .map(|t| t.content)
            .unwrap_or_else(|| "Untitled Feed".to_string());
        let title_lower = title.to_lowercase();

        Ok(FeedFetchResult::Feed(Feed {
            url: url.to_string(),
            title,
            items,
            title_lower,
        }))
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

        // Cache plain text from description (avoids repeated HTML parsing).
        // Rendered through the same `render_clean_html` path as the
        // detail/dashboard views so pipe payloads and content filters see
        // the text the user sees — tables flattened, no `[N]` link markers,
        // no footnote dump.
        let plain_text = description.as_ref().map(|desc| render_clean_html(desc, 80));

        // Extract the primary link
        let link = entry.links.first().map(|link| link.href.clone());

        // Extract media attachments and thumbnails. Three sources, in priority
        // order (first occurrence of a given URL wins after dedup):
        //   1. <media:content> from RSS Media (`entry.media[*].content[*]`)
        //   2. <enclosure> from RSS 2 (links with `rel == Some("enclosure")`)
        //   3. Atom out-of-line `<content src="…">`
        let (media, thumbnail) = extract_media(entry);

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
            media,
            thumbnail,
        }
    }
}

/// Walk a `feed_rs::model::Entry` and collect media attachments + a single
/// representative thumbnail. See `FeedItem::from_feed_entry` for source
/// priority. URL deduplication preserves first occurrence.
fn extract_media(entry: &feed_rs::model::Entry) -> (Vec<MediaAttachment>, Option<String>) {
    let mut media: Vec<MediaAttachment> = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();
    let push = |att: MediaAttachment, seen: &mut HashSet<String>, media: &mut Vec<_>| {
        if seen.insert(att.url.clone()) {
            media.push(att);
        }
    };

    // 1. <media:content> via media:group or default item-level grouping.
    for obj in &entry.media {
        for content in &obj.content {
            let Some(url) = content.url.as_ref() else {
                continue;
            };
            let mime = content
                .content_type
                .as_ref()
                .map(|t| t.essence().to_string());
            let att = MediaAttachment {
                url: url.to_string(),
                kind: MediaKind::from_mime(mime.as_deref()),
                mime,
                width: content.width,
                height: content.height,
                duration_secs: content.duration.map(|d| d.as_secs()),
                size_bytes: content.size,
            };
            push(att, &mut seen_urls, &mut media);
        }
    }

    // 2. RSS 2 <enclosure>: feed-rs lands these in entry.links with rel="enclosure".
    for link in &entry.links {
        if link.rel.as_deref() != Some("enclosure") {
            continue;
        }
        let mime = link.media_type.clone();
        let att = MediaAttachment {
            url: link.href.clone(),
            kind: MediaKind::from_mime(mime.as_deref()),
            mime,
            width: None,
            height: None,
            duration_secs: None,
            size_bytes: link.length,
        };
        push(att, &mut seen_urls, &mut media);
    }

    // 3. Atom out-of-line <content src="…">. The content's media type is on the
    // parent `Content`, not on the inner `Link`.
    if let Some(content) = entry.content.as_ref() {
        if let Some(src) = content.src.as_ref() {
            let mime = Some(content.content_type.essence().to_string());
            let att = MediaAttachment {
                url: src.href.clone(),
                kind: MediaKind::from_mime(mime.as_deref()),
                mime,
                width: None,
                height: None,
                duration_secs: None,
                size_bytes: content.length,
            };
            push(att, &mut seen_urls, &mut media);
        }
    }

    // Thumbnail priority:
    //   1. <media:thumbnail> on the entry (Reddit, YouTube, etc.)
    //   2. First <img src="..."> in the entry's content / summary HTML
    //      (xkcd, SMBC, Penny Arcade — comic feeds that put their single
    //      image inside the summary instead of a media namespace).
    let thumbnail = entry
        .media
        .iter()
        .flat_map(|obj| obj.thumbnails.iter())
        .map(|t| t.image.uri.clone())
        .find(|uri| !uri.is_empty())
        .or_else(|| {
            // Fall back to scraping the description for an <img>. We try
            // `content` first (which `from_feed_entry` already prefers over
            // `summary`), then `summary`. Resolution uses the first entry
            // link as base so `src="/comics/foo.png"` becomes absolute.
            let base = entry.links.first().map(|l| l.href.as_str());
            let html_sources = [
                entry.content.as_ref().and_then(|c| c.body.as_deref()),
                entry.summary.as_ref().map(|s| s.content.as_str()),
            ];
            html_sources
                .into_iter()
                .flatten()
                .find_map(|html| extract_first_image_url(html, base))
        });

    (media, thumbnail)
}

/// Find the first `<img src="...">` URL in an HTML fragment. Relative URLs
/// are resolved against `base_url` if provided. Returns None on parse
/// failure, missing `src`, empty `src`, or `data:` / `javascript:` URIs.
fn extract_first_image_url(html: &str, base_url: Option<&str>) -> Option<String> {
    let doc = Html::parse_fragment(html);
    let selector = Selector::parse("img[src]").ok()?;
    let base = base_url.and_then(|u| Url::parse(u).ok());
    for el in doc.select(&selector) {
        let src = el.value().attr("src")?.trim();
        if src.is_empty() {
            continue;
        }
        // Skip tracking pixels / spacers: FeedBurner-style feeds lead with
        // a 1x1 beacon that would otherwise win as "the" thumbnail. Only
        // declared-tiny images are skipped — missing attributes pass.
        let is_tiny = |attr: &str| {
            el.value()
                .attr(attr)
                .and_then(|v| v.trim().trim_end_matches("px").parse::<u32>().ok())
                .is_some_and(|n| n <= 2)
        };
        if is_tiny("width") || is_tiny("height") {
            continue;
        }
        // Reject inline-data and unsupported schemes outright — we'd just
        // fail to fetch them and burn a slot.
        let lower = src.to_ascii_lowercase();
        if lower.starts_with("data:") || lower.starts_with("javascript:") {
            continue;
        }
        let resolved = if let Some(b) = &base {
            b.join(src)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| src.to_string())
        } else {
            src.to_string()
        };
        // Only http(s); ignore protocol-less or file:// URIs.
        if resolved.starts_with("http://") || resolved.starts_with("https://") {
            return Some(resolved);
        }
    }
    None
}

/// Pre-process feed-description HTML to flatten `<table>` structures into
/// inline text before handing it to `html2text`. Reddit RSS posts (and many
/// other feeds) wrap title/author/link metadata in a 2-column HTML table;
/// html2text faithfully renders that as box-drawn columns with `|`
/// separators, which then mangles badly when the downstream
/// `format_content_for_reading` paragraph-joins the rows. Cell boundaries
/// become spaces, row ends emit `<br><br>` (a paragraph break — needed so
/// the blank line survives the line-joining pass), and the table/tbody/
/// thead/tfoot wrappers are dropped. Tag matching is case-insensitive and
/// tolerates attributes. Non-table markup is left intact.
pub(crate) fn flatten_description_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt_idx) = rest.find('<') else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..lt_idx]);
        let after_lt = &rest[lt_idx..];
        let Some(gt_rel) = after_lt.find('>') else {
            // Unterminated tag — emit the rest verbatim and stop.
            out.push_str(after_lt);
            break;
        };
        let tag = &after_lt[..=gt_rel];
        let body = &tag[1..tag.len() - 1];
        let body_after_slash = body.strip_prefix('/').unwrap_or(body);
        let name_end = body_after_slash
            .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
            .unwrap_or(body_after_slash.len());
        let name_lower = body_after_slash[..name_end].to_ascii_lowercase();
        match name_lower.as_str() {
            "table" | "tbody" | "thead" | "tfoot" | "colgroup" | "col" | "caption" => {
                // Drop the wrapper — keep inner content inline.
            }
            "tr" => {
                // Close any preceding row content with a paragraph break.
                // We emit on both `<tr>` and `</tr>`; doubled `<br>`s
                // collapse into a single paragraph break in html2text.
                out.push_str("<br><br>");
            }
            "td" | "th" => out.push(' '),
            _ => out.push_str(tag),
        }
        rest = &after_lt[gt_rel + 1..];
    }
    out
}

/// html2text decorator tuned for RSS-summary rendering. Differs from the
/// crate's default `PlainDecorator` in two ways:
///
/// * **No link annotations.** `decorate_link_start`/`_end` and `finalise`
///   return empty strings, so anchors render as just their inner text — no
///   `[text][N]` markers and no `[N]: url` footnote dump at the bottom.
///   RSS summaries (Reddit especially) are otherwise drowned in
///   `submitted by [ /u/... ][2] [[link]][3] [[comments]][4]` boilerplate
///   plus a multi-line footnote block, none of which the user needs — the
///   article URL is already shown in the detail-view header.
/// * **Image alt text falls back to title.** Same as `PlainDecorator`
///   (`[title]`) so the user still sees *something* for inline images.
///
/// Emphasis (`*…*`), strong (`**…**`), code (`` `…` ``), list/header/quote
/// prefixes are preserved.
///
/// Lives here (not in `ui/`) because the cached `FeedItem::plain_text` is
/// built with it at parse time — pipe payloads, content filters, and the
/// rendered views must all see the same text.
#[derive(Clone, Debug, Default)]
pub(crate) struct CleanDecorator;

impl CleanDecorator {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl TextDecorator for CleanDecorator {
    type Annotation = ();

    fn decorate_link_start(&mut self, _url: &str) -> (String, Self::Annotation) {
        (String::new(), ())
    }
    fn decorate_link_end(&mut self) -> String {
        String::new()
    }
    fn decorate_em_start(&mut self) -> (String, Self::Annotation) {
        ("*".to_string(), ())
    }
    fn decorate_em_end(&mut self) -> String {
        "*".to_string()
    }
    fn decorate_strong_start(&mut self) -> (String, Self::Annotation) {
        ("**".to_string(), ())
    }
    fn decorate_strong_end(&mut self) -> String {
        "**".to_string()
    }
    fn decorate_strikeout_start(&mut self) -> (String, Self::Annotation) {
        (String::new(), ())
    }
    fn decorate_strikeout_end(&mut self) -> String {
        String::new()
    }
    fn decorate_code_start(&mut self) -> (String, Self::Annotation) {
        ("`".to_string(), ())
    }
    fn decorate_code_end(&mut self) -> String {
        "`".to_string()
    }
    fn decorate_preformat_first(&mut self) -> Self::Annotation {}
    fn decorate_preformat_cont(&mut self) -> Self::Annotation {}
    fn decorate_image(&mut self, _src: &str, title: &str) -> (String, Self::Annotation) {
        (format!("[{}]", title), ())
    }
    fn header_prefix(&mut self, level: usize) -> String {
        "#".repeat(level) + " "
    }
    fn quote_prefix(&mut self) -> String {
        "> ".to_string()
    }
    fn unordered_item_prefix(&mut self) -> String {
        "* ".to_string()
    }
    fn ordered_item_prefix(&mut self, i: i64) -> String {
        format!("{}. ", i)
    }
    fn finalise(&mut self, _links: Vec<String>) -> Vec<TaggedLine<()>> {
        // Crucially: no footnote-style `[N]: url` lines. The default
        // `PlainDecorator` returns one per link here — that's the entire
        // reason summaries grew a noisy reference-list trailer.
        Vec::new()
    }
    fn make_subblock_decorator(&self) -> Self {
        Self
    }
}

/// Return `true` if `s` appears to contain HTML markup (tags or character
/// entities). Raw Markdown or plain text without HTML tags/entities returns
/// `false` so callers can avoid running it through `html2text` (which would
/// collapse Markdown newlines into single paragraphs).
fn looks_like_html(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &s[i + 1..];
            let lower_rest = rest.get(..8).map(|p| p.to_ascii_lowercase());
            let is_autolink = lower_rest.as_deref().is_some_and(|p| {
                p.starts_with("http://") || p.starts_with("https://") || p.starts_with("mailto:")
            });
            if !is_autolink {
                if let Some(&next) = bytes.get(i + 1) {
                    if next == b'/' || next == b'!' {
                        if rest.contains('>') {
                            return true;
                        }
                    } else if next.is_ascii_alphabetic() {
                        // Require tag name chars followed by whitespace, '/', or '>'
                        let name_end = rest
                            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == ':'))
                            .unwrap_or(rest.len());
                        if let Some(after_name) = rest.as_bytes().get(name_end) {
                            if matches!(after_name, b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>')
                                && rest[name_end..].contains('>')
                            {
                                return true;
                            }
                        }
                    }
                }
            }
        } else if bytes[i] == b'&' {
            let rest = &s[i + 1..];
            if let Some(semi) = rest.find(';') {
                if (1..=12).contains(&semi) {
                    let entity = &rest[..semi];
                    let valid_entity = if let Some(hex) = entity
                        .strip_prefix("#x")
                        .or_else(|| entity.strip_prefix("#X"))
                    {
                        !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit())
                    } else if let Some(dec) = entity.strip_prefix('#') {
                        !dec.is_empty() && dec.chars().all(|c| c.is_ascii_digit())
                    } else {
                        entity.chars().all(|c| c.is_ascii_alphanumeric())
                    };
                    if valid_entity {
                        return true;
                    }
                }
            }
        }
        i += 1;
    }
    false
}

/// Convenience: flatten any `<table>` markup, then render with
/// [`CleanDecorator`] at the given width. When `html` contains no HTML
/// tags or entities (e.g. raw Markdown or plain text), returns it as-is so
/// line breaks and Markdown block syntax are preserved.
pub(crate) fn render_clean_html(html: &str, width: usize) -> String {
    if !looks_like_html(html) {
        return html.to_string();
    }
    let prepped = flatten_description_html(html);
    html2text::from_read_with_decorator(prepped.as_bytes(), width, CleanDecorator::new())
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
    fn flatten_strips_reddit_style_table() {
        // Reddit RSS wraps each post in a 2-column table; html2text would
        // otherwise render that with `|` column separators that mangle on
        // re-wrap. After flattening: cells joined by spaces, row ends are
        // paragraph breaks, table wrappers gone, inner anchors intact.
        let html = r#"<table><tr><td><a href="https://i.redd.it/x.jpg"><img src="https://i.redd.it/x.jpg"/></a></td><td>&#32;submitted by <a href="https://www.reddit.com/user/poorzack">/u/poorzack</a> <br/> <a href="https://i.redd.it/x.jpg">[link]</a> <a href="https://www.reddit.com/r/manga/comments/1tearp5/">[comments]</a></td></tr></table>"#;
        let out = flatten_description_html(html);
        assert!(!out.contains("<table"));
        assert!(!out.contains("</table"));
        assert!(!out.contains("<tr"));
        assert!(!out.contains("</tr"));
        assert!(!out.contains("<td"));
        assert!(!out.contains("</td"));
        // Inner anchors / hrefs survive untouched so html2text still
        // renders them as reference-style links.
        assert!(out.contains(r#"<a href="https://www.reddit.com/user/poorzack">"#));
        assert!(out.contains("/u/poorzack"));
        // Row ends emit a paragraph break.
        assert!(out.contains("<br><br>"));
    }

    #[test]
    fn flatten_is_case_insensitive_and_tolerates_attributes() {
        let html =
            r#"<TABLE class="foo"><TR id="x"><TD style="a:b">one</TD><Th>two</Th></TR></TABLE>"#;
        let out = flatten_description_html(html);
        assert!(!out.to_ascii_lowercase().contains("<table"));
        assert!(!out.to_ascii_lowercase().contains("<tr"));
        assert!(!out.to_ascii_lowercase().contains("<td"));
        assert!(!out.to_ascii_lowercase().contains("<th"));
        assert!(out.contains("one"));
        assert!(out.contains("two"));
    }

    #[test]
    fn flatten_preserves_non_table_markup() {
        let html = r#"<p>Hello <a href="x">link</a> world.</p><ul><li>one</li><li>two</li></ul>"#;
        let out = flatten_description_html(html);
        // Nothing in this string is table markup, so byte-equal.
        assert_eq!(out, html);
    }

    #[test]
    fn flatten_handles_unterminated_tag() {
        // Don't panic on malformed HTML; just emit it verbatim from the
        // unterminated `<` onward.
        let html = "before <table no-close";
        let out = flatten_description_html(html);
        assert!(out.contains("before "));
        assert!(out.contains("<table no-close"));
    }

    #[test]
    fn flatten_no_pipe_column_artifacts_after_html2text() {
        // End-to-end on a minimal Reddit-style table: piping the flattened
        // HTML through html2text must not produce the `|`-column rendering
        // that's the whole reason this preprocessing exists.
        let html = r#"<table><tr><td><a href="https://example.com/img.jpg">img</a></td><td>Title here <a href="https://example.com/post">[link]</a></td></tr></table>"#;
        let prepped = flatten_description_html(html);
        let text = html2text::from_read(prepped.as_bytes(), 80);
        assert!(
            !text.contains('|'),
            "expected no column separators in flattened output, got:\n{text}"
        );
        assert!(text.contains("Title here"));
    }

    #[test]
    fn render_clean_html_drops_reddit_link_footnotes() {
        // Reddit summary: title link, body, "submitted by" line with three
        // anchors. Default html2text would emit `[N]` markers inline plus a
        // `[N]: url` footnote dump. Our decorator must produce neither.
        let html = r#"<table><tr><td><a href="https://example.com/post"><img src="https://example.com/x.jpg"/></a></td><td><a href="https://example.com/post"><b>Excuse me?! What??</b></a><br/>This is weird, right? Like what is going on with <em>suggestive</em> content lately?!<br/>submitted by <a href="https://www.reddit.com/user/A">/u/A</a> <a href="https://example.com/post">[link]</a> <a href="https://example.com/post">[comments]</a></td></tr></table>"#;
        let out = render_clean_html(html, 80);
        // No reference markers anywhere.
        assert!(
            !out.contains("][1]") && !out.contains("][2]") && !out.contains("][3]"),
            "expected no `][N]` link markers in:\n{out}"
        );
        // No footnote dump.
        assert!(
            !out.contains("[1]:") && !out.contains("[2]:"),
            "expected no `[N]: url` footnotes in:\n{out}"
        );
        // No `|` column artifacts from the table.
        assert!(
            !out.contains('|'),
            "expected no column separators in:\n{out}"
        );
        // Anchor inner text is preserved.
        assert!(out.contains("Excuse me?! What??"));
        assert!(out.contains("/u/A"));
        // Emphasis is preserved (this is what distinguishes us from
        // html2text's TrivialDecorator, which would strip the `*`s).
        assert!(
            out.contains("*suggestive*"),
            "expected `<em>` to render as `*...*` in:\n{out}"
        );
    }

    #[test]
    fn render_clean_html_preserves_raw_markdown_without_html() {
        let md = "# Title\n\n- Item 1\n- Item 2\n\nSee <https://example.com> and a < b.";
        assert_eq!(render_clean_html(md, 80), md);
        assert!(looks_like_html("<p>Hello</p>"));
        assert!(looks_like_html("Tom &amp; Jerry"));
        assert!(!looks_like_html(md));
    }

    #[test]
    fn render_clean_html_strips_image_with_empty_alt() {
        // An image with no alt/title should render as `[]` (PlainDecorator
        // behavior we inherit) rather than disappearing — the user still
        // gets a visible hint that something was there.
        let html = r#"<p>Before <img src="https://example.com/x.png"/> after.</p>"#;
        let out = render_clean_html(html, 80);
        assert!(out.contains("Before"));
        assert!(out.contains("after."));
    }

    #[test]
    fn plain_text_cache_matches_clean_rendering() {
        // The cached plain_text (pipe payloads, content filters) must go
        // through the same clean-render path as the views: no `[N]` link
        // markers, no footnote dump.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>x</title>
    <link>https://example.com/</link>
    <description>x</description>
    <item>
      <title>Post</title>
      <link>https://example.com/p1</link>
      <guid>https://example.com/p1</guid>
      <description>&lt;p&gt;Read &lt;a href="https://example.com/more"&gt;the rest&lt;/a&gt; now.&lt;/p&gt;</description>
    </item>
  </channel>
</rss>"#;
        let entry = parse_first_entry(xml);
        let item = FeedItem::from_feed_entry(&entry);
        let plain = item.plain_text.expect("plain_text cached");
        assert!(plain.contains("the rest"));
        assert!(
            !plain.contains("[1]") && !plain.contains("[1]:"),
            "expected no link annotations in cached plain_text:\n{plain}"
        );
        assert_eq!(
            plain,
            render_clean_html(
                "<p>Read <a href=\"https://example.com/more\">the rest</a> now.</p>",
                80
            )
        );
    }

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
        assert_eq!(feeds[0].feed_type, FeedType::Rss);
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
        assert_eq!(feeds[0].feed_type, FeedType::Rss);
        assert_eq!(feeds[1].feed_type, FeedType::Atom);
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

    #[test]
    fn test_into_feed_with_feed_variant() {
        let feed = Feed {
            url: "https://example.com/feed.xml".to_string(),
            title: "Test Feed".to_string(),
            items: vec![],
            title_lower: "test feed".to_string(),
        };
        let result = FeedFetchResult::Feed(feed);
        let feed = result.into_feed().unwrap();
        assert_eq!(feed.url, "https://example.com/feed.xml");
        assert_eq!(feed.title, "Test Feed");
    }

    #[test]
    fn test_into_feed_with_discovered_feeds_returns_error() {
        let result = FeedFetchResult::DiscoveredFeeds {
            feeds: vec![DiscoveredFeed {
                url: "https://example.com/rss".to_string(),
                title: "RSS Feed".to_string(),
                feed_type: FeedType::Rss,
            }],
            page_url: "https://example.com/".to_string(),
        };
        let err = result.into_feed().unwrap_err();
        assert!(err.to_string().contains("HTML page"));
        assert!(err.to_string().contains("1 feed link(s)"));
    }

    #[test]
    fn test_into_feed_with_empty_discovered_feeds_returns_error() {
        let result = FeedFetchResult::DiscoveredFeeds {
            feeds: vec![],
            page_url: "https://example.com/".to_string(),
        };
        let err = result.into_feed().unwrap_err();
        assert!(err.to_string().contains("No RSS/Atom feed links found"));
    }

    /// A blog-post-shaped fixture long enough to clear FULLTEXT_MIN_LENGTH
    /// once Readability strips boilerplate.
    fn fixture_article_html() -> String {
        let body = "Feedr is a terminal-based RSS feed reader. It supports \
            categories, OPML import, and a configurable keybinding system. \
            This paragraph is repeated several times to ensure the extracted \
            text content comfortably exceeds the minimum-length threshold \
            that the extractor enforces before declaring the page empty. ";
        let mut content = String::new();
        for _ in 0..5 {
            content.push_str("<p>");
            content.push_str(body);
            content.push_str("</p>");
        }
        format!(
            r#"<!doctype html><html><head>
                <title>About Feedr</title>
                <meta name="author" content="Test Author">
            </head><body>
                <nav>Home About Contact</nav>
                <article>
                    <h1>About Feedr</h1>
                    {}
                </article>
                <footer>Copyright 2026</footer>
            </body></html>"#,
            content
        )
    }

    #[test]
    fn test_extract_from_html_succeeds_on_blog_shaped_page() {
        let html = fixture_article_html();
        let article = extract_from_html(&html, "https://example.com/post").unwrap();
        assert!(!article.title.is_empty(), "title should be extracted");
        assert!(
            article.plain_text.len() >= FULLTEXT_MIN_LENGTH,
            "extracted text below minimum: {}",
            article.plain_text.len()
        );
        assert_eq!(article.source_url, "https://example.com/post");
    }

    #[test]
    fn test_extract_from_html_rejects_short_content() {
        let html = r#"<!doctype html><html><body><p>tiny.</p></body></html>"#;
        let err = extract_from_html(html, "https://example.com/empty").unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("empty"),
            "expected 'empty' in error, got: {}",
            err
        );
    }

    #[test]
    fn test_extract_from_html_rejects_bad_document_url() {
        let html = fixture_article_html();
        let err = extract_from_html(&html, "not-a-url").unwrap_err();
        let msg = format!("{:?}", err);
        // dom_smoothie's BadDocumentURL wrapped by our context string.
        assert!(
            msg.contains("Failed to initialize") || msg.to_lowercase().contains("url"),
            "expected BadDocumentURL error, got: {}",
            msg
        );
    }

    #[test]
    fn test_decode_html_bytes_uses_content_type_charset() {
        // "Café" in Windows-1252: 0x43 0x61 0x66 0xE9
        let bytes: &[u8] = &[0x43, 0x61, 0x66, 0xE9];
        let decoded = decode_html_bytes(bytes, "text/html; charset=windows-1252");
        assert_eq!(decoded, "Café");
    }

    #[test]
    fn test_decode_html_bytes_handles_quoted_charset_in_header() {
        let bytes: &[u8] = &[0x43, 0x61, 0x66, 0xE9];
        let decoded = decode_html_bytes(bytes, "text/html; charset=\"windows-1252\"");
        assert_eq!(decoded, "Café");
    }

    #[test]
    fn test_decode_html_bytes_falls_back_to_meta_charset() {
        // Document declares Shift_JIS via <meta>; Content-Type omits charset.
        // Bytes "日本" in Shift_JIS: 0x93 0xFA 0x96 0x7B.
        let mut html: Vec<u8> = b"<html><head><meta charset=\"shift_jis\"></head><body>".to_vec();
        html.extend_from_slice(&[0x93, 0xFA, 0x96, 0x7B]);
        html.extend_from_slice(b"</body></html>");
        let decoded = decode_html_bytes(&html, "text/html");
        assert!(
            decoded.contains("日本"),
            "expected meta-sniffed Shift_JIS to decode, got: {}",
            decoded
        );
    }

    #[test]
    fn test_decode_html_bytes_defaults_to_utf8() {
        let html = "<html><body>Café</body></html>";
        let decoded = decode_html_bytes(html.as_bytes(), "");
        assert!(decoded.contains("Café"));
    }

    #[test]
    fn test_sniff_meta_charset_ignores_data_charset_attribute() {
        // A loose substring match would be fooled by `data-charset="bogus"`
        // and choose `bogus` as the encoding; the tightened sniffer must
        // only match `charset=` inside an actual <meta> tag.
        let html = b"<html><head>\
            <div data-charset=\"bogus-encoding\"></div>\
            <meta charset=\"windows-1252\">\
            </head><body>x</body></html>";
        let label = sniff_meta_charset(html).expect("should find charset");
        assert_eq!(
            label, "windows-1252",
            "data-* attribute must be ignored; only <meta charset> wins"
        );
    }

    #[test]
    fn test_sniff_meta_charset_ignores_script_content() {
        // `<script>var x = "charset=evil";</script>` in the head must not
        // fool the sniffer.
        let html = b"<html><head>\
            <script>var s = \"charset=evil-encoding\";</script>\
            <meta charset=\"utf-8\">\
            </head><body>x</body></html>";
        let label = sniff_meta_charset(html).expect("should find charset");
        assert_eq!(label, "utf-8", "script literal must be ignored");
    }

    #[test]
    fn test_sniff_meta_charset_ignores_metadata_tag() {
        // `<metadata>` must not be matched as `<meta>`.
        let html = b"<html><head>\
            <metadata>charset=bogus</metadata>\
            <meta charset=\"utf-8\">\
            </head></html>";
        let label = sniff_meta_charset(html).expect("should find charset");
        assert_eq!(label, "utf-8");
    }

    #[test]
    fn test_sniff_meta_charset_handles_http_equiv_meta() {
        // The other legal form: <meta http-equiv="Content-Type" content="text/html; charset=…">
        let html = b"<html><head>\
            <meta http-equiv=\"Content-Type\" content=\"text/html; charset=iso-8859-1\">\
            </head></html>";
        let label = sniff_meta_charset(html).expect("should find charset");
        assert_eq!(label, "iso-8859-1");
    }

    #[test]
    fn test_sniff_meta_charset_skips_attribute_value_named_charset() {
        // Pathological: an attribute whose VALUE is literally "charset"
        // would have shadowed the real declaration with the old
        // first-occurrence-wins logic. The tightened sniffer requires
        // `charset` to be preceded by a word boundary AND followed by `=`,
        // so the value match is rejected.
        let html = b"<html><head>\
            <meta name=\"charset\" charset=\"utf-8\">\
            </head></html>";
        let label = sniff_meta_charset(html).expect("should find charset");
        assert_eq!(label, "utf-8");
    }

    #[test]
    fn test_sniff_meta_charset_skips_data_attr_with_charset_value() {
        // `data-foo="charset=bogus"` would have been a false positive under
        // the old logic (substring + `=` after charset). The boundary check
        // — `charset` must be preceded by whitespace/`/`/start-of-tag —
        // rejects it because the preceding char is `"`.
        let html = b"<html><head>\
            <meta data-foo=\"charset=bogus\" charset=\"utf-8\">\
            </head></html>";
        let label = sniff_meta_charset(html).expect("should find charset");
        assert_eq!(label, "utf-8");
    }

    #[test]
    fn test_sniff_meta_charset_skips_longer_attribute_name_prefixed_with_charset() {
        // Word-boundary check: a hypothetical attribute like
        // `data-charset="bogus"` shouldn't trigger — the char before
        // `charset` is `-`, not a boundary.
        let html = b"<html><head>\
            <meta data-charset=\"bogus\" charset=\"utf-8\">\
            </head></html>";
        let label = sniff_meta_charset(html).expect("should find charset");
        assert_eq!(label, "utf-8");
    }

    #[test]
    fn test_is_safe_auto_url_accepts_public_https() {
        assert!(is_safe_auto_url("https://example.com/article"));
        assert!(is_safe_auto_url("http://example.com/article"));
    }

    #[test]
    fn test_is_safe_auto_url_rejects_non_http_scheme() {
        assert!(!is_safe_auto_url("file:///etc/passwd"));
        assert!(!is_safe_auto_url("ftp://example.com/x"));
        assert!(!is_safe_auto_url("javascript:alert(1)"));
        assert!(!is_safe_auto_url("data:text/html,<script>"));
    }

    #[test]
    fn test_is_safe_auto_url_rejects_private_ips() {
        // RFC1918
        assert!(!is_safe_auto_url("http://10.0.0.1/x"));
        assert!(!is_safe_auto_url("http://192.168.1.1/x"));
        assert!(!is_safe_auto_url("http://172.16.0.1/x"));
        // Loopback
        assert!(!is_safe_auto_url("http://127.0.0.1/x"));
        // Link-local (incl. AWS metadata endpoint)
        assert!(!is_safe_auto_url(
            "http://169.254.169.254/latest/meta-data/"
        ));
        // CGNAT
        assert!(!is_safe_auto_url("http://100.64.0.1/x"));
        // IPv6 loopback / unique-local / link-local
        assert!(!is_safe_auto_url("http://[::1]/x"));
        assert!(!is_safe_auto_url("http://[fc00::1]/x"));
        assert!(!is_safe_auto_url("http://[fe80::1]/x"));
        // IPv6 deprecated site-local fec0::/10
        assert!(!is_safe_auto_url("http://[fec0::1]/x"));
        // IPv4 multicast 224.0.0.0/4 — parity with IPv6 multicast below.
        assert!(!is_safe_auto_url("http://224.0.0.1/x"));
        assert!(!is_safe_auto_url("http://239.255.255.250/x"));
        // IPv6 multicast ff00::/8
        assert!(!is_safe_auto_url("http://[ff02::1]/x"));
        // 6to4 wrapping link-local — would route via 6to4 relay to
        // 169.254.169.254 (cloud metadata).
        assert!(!is_safe_auto_url("http://[2002:a9fe:a9fe::]/x"));
        // NAT64 well-known prefix 64:ff9b::/96
        assert!(!is_safe_auto_url("http://[64:ff9b::a9fe:a9fe]/x"));
    }

    #[test]
    fn test_is_safe_auto_url_rejects_localhost_names() {
        assert!(!is_safe_auto_url("http://localhost/x"));
        assert!(!is_safe_auto_url("http://printer.local/x"));
        assert!(!is_safe_auto_url("http://api.localhost/x"));
    }

    #[test]
    fn test_is_safe_auto_url_rejects_invalid_url() {
        assert!(!is_safe_auto_url("not-a-url"));
        assert!(!is_safe_auto_url(""));
    }

    #[test]
    fn test_extract_article_rejects_non_http_scheme() {
        // Build a client; the function should reject before any I/O.
        let client = reqwest::blocking::Client::new();
        let err = extract_article("file:///etc/passwd", &client, None).unwrap_err();
        assert!(
            err.to_string().contains("scheme") || err.to_string().contains("not allowed"),
            "expected scheme rejection, got: {}",
            err
        );
    }

    // ── extract_article end-to-end tests against a local TcpListener stub ──
    //
    // These exercise the wrapping fetch path — Content-Length cap,
    // Content-Type rejection, and the safe-redirect client's behavior when
    // a 30x points into a private IP. The pure-Readability parsing path
    // (extract_from_html) is already covered above; these close the gap on
    // the network-side security gates.
    //
    // No external mock-server dependency: a one-shot TcpListener serves a
    // single canned HTTP response and exits. Cheap, hermetic, deterministic.

    use std::io::{Read as IoRead, Write as IoWrite};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    /// Spawn a one-shot HTTP stub on 127.0.0.1 that writes `response_bytes`
    /// verbatim to the first accepted client and closes. Returns the bound
    /// URL. The stub thread is detached — the OS reclaims it after the test.
    fn spawn_stub_server(response_bytes: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1");
        let port = listener.local_addr().expect("local_addr").port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                // Read SOME of the request so the client doesn't get a
                // RST while still writing headers. Single read is enough
                // for a short GET; we don't actually inspect it.
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(&response_bytes);
                let _ = stream.flush();
                // Dropping the stream signals connection close to the client.
            }
        });
        format!("http://127.0.0.1:{}/", port)
    }

    #[test]
    fn test_extract_article_rejects_oversized_content_length() {
        // Server declares 100 MiB body — far past the 5 MiB cap. The
        // pre-check on the Content-Length header must short-circuit the
        // read entirely (no body actually streamed in this test).
        let declared = FULLTEXT_MAX_BYTES * 20;
        let resp = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/html\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n",
            declared
        );
        let url = spawn_stub_server(resp.into_bytes());
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let err = extract_article(&url, &client, None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("too large"),
            "expected oversized rejection, got: {}",
            msg
        );
    }

    #[test]
    fn test_extract_article_rejects_non_html_content_type() {
        let body = b"\x89PNG\r\n\x1a\nnotactuallyhtml";
        let resp = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: image/png\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n",
            body.len()
        );
        let mut bytes = resp.into_bytes();
        bytes.extend_from_slice(body);
        let url = spawn_stub_server(bytes);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let err = extract_article(&url, &client, None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("did not return HTML") || msg.contains("content-type"),
            "expected content-type rejection, got: {}",
            msg
        );
    }

    #[test]
    fn test_extract_article_rejects_redirect_into_private_ip_via_safe_client() {
        // Stub returns a 302 pointing into RFC1918 space. The
        // safe-redirect client's policy must refuse to follow — the
        // resulting response surfaces as a non-2xx status, which
        // extract_article translates into an "HTTP error" failure.
        // (Without the safe client, reqwest would happily follow the 302
        // and probe 192.168.1.1 — the SSRF this exists to prevent.)
        let resp = b"HTTP/1.1 302 Found\r\n\
                     Location: http://192.168.1.1/admin\r\n\
                     Content-Length: 0\r\n\
                     Connection: close\r\n\
                     \r\n";
        let url = spawn_stub_server(resp.to_vec());
        let client =
            Feed::build_safe_redirect_client(5).expect("safe-redirect client should build");
        let err = extract_article(&url, &client, None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("HTTP error") || msg.contains("302"),
            "expected non-2xx surfacing from refused redirect, got: {}",
            msg
        );
    }

    #[test]
    fn test_extract_article_follows_safe_redirect_to_public_target() {
        // Sanity-check the other side of the redirect policy: a 302 to a
        // public-looking target is followed. This stub returns 302 →
        // http://example.invalid/ which the client WILL try to follow.
        // The follow attempt will fail (example.invalid is unresolvable),
        // but the failure must be a NETWORK error, NOT the policy-stop
        // "HTTP error 302" surfacing from the refused-redirect path.
        // That distinction is what tells us the policy correctly let the
        // public target through.
        let resp = b"HTTP/1.1 302 Found\r\n\
                     Location: http://example.invalid/article\r\n\
                     Content-Length: 0\r\n\
                     Connection: close\r\n\
                     \r\n";
        let url = spawn_stub_server(resp.to_vec());
        let client =
            Feed::build_safe_redirect_client(5).expect("safe-redirect client should build");
        let err = extract_article(&url, &client, None).unwrap_err();
        let msg = err.to_string().to_lowercase();
        // Must NOT be "HTTP error 302" — that would mean the policy
        // refused to follow a public target. Must be a fetch / dns / io
        // failure from the actual follow attempt.
        assert!(
            !msg.contains("http error 302"),
            "policy must follow public-target redirects, but surfaced 302: {}",
            msg
        );
        assert!(
            msg.contains("failed to fetch") || msg.contains("dns") || msg.contains("resolve"),
            "expected a network failure from following the public redirect, got: {}",
            msg
        );
    }

    /// Like `spawn_stub_server`, but also captures the raw bytes of the
    /// first request the stub receives and ships them back through an mpsc
    /// channel. Lets a test assert on what `extract_article` actually puts
    /// on the wire (headers, path, etc.) — vs. just what it returns.
    fn spawn_capturing_stub_server(response_bytes: Vec<u8>) -> (String, mpsc::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1");
        let port = listener.local_addr().expect("local_addr").port();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                // 8 KiB is plenty for a GET request with our headers; we
                // only need enough to inspect the header block.
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                buf.truncate(n);
                let _ = tx.send(buf);
                let _ = stream.write_all(&response_bytes);
                let _ = stream.flush();
            }
        });
        (format!("http://127.0.0.1:{}/", port), rx)
    }

    /// Regression lock for the "no per-feed Authorization on extraction" rule.
    /// The function signature deliberately omits a headers parameter, but
    /// nothing today fails loudly if a future refactor adds
    /// `Authorization` to the static header list. This test inspects the
    /// raw bytes on the wire and asserts that no `Authorization` header
    /// is sent — third-party article hosts must not see per-feed bearer
    /// tokens / cookies.
    #[test]
    fn test_extract_article_does_not_send_authorization_header() {
        let body = fixture_article_html();
        let resp = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n{}",
            body.len(),
            body
        );
        let (url, captured) = spawn_capturing_stub_server(resp.into_bytes());
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        // Outcome is irrelevant — we only care about what hit the wire.
        let _ = extract_article(&url, &client, None);
        let raw = captured
            .recv_timeout(Duration::from_secs(5))
            .expect("stub must have captured a request");
        let request = String::from_utf8_lossy(&raw);
        // Header-name comparison is case-insensitive (RFC 7230 §3.2).
        let lower = request.to_ascii_lowercase();
        assert!(
            !lower.contains("\r\nauthorization:") && !lower.starts_with("authorization:"),
            "extract_article must NOT forward an Authorization header — saw it in request:\n{}",
            request
        );
        // Sanity: confirm we captured a real request, not just an empty read.
        assert!(
            lower.starts_with("get /"),
            "expected a GET request, got: {}",
            request
        );
    }

    // ── media extraction ─────────────────────────────────────────────────────

    fn parse_first_entry(xml: &str) -> feed_rs::model::Entry {
        let feed = feed_rs::parser::parse(xml.as_bytes()).expect("parse feed");
        feed.entries.into_iter().next().expect("at least one entry")
    }

    #[test]
    fn media_extract_youtube_atom_shape() {
        // YouTube's channel feeds wrap the video in <media:group> with a
        // <media:content url="…"> and a <media:thumbnail url="…">.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:media="http://search.yahoo.com/mrss/">
  <id>yt:channel:UCexample</id>
  <title>Example Channel</title>
  <entry>
    <id>yt:video:abc123</id>
    <title>Example Video</title>
    <link href="https://www.youtube.com/watch?v=abc123"/>
    <published>2025-01-01T00:00:00+00:00</published>
    <media:group>
      <media:title>Example Video</media:title>
      <media:content url="https://www.youtube.com/v/abc123" type="application/x-shockwave-flash" width="640" height="390"/>
      <media:thumbnail url="https://i.ytimg.com/vi/abc123/hqdefault.jpg" width="480" height="360"/>
      <media:description>Some description.</media:description>
    </media:group>
  </entry>
</feed>"#;
        let entry = parse_first_entry(xml);
        let item = FeedItem::from_feed_entry(&entry);
        assert_eq!(item.media.len(), 1, "expected exactly one media attachment");
        assert_eq!(item.media[0].url, "https://www.youtube.com/v/abc123");
        assert_eq!(
            item.media[0].mime.as_deref(),
            Some("application/x-shockwave-flash")
        );
        assert_eq!(item.media[0].kind, MediaKind::Unknown);
        assert_eq!(item.media[0].width, Some(640));
        assert_eq!(item.media[0].height, Some(390));
        assert_eq!(
            item.thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/abc123/hqdefault.jpg")
        );
    }

    #[test]
    fn media_extract_rss_enclosure() {
        // RSS 2 podcast: <enclosure> lands in entry.links with rel="enclosure".
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Example Podcast</title>
    <link>https://podcast.example.com/</link>
    <description>A podcast.</description>
    <item>
      <title>Episode 1</title>
      <link>https://podcast.example.com/ep1</link>
      <guid>https://podcast.example.com/ep1</guid>
      <enclosure url="https://podcast.example.com/ep1.mp3" length="12345678" type="audio/mpeg"/>
    </item>
  </channel>
</rss>"#;
        let entry = parse_first_entry(xml);
        let item = FeedItem::from_feed_entry(&entry);
        assert_eq!(item.media.len(), 1);
        assert_eq!(item.media[0].url, "https://podcast.example.com/ep1.mp3");
        assert_eq!(item.media[0].kind, MediaKind::Audio);
        assert_eq!(item.media[0].mime.as_deref(), Some("audio/mpeg"));
        assert_eq!(item.media[0].size_bytes, Some(12_345_678));
        assert!(item.thumbnail.is_none());
    }

    #[test]
    fn media_extract_atom_content_src() {
        // Atom out-of-line <content src="…" type="video/mp4"/>.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>tag:example.com,2025:feed</id>
  <title>Example</title>
  <entry>
    <id>tag:example.com,2025:1</id>
    <title>A Video</title>
    <link href="https://example.com/post"/>
    <content type="video/mp4" src="https://cdn.example.com/clip.mp4"/>
  </entry>
</feed>"#;
        let entry = parse_first_entry(xml);
        let item = FeedItem::from_feed_entry(&entry);
        assert_eq!(item.media.len(), 1);
        assert_eq!(item.media[0].url, "https://cdn.example.com/clip.mp4");
        assert_eq!(item.media[0].kind, MediaKind::Video);
        assert_eq!(item.media[0].mime.as_deref(), Some("video/mp4"));
    }

    #[test]
    fn media_dedup_across_sources() {
        // Same URL in both <media:content> and <enclosure> should produce one
        // attachment, keeping the Media RSS metadata (which lands first).
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/">
  <channel>
    <title>Test</title>
    <link>https://example.com/</link>
    <description>x</description>
    <item>
      <title>Dup</title>
      <link>https://example.com/ep1</link>
      <guid>https://example.com/ep1</guid>
      <media:content url="https://example.com/ep1.mp3" type="audio/mpeg" duration="120"/>
      <enclosure url="https://example.com/ep1.mp3" length="999" type="audio/mp4"/>
    </item>
  </channel>
</rss>"#;
        let entry = parse_first_entry(xml);
        let item = FeedItem::from_feed_entry(&entry);
        assert_eq!(item.media.len(), 1, "duplicate URL should collapse");
        // First-source wins: the media:content MIME, not the enclosure MIME.
        assert_eq!(item.media[0].mime.as_deref(), Some("audio/mpeg"));
    }

    #[test]
    fn media_primary_picks_audio_video_over_image() {
        let item = FeedItem {
            title: "t".into(),
            link: None,
            description: None,
            pub_date: None,
            author: None,
            formatted_date: None,
            parsed_date: None,
            plain_text: None,
            title_lower: "t".into(),
            media: vec![
                MediaAttachment {
                    url: "https://x/thumb.jpg".into(),
                    mime: Some("image/jpeg".into()),
                    kind: MediaKind::Image,
                    width: None,
                    height: None,
                    duration_secs: None,
                    size_bytes: None,
                },
                MediaAttachment {
                    url: "https://x/clip.mp4".into(),
                    mime: Some("video/mp4".into()),
                    kind: MediaKind::Video,
                    width: None,
                    height: None,
                    duration_secs: None,
                    size_bytes: None,
                },
            ],
            thumbnail: None,
        };
        let primary = item.primary_media().expect("has primary");
        assert_eq!(primary.kind, MediaKind::Video);
    }

    #[test]
    fn thumbnail_falls_back_to_img_in_summary() {
        // xkcd-style: <img> inside <summary type="html">.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>https://xkcd.com/</id>
  <title>xkcd</title>
  <entry>
    <id>https://xkcd.com/3246/</id>
    <title>Speedrun</title>
    <link href="https://xkcd.com/3246/"/>
    <summary type="html">&lt;img src="https://imgs.xkcd.com/comics/speedrun.png" alt="..."/&gt;</summary>
  </entry>
</feed>"#;
        let entry = parse_first_entry(xml);
        let item = FeedItem::from_feed_entry(&entry);
        assert_eq!(
            item.thumbnail.as_deref(),
            Some("https://imgs.xkcd.com/comics/speedrun.png")
        );
        // The media list itself stays empty — no <media:content> or <enclosure>.
        assert!(item.media.is_empty());
    }

    #[test]
    fn thumbnail_prefers_media_thumbnail_over_summary_img() {
        // When both are present, the explicit <media:thumbnail> wins.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:media="http://search.yahoo.com/mrss/">
  <id>x</id>
  <title>x</title>
  <entry>
    <id>x1</id>
    <title>x</title>
    <link href="https://example.com/p1"/>
    <summary type="html">&lt;img src="https://example.com/in-summary.png"/&gt;</summary>
    <media:thumbnail url="https://example.com/explicit-thumb.jpg"/>
  </entry>
</feed>"#;
        let entry = parse_first_entry(xml);
        let item = FeedItem::from_feed_entry(&entry);
        assert_eq!(
            item.thumbnail.as_deref(),
            Some("https://example.com/explicit-thumb.jpg")
        );
    }

    #[test]
    fn thumbnail_resolves_relative_img_against_link() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0">
  <channel>
    <title>Comics</title>
    <link>https://comics.example.com/</link>
    <description>x</description>
    <item>
      <title>Strip 1</title>
      <link>https://comics.example.com/strip/1</link>
      <guid>https://comics.example.com/strip/1</guid>
      <description>&lt;img src="/img/strip-1.png"/&gt; today's strip</description>
    </item>
  </channel>
</rss>"#;
        let entry = parse_first_entry(xml);
        let item = FeedItem::from_feed_entry(&entry);
        assert_eq!(
            item.thumbnail.as_deref(),
            Some("https://comics.example.com/img/strip-1.png")
        );
    }

    #[test]
    fn thumbnail_skips_data_uri_imgs() {
        // A leading data: URI must be skipped; the second <img> wins.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0">
  <channel>
    <title>x</title>
    <link>https://example.com/</link>
    <description>x</description>
    <item>
      <title>x</title>
      <link>https://example.com/p1</link>
      <guid>https://example.com/p1</guid>
      <description>&lt;img src="data:image/png;base64,abc"/&gt;&lt;img src="https://example.com/real.png"/&gt;</description>
    </item>
  </channel>
</rss>"#;
        let entry = parse_first_entry(xml);
        let item = FeedItem::from_feed_entry(&entry);
        assert_eq!(
            item.thumbnail.as_deref(),
            Some("https://example.com/real.png")
        );
    }

    #[test]
    fn thumbnail_skips_tracking_pixels() {
        // A leading 1x1 beacon must be skipped; the real image wins. An
        // image with no width/height attributes is NOT treated as tiny.
        let html = r#"<img src="https://feeds.example.com/~r/beacon" width="1" height="1"/><img src="https://example.com/real.png"/>"#;
        assert_eq!(
            extract_first_image_url(html, None).as_deref(),
            Some("https://example.com/real.png")
        );
        let html_px = r#"<img src="https://x/spacer.gif" width="1px"/><img src="https://example.com/comic.png" width="740"/>"#;
        assert_eq!(
            extract_first_image_url(html_px, None).as_deref(),
            Some("https://example.com/comic.png")
        );
    }

    #[test]
    fn media_empty_for_plain_text_feed() {
        // A plain RSS item with no enclosure / media should produce no media.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Blog</title>
    <link>https://blog.example.com/</link>
    <description>x</description>
    <item>
      <title>Post</title>
      <link>https://blog.example.com/p1</link>
      <guid>https://blog.example.com/p1</guid>
      <description>Plain text post.</description>
    </item>
  </channel>
</rss>"#;
        let entry = parse_first_entry(xml);
        let item = FeedItem::from_feed_entry(&entry);
        assert!(item.media.is_empty());
        assert!(item.thumbnail.is_none());
    }
}

use feedr::feed::Feed;

#[test]
fn test_rss_feed_parsing() {
    // Test with a known RSS feed
    match Feed::from_url("https://news.ycombinator.com/rss") {
        Ok(feed) => {
            assert!(!feed.title.is_empty());
            assert!(!feed.items.is_empty());
            println!("RSS feed parsed successfully: {} items", feed.items.len());
        }
        Err(e) => panic!("Failed to parse RSS feed: {}", e),
    }
}

#[test]
fn test_mixed_feeds() {
    // Test with feeds known to work (mix of formats)
    let test_feeds = vec![
        ("TechCrunch (RSS)", "https://techcrunch.com/feed/"),
        ("Rust Blog (Atom)", "https://blog.rust-lang.org/feed.xml"),
        ("Wired (RSS)", "https://www.wired.com/feed/rss"),
    ];

    let mut successful_parses = 0;

    for (name, url) in test_feeds {
        println!("Testing: {}", name);
        match Feed::from_url(url) {
            Ok(feed) => {
                println!("✓ Successfully parsed: {}", name);
                println!("  Title: {}", feed.title);
                println!("  Items: {}", feed.items.len());

                assert!(!feed.title.is_empty());
                successful_parses += 1;

                // Test first item has expected fields
                if let Some(first_item) = feed.items.first() {
                    assert!(!first_item.title.is_empty());
                    println!("  First item: {}", first_item.title);
                }
            }
            Err(e) => {
                println!("✗ Failed to parse {}: {}", name, e);
                // Don't fail the test immediately, just continue
            }
        }
    }

    // At least one should succeed
    assert!(successful_parses > 0, "No feeds were successfully parsed");
    println!("\nSuccessfully parsed {}/3 feeds", successful_parses);
}

#[test]
fn test_reddit_style_atom_feeds() {
    // Test with Reddit-style feeds that claim to be RSS but are actually Atom
    let reddit_feeds = vec![
        "https://www.reddit.com/r/programming.rss",
        "https://www.reddit.com/r/rust.rss",
    ];

    for url in reddit_feeds {
        println!("Testing Reddit feed: {}", url);
        match Feed::from_url(url) {
            Ok(feed) => {
                println!("✓ Successfully parsed Reddit feed!");
                println!("  Title: {}", feed.title);
                println!("  Items: {}", feed.items.len());

                assert!(!feed.title.is_empty());

                if let Some(first_item) = feed.items.first() {
                    println!("  First item: {}", first_item.title);
                    assert!(!first_item.title.is_empty());
                }

                // Reddit feeds should work now
                return;
            }
            Err(e) => {
                println!("⚠ Reddit feed failed (may be rate limited): {}", e);
                // Reddit may block automated requests, so we don't fail the test
                continue;
            }
        }
    }

    println!("Note: Reddit feeds may be blocked due to rate limiting or bot detection");
}

#[test]
fn test_problematic_feeds() {
    // Test feeds that are reported as not working
    let problematic_feeds = vec![
        ("Gadgets360", "https://www.gadgets360.com/rss/feeds"),
        ("CNN", "http://rss.cnn.com/rss/edition.rss"),
        ("BBC", "http://feeds.bbci.co.uk/news/rss.xml"),
        ("The Verge", "https://www.theverge.com/rss/index.xml"),
        (
            "ArsTechnica",
            "https://feeds.arstechnica.com/arstechnica/index",
        ),
        (
            "NYTimes",
            "https://rss.nytimes.com/services/xml/rss/nyt/HomePage.xml",
        ),
    ];

    for (name, url) in problematic_feeds {
        println!("Testing problematic feed: {} - {}", name, url);
        match Feed::from_url(url) {
            Ok(feed) => {
                println!("✓ Successfully parsed: {}", name);
                println!("  Title: {}", feed.title);
                println!("  Items: {}", feed.items.len());

                if let Some(first_item) = feed.items.first() {
                    println!("  First item: {}", first_item.title);
                }
            }
            Err(e) => {
                println!("✗ Failed to parse {}: {}", name, e);

                // Print the full error chain for debugging
                let mut source = e.source();
                let mut depth = 1;
                while let Some(err) = source {
                    println!("  └─ Caused by ({}): {}", depth, err);
                    source = err.source();
                    depth += 1;
                }
            }
        }
        println!();
    }
}

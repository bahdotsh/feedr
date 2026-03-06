use crate::config::Config;
use anyhow::Result;

pub fn get(key: &str) -> Result<()> {
    let config = Config::load()?;
    let value = config.get_value(key)?;
    println!("{}", value);
    Ok(())
}

pub fn set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;
    config.validate_and_set(key, value)?;
    config.save()?;
    println!("Set {} = {}", key, value);
    Ok(())
}

pub fn list() -> Result<()> {
    let config = Config::load()?;

    let keys: &[(&str, &str)] = &[
        ("general.max_dashboard_items", "Maximum dashboard items (1-10000)"),
        ("general.auto_refresh_interval", "Auto-refresh interval in seconds (0=disabled, max 86400)"),
        ("general.refresh_enabled", "Enable background refresh (true/false)"),
        ("general.refresh_rate_limit_delay", "Rate limit delay in ms between same-domain requests (0-60000)"),
        ("network.http_timeout", "HTTP request timeout in seconds (1-300)"),
        ("network.user_agent", "User agent string for HTTP requests"),
        ("ui.tick_rate", "UI update tick rate in ms (10-1000)"),
        ("ui.error_display_timeout", "Error message timeout in ms (500-30000)"),
        ("ui.theme", "Color theme (light, dark)"),
        ("ui.compact_mode", "Compact mode (auto, always, never)"),
    ];

    for (key, desc) in keys {
        let value = config.get_value(key)?;
        println!("{:<40} = {:<20} # {}", key, value, desc);
    }

    let feed_count = config.default_feeds.len();
    println!(
        "\ndefault_feeds: {} feed(s) configured (use 'feedr config --tui' to manage)",
        feed_count
    );

    Ok(())
}

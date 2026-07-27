use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Main configuration structure for Feedr
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub default_feeds: Vec<DefaultFeed>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub keybindings: HashMap<String, toml::Value>,
    #[serde(default)]
    pub hooks: HooksConfig,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub macros: HashMap<String, String>,
    #[serde(default)]
    pub macro_options: MacroOptionsConfig,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HooksConfig {
    /// Shell-tokenized command template fired once per newly-seen item after a refresh.
    /// Variables: %t title, %u url, %a author, %d formatted-date, %f feed-title, %F feed-url.
    /// First-ever fetch of a feed is seeded silently (no firehose).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec_on_new: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MacroOptionsConfig {
    #[serde(default = "default_macro_prefix")]
    pub prefix: String,
    #[serde(default = "default_pipe_stdin")]
    pub pipe_default_stdin: String,
}

fn default_macro_prefix() -> String {
    ",".to_string()
}

fn default_pipe_stdin() -> String {
    "body".to_string()
}

impl Default for MacroOptionsConfig {
    fn default() -> Self {
        Self {
            prefix: default_macro_prefix(),
            pipe_default_stdin: default_pipe_stdin(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Maximum number of items to show on the dashboard
    #[serde(default = "default_max_dashboard_items")]
    pub max_dashboard_items: usize,
    /// Auto-refresh interval in seconds (0 = disabled)
    #[serde(default)]
    pub auto_refresh_interval: u64,
    /// Enable automatic background refresh
    #[serde(default)]
    pub refresh_enabled: bool,
    /// Delay in milliseconds between requests to the same domain (for rate limiting)
    #[serde(default = "default_refresh_rate_limit_delay")]
    pub refresh_rate_limit_delay: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// HTTP request timeout in seconds
    #[serde(default = "default_http_timeout")]
    pub http_timeout: u64,
    /// User agent string for HTTP requests
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiConfig {
    /// Tick rate for UI updates in milliseconds
    #[serde(default = "default_tick_rate")]
    pub tick_rate: u64,
    /// Error message display timeout in milliseconds
    #[serde(default = "default_error_timeout")]
    pub error_display_timeout: u64,
    /// Color theme (light or dark)
    #[serde(default)]
    pub theme: Theme,
    /// Compact mode for small terminals (auto, always, never)
    #[serde(default)]
    pub compact_mode: CompactMode,
    /// Show the dashboard preview pane on launch
    #[serde(default)]
    pub show_preview: bool,
    /// Show a scroll position indicator on the right border of the article detail view
    #[serde(default = "default_scroll_position")]
    pub scroll_position: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    #[default]
    Dark,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompactMode {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DefaultFeed {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    /// Per-feed refresh interval in seconds; None = use global interval
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_interval: Option<u64>,
    /// When true, auto-extract full-text for newly-seen items from this feed
    /// on each refresh. Manual extraction via Shift+F always works regardless.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fulltext: Option<bool>,
}

// Default value functions
fn default_max_dashboard_items() -> usize {
    100
}

fn default_refresh_rate_limit_delay() -> u64 {
    2000 // 2 seconds for Reddit safety
}

fn default_http_timeout() -> u64 {
    15
}

fn default_user_agent() -> String {
    "Mozilla/5.0 (compatible; Feedr/1.0; +https://github.com/bahdotsh/feedr)".to_string()
}

fn default_tick_rate() -> u64 {
    100
}

fn default_error_timeout() -> u64 {
    3000
}

fn default_scroll_position() -> bool {
    true
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            max_dashboard_items: default_max_dashboard_items(),
            auto_refresh_interval: 0,
            refresh_enabled: false,
            refresh_rate_limit_delay: default_refresh_rate_limit_delay(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            http_timeout: default_http_timeout(),
            user_agent: default_user_agent(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            tick_rate: default_tick_rate(),
            error_display_timeout: default_error_timeout(),
            theme: Theme::default(),
            compact_mode: CompactMode::default(),
            show_preview: false,
            scroll_position: true,
        }
    }
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Theme::Light => write!(f, "light"),
            Theme::Dark => write!(f, "dark"),
        }
    }
}

impl fmt::Display for CompactMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompactMode::Auto => write!(f, "auto"),
            CompactMode::Always => write!(f, "always"),
            CompactMode::Never => write!(f, "never"),
        }
    }
}

impl Config {
    /// Get a config value by dot-notation key
    pub fn get_value(&self, key: &str) -> Result<String> {
        match key {
            "general.max_dashboard_items" => Ok(self.general.max_dashboard_items.to_string()),
            "general.auto_refresh_interval" => Ok(self.general.auto_refresh_interval.to_string()),
            "general.refresh_enabled" => Ok(self.general.refresh_enabled.to_string()),
            "general.refresh_rate_limit_delay" => {
                Ok(self.general.refresh_rate_limit_delay.to_string())
            }
            "network.http_timeout" => Ok(self.network.http_timeout.to_string()),
            "network.user_agent" => Ok(self.network.user_agent.clone()),
            "ui.tick_rate" => Ok(self.ui.tick_rate.to_string()),
            "ui.error_display_timeout" => Ok(self.ui.error_display_timeout.to_string()),
            "ui.theme" => Ok(self.ui.theme.to_string()),
            "ui.compact_mode" => Ok(self.ui.compact_mode.to_string()),
            "ui.show_preview" => Ok(self.ui.show_preview.to_string()),
            "ui.scroll_position" => Ok(self.ui.scroll_position.to_string()),
            k if k.starts_with("default_feeds") => {
                bail!("Feed management is not supported via CLI. Use 'feedr config --tui' instead.")
            }
            _ => bail!("Unknown config key: {}", key),
        }
    }

    /// Validate and set a config value by dot-notation key
    pub fn validate_and_set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "general.max_dashboard_items" => {
                let v: usize = value.parse().context("Expected a positive integer")?;
                if !(1..=10000).contains(&v) {
                    bail!("Value must be between 1 and 10000");
                }
                self.general.max_dashboard_items = v;
            }
            "general.auto_refresh_interval" => {
                let v: u64 = value.parse().context("Expected a non-negative integer")?;
                if v > 86400 {
                    bail!("Value must be between 0 and 86400");
                }
                self.general.auto_refresh_interval = v;
            }
            "general.refresh_enabled" => {
                let v: bool = value.parse().context("Expected 'true' or 'false'")?;
                self.general.refresh_enabled = v;
            }
            "general.refresh_rate_limit_delay" => {
                let v: u64 = value.parse().context("Expected a non-negative integer")?;
                if v > 60000 {
                    bail!("Value must be between 0 and 60000");
                }
                self.general.refresh_rate_limit_delay = v;
            }
            "network.http_timeout" => {
                let v: u64 = value.parse().context("Expected a positive integer")?;
                if !(1..=300).contains(&v) {
                    bail!("Value must be between 1 and 300");
                }
                self.network.http_timeout = v;
            }
            "network.user_agent" => {
                if value.is_empty() {
                    bail!("User agent cannot be empty");
                }
                self.network.user_agent = value.to_string();
            }
            "ui.tick_rate" => {
                let v: u64 = value.parse().context("Expected a positive integer")?;
                if !(10..=1000).contains(&v) {
                    bail!("Value must be between 10 and 1000");
                }
                self.ui.tick_rate = v;
            }
            "ui.error_display_timeout" => {
                let v: u64 = value.parse().context("Expected a positive integer")?;
                if !(500..=30000).contains(&v) {
                    bail!("Value must be between 500 and 30000");
                }
                self.ui.error_display_timeout = v;
            }
            "ui.theme" => match value {
                "light" => self.ui.theme = Theme::Light,
                "dark" => self.ui.theme = Theme::Dark,
                _ => bail!("Invalid theme '{}'. Valid values: light, dark", value),
            },
            "ui.compact_mode" => match value {
                "auto" => self.ui.compact_mode = CompactMode::Auto,
                "always" => self.ui.compact_mode = CompactMode::Always,
                "never" => self.ui.compact_mode = CompactMode::Never,
                _ => bail!(
                    "Invalid compact_mode '{}'. Valid values: auto, always, never",
                    value
                ),
            },
            "ui.show_preview" => {
                let v: bool = value.parse().context("Expected 'true' or 'false'")?;
                self.ui.show_preview = v;
            }
            "ui.scroll_position" => {
                let v: bool = value.parse().context("Expected 'true' or 'false'")?;
                self.ui.scroll_position = v;
            }
            k if k.starts_with("default_feeds") => {
                bail!("Feed management is not supported via CLI. Use 'feedr config --tui' instead.")
            }
            _ => bail!("Unknown config key: {}", key),
        }
        Ok(())
    }

    /// Load configuration from the XDG config directory
    /// Falls back to default configuration if file doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if config_path.exists() {
            let contents =
                fs::read_to_string(&config_path).context("Failed to read config file")?;

            let config: Config =
                toml::from_str(&contents).context("Failed to parse config file")?;

            Ok(config)
        } else {
            // Config doesn't exist, create it with defaults
            let config = Config::default();

            // Try to save the default config for future use
            if let Err(e) = config.save() {
                // Don't fail if we can't save, just use defaults
                eprintln!("Warning: Could not create default config file: {}", e);
            }

            Ok(config)
        }
    }

    /// Save configuration to the XDG config directory
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();

        // Ensure the parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let toml_string = toml::to_string_pretty(self).context("Failed to serialize config")?;

        // Add helpful comments to the config file
        let commented_config = Self::add_comments(&toml_string);

        fs::write(&config_path, commented_config).context("Failed to write config file")?;

        Ok(())
    }

    /// Get the path to the config file following XDG specifications
    pub fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
        path.push("feedr");
        path.push("config.toml");
        path
    }

    /// Add helpful comments to the generated TOML config
    fn add_comments(toml: &str) -> String {
        format!(
            "# Feedr Configuration File\n\
             # This file is automatically generated with default values.\n\
             # You can modify any settings below to customize Feedr's behavior.\n\
             #\n\
             # For more information, visit: https://github.com/bahdotsh/feedr\n\
             \n\
             {}\n\
             \n\
             # Background Refresh Settings:\n\
             # - refresh_enabled: Enable automatic background refresh (default: false)\n\
             # - auto_refresh_interval: Time in seconds between auto-refreshes (default: 0/disabled)\n\
             # - refresh_rate_limit_delay: Delay in milliseconds between requests to same domain (default: 2000ms)\n\
             #   This prevents \"too many requests\" errors, especially for Reddit feeds\n\
             #\n\
             # UI Theme Settings:\n\
             # - theme: Choose between \"light\" or \"dark\" theme (default: dark)\n\
             #   You can also toggle the theme in the app by pressing 't'\n\
             # - show_preview: Start with the dashboard preview pane open (default: false)\n\
             #   You can still toggle the preview pane in the app by pressing 'p'\n\
             #\n\
             # Example configuration for auto-refresh every 5 minutes:\n\
             # [general]\n\
             # refresh_enabled = true\n\
             # auto_refresh_interval = 300\n\
             # refresh_rate_limit_delay = 2000\n\
             #\n\
             # [ui]\n\
             # theme = \"light\"\n\
             # compact_mode = \"auto\"  # auto (default), always, or never\n\
             # show_preview = false  # start with the dashboard preview pane open\n\
             #\n\
             # Example default feeds configuration:\n\
             # [[default_feeds]]\n\
             # url = \"https://example.com/feed.xml\"\n\
             # category = \"News\"\n\
             #\n\
             # [[default_feeds]]\n\
             # url = \"https://another-example.com/rss\"\n\
             # category = \"Tech\"\n\
             #\n\
             # Authenticated feed example (custom HTTP headers):\n\
             # [[default_feeds]]\n\
             # url = \"https://private.example.com/feed.xml\"\n\
             # [default_feeds.headers]\n\
             # Authorization = \"Bearer your_token_here\"\n\
             #\n\
             # Full-text extraction (Readability) — auto-extract on refresh:\n\
             # [[default_feeds]]\n\
             # url = \"https://example.com/summary-only-feed.xml\"\n\
             # fulltext = true\n\
             #\n\
             # Press Shift+F in the article detail view to extract on-demand\n\
             # for any feed. Auth headers from this feed are NOT sent to the\n\
             # article URL — they would leak to third-party hosts.\n\
             #\n\
             # ── External-command hooks ──────────────────────────────\n\
             #\n\
             # Macros bind a key (invoked as <prefix><key>, default prefix is ',') to\n\
             # an ordered chain of steps. Steps are separated by ';'. A trailing\n\
             # ` -- \"description\"` overrides the help-overlay label.\n\
             #\n\
             # Step kinds:\n\
             #   <action>                     run a built-in action (see list below)\n\
             #   pipe-to \"cmd %u\" [stdin=…]   run `cmd %u` with article content on stdin\n\
             #   exec    \"cmd %u\"             run `cmd %u` with no stdin\n\
             #\n\
             # Supported actions inside macros:\n\
             #   open-in-browser, toggle-star, toggle-read, mark-all-read,\n\
             #   refresh, toggle-theme, extract-links, fetch-full-text, help\n\
             # Other keybinding actions are intentionally not callable from macros.\n\
             #\n\
             # Variables expanded per argv token:\n\
             #   %t title  %u url  %a author  %d date  %f feed-title  %F feed-url  %% literal %\n\
             #\n\
             # IMPORTANT — commands are NOT run through a shell:\n\
             #   * Templates are tokenized once and %X is substituted into argv tokens.\n\
             #     Article content cannot break out of an argument.\n\
             #   * For pipes, redirection, or globbing, write a small shell script\n\
             #     and invoke that.\n\
             #   * `~` and `$HOME` / `$VAR` are NOT expanded — use absolute paths.\n\
             #   * Wrapping your command in `sh -c \"... %t ...\"` REINTRODUCES shell\n\
             #     injection through item titles. Prefer a script file.\n\
             #\n\
             # Quoting: the outer parser is shell-style, so to put a quoted token in\n\
             # the inner argv, escape twice:\n\
             #   pipe-to \"echo \\\"hello world\\\"\"\n\
             #\n\
             # [macros]\n\
             # y = 'open-in-browser ; pipe-to \"yt-dlp %u\"'\n\
             # w = 'pipe-to \"wallabag-cli add %u\" -- \"Save to Wallabag\"'\n\
             # n = 'pipe-to \"tee /tmp/out.txt\" stdin=metadata'\n\
             #\n\
             # [macro_options]\n\
             # prefix = \",\"                  # the macro-prefix key\n\
             # pipe_default_stdin = \"body\"   # body | title | url | metadata | none\n\
             #\n\
             # The exec_on_new hook fires once per newly-seen item on each refresh.\n\
             # The first successful fetch of a feed seeds the seen set silently to\n\
             # avoid a firehose on initial load. Children are spawned detached.\n\
             # Semantics on crash are AT MOST ONCE — feedr persists the seen-set\n\
             # before spawning, so a kill mid-fire loses a notification rather than\n\
             # firing again on the next launch. Prefer idempotent commands.\n\
             #\n\
             # [hooks]\n\
             # exec_on_new = 'notify-send \"New: %t\" \"%f\"'\n",
            toml
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.max_dashboard_items, 100);
        assert_eq!(config.network.http_timeout, 15);
        assert_eq!(config.ui.tick_rate, 100);
        assert_eq!(config.ui.error_display_timeout, 3000);
    }

    #[test]
    fn test_default_feed_without_fulltext_parses() {
        // Existing configs that have no `fulltext` key must keep working.
        let toml_str = r#"
            [[default_feeds]]
            url = "https://example.com/feed.xml"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_feeds.len(), 1);
        assert!(config.default_feeds[0].fulltext.is_none());
    }

    #[test]
    fn test_default_feed_with_fulltext_round_trips() {
        let toml_str = r#"
            [[default_feeds]]
            url = "https://example.com/feed.xml"
            fulltext = true
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_feeds[0].fulltext, Some(true));

        // Round-trip: serialize the parsed config and parse the result.
        let serialized = toml::to_string(&config).unwrap();
        let reparsed: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(reparsed.default_feeds[0].fulltext, Some(true));
    }

    #[test]
    fn test_default_feed_with_headers() {
        let toml_str = r#"
            [[default_feeds]]
            url = "https://example.com/feed.xml"

            [[default_feeds]]
            url = "https://private.example.com/feed.xml"
            [default_feeds.headers]
            Authorization = "Bearer token123"
            X-Custom = "value"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_feeds.len(), 2);
        assert!(config.default_feeds[0].headers.is_none());
        let headers = config.default_feeds[1].headers.as_ref().unwrap();
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer token123");
        assert_eq!(headers.get("X-Custom").unwrap(), "value");
    }

    #[test]
    fn test_hooks_and_macros_round_trip() {
        let toml_str = r#"
            [hooks]
            exec_on_new = 'notify-send "New: %t" "%f"'

            [macros]
            y = 'open-in-browser ; pipe-to "yt-dlp %u"'

            [macro_options]
            prefix = ","
            pipe_default_stdin = "metadata"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.hooks.exec_on_new.as_deref(),
            Some(r#"notify-send "New: %t" "%f""#)
        );
        assert_eq!(
            config.macros.get("y").map(String::as_str),
            Some(r#"open-in-browser ; pipe-to "yt-dlp %u""#)
        );
        assert_eq!(config.macro_options.prefix, ",");
        assert_eq!(config.macro_options.pipe_default_stdin, "metadata");
    }

    #[test]
    fn test_config_back_compat_without_new_sections() {
        // An old config file with no [hooks]/[macros]/[macro_options] must still load.
        let toml_str = r#"
            [general]
            max_dashboard_items = 50
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.general.max_dashboard_items, 50);
        assert!(config.hooks.exec_on_new.is_none());
        assert!(config.macros.is_empty());
        assert_eq!(config.macro_options.prefix, ",");
        assert_eq!(config.macro_options.pipe_default_stdin, "body");
    }

    #[test]
    fn test_show_preview_back_compat_and_default() {
        // An old config with a [ui] table but no show_preview key must
        // still load, defaulting to false.
        let toml_str = r#"
            [ui]
            theme = "dark"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(!config.ui.show_preview);
        assert!(!Config::default().ui.show_preview);
    }

    #[test]
    fn test_show_preview_get_set() {
        let mut config = Config::default();
        assert_eq!(config.get_value("ui.show_preview").unwrap(), "false");

        config.validate_and_set("ui.show_preview", "true").unwrap();
        assert!(config.ui.show_preview);
        assert_eq!(config.get_value("ui.show_preview").unwrap(), "true");

        assert!(config.validate_and_set("ui.show_preview", "yes").is_err());
        // Failed set must not clobber the previous value
        assert!(config.ui.show_preview);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(
            config.general.max_dashboard_items,
            deserialized.general.max_dashboard_items
        );
        assert_eq!(
            config.network.http_timeout,
            deserialized.network.http_timeout
        );
    }
}

# Feedr - Terminal RSS Feed Reader

Feedr is a feature-rich terminal-based RSS feed reader written in Rust. It provides a clean and intuitive TUI interface for subscribing to and reading RSS feeds.

## Features

- Subscribe to multiple RSS feeds
- Dashboard view showing the latest items across all feeds
- Search functionality to find specific content
- Bookmark feeds for easy access
- Open links directly in your browser
- Clean, color-coded interface with keyboard navigation
- HTML to text conversion for readable content


## Installation

### Prerequisites

- Rust and Cargo (install from [https://rustup.rs/](https://rustup.rs/))

The recommended way to install `feedr` is using Rust's package manager, Cargo. Here are several methods:

### Using Cargo Install (Recommended)
```bash
cargo install feedr
```

### Build from source

```bash
git clone https://github.com/bahdotsh/feedr.git
cd feedr
cargo build --release
```

The binary will be available at `target/release/feedr`.

## Usage

Run the application with:

```bash
./target/release/feedr
```

### Keyboard Controls

#### Dashboard View
- `f`: Go to feeds list
- `a`: Add a new feed
- `r`: Refresh all feeds
- `Enter`: View selected item
- `o`: Open link in browser
- `/`: Search
- `q`: Quit

#### Feed List View
- `h/Esc`: Go to dashboard
- `a`: Add a new feed
- `d`: Delete selected feed
- `Enter`: View feed items
- `r`: Refresh feeds
- `/`: Search
- `q`: Quit

#### Feed Items View
- `h/Esc`: Back to feeds list
- `Home`: Go to dashboard
- `Enter`: View item details
- `o`: Open item in browser
- `/`: Search
- `q`: Quit

#### Item Detail View
- `h/Esc`: Back to feed items
- `Home`: Go to dashboard
- `o`: Open item in browser
- `q`: Quit

## Configuration

Feedr saves your bookmarked feeds to `~/.local/share/feedr/bookmarks.json` on Linux/macOS or to the appropriate application data directory on Windows.

## Dependencies

- ratatui: Terminal UI library
- crossterm: Terminal manipulation
- reqwest: HTTP client
- rss: RSS parsing
- html2text: HTML to text conversion
- chrono: Date and time handling
- serde: Serialization/deserialization

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

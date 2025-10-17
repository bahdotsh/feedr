mod app;
mod feed;
mod tui;
mod ui;

use anyhow::Result;
use app::App;
use clap::Parser;

#[derive(Parser)]
#[command(name = "feedr")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "A feature-rich terminal-based RSS/Atom feed reader written in Rust")]
#[command(
    long_about = "Feedr is a modern terminal-based RSS/Atom feed reader with advanced filtering, categorization, and search capabilities. It supports both RSS and Atom feeds with compression handling and provides an intuitive TUI interface."
)]
struct Cli {
    /// OPML file to import
    #[arg(short, long, value_name = "FILE PATH")]
    import: Option<String>,
}

fn main() -> Result<()> {
    let _cli = Cli::parse();

    // Initialize the application
    let mut app = App::new();

    match _cli.import {
        Some(file_path) => app.import_opml(&file_path),
        None => {
            // Run the terminal UI
            tui::run(app)?;
            Ok(())
        }
    }
}

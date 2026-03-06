use anyhow::Result;
use clap::{Parser, Subcommand};
use feedr::app::App;
use feedr::{config_cli, config_tui, tui};

#[derive(Parser)]
#[command(name = "feedr")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "A feature-rich terminal-based RSS/Atom feed reader written in Rust")]
#[command(
    long_about = "Feedr is a modern terminal-based RSS/Atom feed reader with advanced filtering, categorization, and search capabilities. It supports both RSS and Atom feeds with compression handling and provides an intuitive TUI interface."
)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    /// OPML file to import
    #[arg(short, long, value_name = "FILE PATH")]
    import: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,

        /// Open interactive TUI config editor
        #[arg(long)]
        tui: bool,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Get a config value by key
    Get {
        /// Config key in dot-notation (e.g. ui.theme)
        key: String,
    },
    /// Set a config value
    Set {
        /// Config key in dot-notation (e.g. ui.theme)
        key: String,
        /// New value
        value: String,
    },
    /// List all config keys and values
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config { action, tui: use_tui }) => {
            if use_tui {
                return config_tui::run();
            }
            match action {
                Some(ConfigAction::Get { key }) => config_cli::get(&key),
                Some(ConfigAction::Set { key, value }) => config_cli::set(&key, &value),
                Some(ConfigAction::List) | None => config_cli::list(),
            }
        }
        None => match cli.import {
            Some(file_path) => {
                let mut app = App::new();
                app.import_opml(&file_path)
            }
            None => {
                let app = App::new();
                tui::run(app)?;
                Ok(())
            }
        },
    }
}

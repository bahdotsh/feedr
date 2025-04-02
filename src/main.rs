mod app;
mod feed;
mod tui;
mod ui;

use anyhow::Result;
use app::App;

fn main() -> Result<()> {
    // Initialize the application
    let app = App::new();

    // Run the terminal UI
    tui::run(app)?;

    Ok(())
}


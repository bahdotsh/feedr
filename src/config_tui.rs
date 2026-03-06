use crate::config::{Config, DefaultFeed};
use crate::config_ui;
use crate::ui::ColorScheme;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConfigSection {
    General,
    Network,
    Ui,
    DefaultFeeds,
}

impl ConfigSection {
    pub const ALL: [ConfigSection; 4] = [
        ConfigSection::General,
        ConfigSection::Network,
        ConfigSection::Ui,
        ConfigSection::DefaultFeeds,
    ];

    pub fn title(&self) -> &str {
        match self {
            ConfigSection::General => "General",
            ConfigSection::Network => "Network",
            ConfigSection::Ui => "UI",
            ConfigSection::DefaultFeeds => "Default Feeds",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            ConfigSection::General => 0,
            ConfigSection::Network => 1,
            ConfigSection::Ui => 2,
            ConfigSection::DefaultFeeds => 3,
        }
    }

    pub fn field_count(&self, config: &Config) -> usize {
        match self {
            ConfigSection::General => 4,
            ConfigSection::Network => 2,
            ConfigSection::Ui => 4,
            ConfigSection::DefaultFeeds => config.default_feeds.len().max(1),
        }
    }
}

pub enum FieldKind {
    Text,
    Bool,
    Enum,
}

pub struct FieldInfo {
    pub key: String,
    pub label: String,
    pub value: String,
    pub kind: FieldKind,
    pub description: String,
}

pub fn get_fields(section: ConfigSection, config: &Config) -> Vec<FieldInfo> {
    match section {
        ConfigSection::General => vec![
            FieldInfo {
                key: "general.max_dashboard_items".into(),
                label: "Max Dashboard Items".into(),
                value: config.general.max_dashboard_items.to_string(),
                kind: FieldKind::Text,
                description: "1-10000".into(),
            },
            FieldInfo {
                key: "general.auto_refresh_interval".into(),
                label: "Auto Refresh Interval".into(),
                value: config.general.auto_refresh_interval.to_string(),
                kind: FieldKind::Text,
                description: "Seconds (0=disabled, max 86400)".into(),
            },
            FieldInfo {
                key: "general.refresh_enabled".into(),
                label: "Refresh Enabled".into(),
                value: config.general.refresh_enabled.to_string(),
                kind: FieldKind::Bool,
                description: "true/false".into(),
            },
            FieldInfo {
                key: "general.refresh_rate_limit_delay".into(),
                label: "Rate Limit Delay".into(),
                value: config.general.refresh_rate_limit_delay.to_string(),
                kind: FieldKind::Text,
                description: "Milliseconds (0-60000)".into(),
            },
        ],
        ConfigSection::Network => vec![
            FieldInfo {
                key: "network.http_timeout".into(),
                label: "HTTP Timeout".into(),
                value: config.network.http_timeout.to_string(),
                kind: FieldKind::Text,
                description: "Seconds (1-300)".into(),
            },
            FieldInfo {
                key: "network.user_agent".into(),
                label: "User Agent".into(),
                value: config.network.user_agent.clone(),
                kind: FieldKind::Text,
                description: "Non-empty string".into(),
            },
        ],
        ConfigSection::Ui => vec![
            FieldInfo {
                key: "ui.tick_rate".into(),
                label: "Tick Rate".into(),
                value: config.ui.tick_rate.to_string(),
                kind: FieldKind::Text,
                description: "Milliseconds (10-1000)".into(),
            },
            FieldInfo {
                key: "ui.error_display_timeout".into(),
                label: "Error Display Timeout".into(),
                value: config.ui.error_display_timeout.to_string(),
                kind: FieldKind::Text,
                description: "Milliseconds (500-30000)".into(),
            },
            FieldInfo {
                key: "ui.theme".into(),
                label: "Theme".into(),
                value: config.ui.theme.to_string(),
                kind: FieldKind::Enum,
                description: "light, dark".into(),
            },
            FieldInfo {
                key: "ui.compact_mode".into(),
                label: "Compact Mode".into(),
                value: config.ui.compact_mode.to_string(),
                kind: FieldKind::Enum,
                description: "auto, always, never".into(),
            },
        ],
        ConfigSection::DefaultFeeds => {
            if config.default_feeds.is_empty() {
                vec![FieldInfo {
                    key: String::new(),
                    label: "(no feeds configured — press 'a' to add)".into(),
                    value: String::new(),
                    kind: FieldKind::Text,
                    description: String::new(),
                }]
            } else {
                config
                    .default_feeds
                    .iter()
                    .enumerate()
                    .map(|(i, feed)| {
                        let cat = feed
                            .category
                            .as_deref()
                            .map(|c| format!(" [{}]", c))
                            .unwrap_or_default();
                        let hdr = if feed.headers.is_some() {
                            " (auth)"
                        } else {
                            ""
                        };
                        FieldInfo {
                            key: format!("default_feeds.{}", i),
                            label: format!("Feed #{}", i + 1),
                            value: format!("{}{}{}", feed.url, cat, hdr),
                            kind: FieldKind::Text,
                            description: String::new(),
                        }
                    })
                    .collect()
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConfirmChoice {
    Save,
    Discard,
    Cancel,
}

pub struct ConfigEditor {
    pub config: Config,
    pub color_scheme: ColorScheme,
    pub section: ConfigSection,
    pub selected_field: usize,
    pub editing: bool,
    pub edit_buffer: String,
    pub edit_cursor: usize,
    pub dirty: bool,
    pub error: Option<String>,
    pub success: Option<String>,
    pub confirm_quit: bool,
    pub confirm_selection: ConfirmChoice,
    // For adding feeds
    pub adding_feed: bool,
    pub add_url_buffer: String,
    pub add_category_buffer: String,
    pub add_field_focus: usize, // 0=url, 1=category, 2=confirm
    pub add_cursor: usize,
}

impl ConfigEditor {
    pub fn new(config: Config) -> Self {
        let color_scheme = ColorScheme::from_theme(&config.ui.theme);
        Self {
            config,
            color_scheme,
            section: ConfigSection::General,
            selected_field: 0,
            editing: false,
            edit_buffer: String::new(),
            edit_cursor: 0,
            dirty: false,
            error: None,
            success: None,
            confirm_quit: false,
            confirm_selection: ConfirmChoice::Save,
            adding_feed: false,
            add_url_buffer: String::new(),
            add_category_buffer: String::new(),
            add_field_focus: 0,
            add_cursor: 0,
        }
    }

    fn field_count(&self) -> usize {
        self.section.field_count(&self.config)
    }

    fn current_field_info(&self) -> Option<FieldInfo> {
        let fields = get_fields(self.section, &self.config);
        fields.into_iter().nth(self.selected_field)
    }

    fn toggle_bool_or_cycle_enum(&mut self) {
        let Some(field) = self.current_field_info() else {
            return;
        };
        match field.kind {
            FieldKind::Bool => {
                let new_val = if field.value == "true" {
                    "false"
                } else {
                    "true"
                };
                if let Err(e) = self.config.validate_and_set(&field.key, new_val) {
                    self.error = Some(e.to_string());
                } else {
                    self.dirty = true;
                    self.success = Some(format!("Set {} = {}", field.label, new_val));
                }
            }
            FieldKind::Enum => {
                let new_val = match field.key.as_str() {
                    "ui.theme" => {
                        if field.value == "dark" {
                            "light"
                        } else {
                            "dark"
                        }
                    }
                    "ui.compact_mode" => match field.value.as_str() {
                        "auto" => "always",
                        "always" => "never",
                        _ => "auto",
                    },
                    _ => return,
                };
                if let Err(e) = self.config.validate_and_set(&field.key, new_val) {
                    self.error = Some(e.to_string());
                } else {
                    self.dirty = true;
                    self.success = Some(format!("Set {} = {}", field.label, new_val));
                    if field.key == "ui.theme" {
                        self.color_scheme = ColorScheme::from_theme(&self.config.ui.theme);
                    }
                }
            }
            FieldKind::Text => {}
        }
    }

    fn start_edit(&mut self) {
        if self.section == ConfigSection::DefaultFeeds {
            return;
        }
        let Some(field) = self.current_field_info() else {
            return;
        };
        match field.kind {
            FieldKind::Bool | FieldKind::Enum => {
                self.toggle_bool_or_cycle_enum();
            }
            FieldKind::Text => {
                self.editing = true;
                self.edit_buffer = field.value.clone();
                self.edit_cursor = self.edit_buffer.len();
            }
        }
    }

    fn confirm_edit(&mut self) {
        let Some(field) = self.current_field_info() else {
            self.editing = false;
            return;
        };
        match self.config.validate_and_set(&field.key, &self.edit_buffer) {
            Ok(()) => {
                self.dirty = true;
                self.editing = false;
                self.success = Some(format!("Set {} = {}", field.label, self.edit_buffer));
                self.error = None;
            }
            Err(e) => {
                self.error = Some(e.to_string());
            }
        }
    }

    fn save(&mut self) {
        match self.config.save() {
            Ok(()) => {
                self.dirty = false;
                self.success = Some("Configuration saved.".into());
                self.error = None;
            }
            Err(e) => {
                self.error = Some(format!("Failed to save: {}", e));
            }
        }
    }

    fn add_feed(&mut self) {
        if self.add_url_buffer.trim().is_empty() {
            self.error = Some("Feed URL cannot be empty".into());
            return;
        }
        let url = self.add_url_buffer.trim().to_string();
        let category = if self.add_category_buffer.trim().is_empty() {
            None
        } else {
            Some(self.add_category_buffer.trim().to_string())
        };
        self.config.default_feeds.push(DefaultFeed {
            url,
            category,
            headers: None,
        });
        self.dirty = true;
        self.adding_feed = false;
        self.add_url_buffer.clear();
        self.add_category_buffer.clear();
        self.add_field_focus = 0;
        self.add_cursor = 0;
        self.success = Some("Feed added.".into());
    }

    fn delete_feed(&mut self) {
        if self.section != ConfigSection::DefaultFeeds || self.config.default_feeds.is_empty() {
            return;
        }
        if self.selected_field < self.config.default_feeds.len() {
            self.config.default_feeds.remove(self.selected_field);
            self.dirty = true;
            if self.selected_field > 0
                && self.selected_field >= self.config.default_feeds.len().max(1)
            {
                self.selected_field = self.selected_field.saturating_sub(1);
            }
            self.success = Some("Feed removed.".into());
        }
    }
}

pub fn run() -> Result<()> {
    let config = Config::load()?;
    let mut editor = ConfigEditor::new(config);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_editor(&mut terminal, &mut editor);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    editor: &mut ConfigEditor,
) -> Result<()> {
    loop {
        terminal.draw(|f| config_ui::render(f, editor))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Dismiss error/success messages on any key
                if editor.error.is_some() && !editor.editing && !editor.adding_feed {
                    editor.error = None;
                    continue;
                }
                if editor.success.is_some() && !editor.editing && !editor.adding_feed {
                    editor.success = None;
                    continue;
                }

                // Confirm quit dialog
                if editor.confirm_quit {
                    match key.code {
                        KeyCode::Left | KeyCode::Char('h') => {
                            editor.confirm_selection = match editor.confirm_selection {
                                ConfirmChoice::Save => ConfirmChoice::Save,
                                ConfirmChoice::Discard => ConfirmChoice::Save,
                                ConfirmChoice::Cancel => ConfirmChoice::Discard,
                            };
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            editor.confirm_selection = match editor.confirm_selection {
                                ConfirmChoice::Save => ConfirmChoice::Discard,
                                ConfirmChoice::Discard => ConfirmChoice::Cancel,
                                ConfirmChoice::Cancel => ConfirmChoice::Cancel,
                            };
                        }
                        KeyCode::Enter => match editor.confirm_selection {
                            ConfirmChoice::Save => {
                                editor.save();
                                return Ok(());
                            }
                            ConfirmChoice::Discard => return Ok(()),
                            ConfirmChoice::Cancel => {
                                editor.confirm_quit = false;
                                editor.confirm_selection = ConfirmChoice::Save;
                            }
                        },
                        KeyCode::Esc => {
                            editor.confirm_quit = false;
                            editor.confirm_selection = ConfirmChoice::Save;
                        }
                        _ => {}
                    }
                    continue;
                }

                // Add feed dialog
                if editor.adding_feed {
                    match key.code {
                        KeyCode::Esc => {
                            editor.adding_feed = false;
                            editor.add_url_buffer.clear();
                            editor.add_category_buffer.clear();
                            editor.add_field_focus = 0;
                            editor.add_cursor = 0;
                        }
                        KeyCode::Tab => {
                            editor.add_field_focus = (editor.add_field_focus + 1) % 3;
                            editor.add_cursor = match editor.add_field_focus {
                                0 => editor.add_url_buffer.len(),
                                1 => editor.add_category_buffer.len(),
                                _ => 0,
                            };
                        }
                        KeyCode::BackTab => {
                            editor.add_field_focus = (editor.add_field_focus + 2) % 3;
                            editor.add_cursor = match editor.add_field_focus {
                                0 => editor.add_url_buffer.len(),
                                1 => editor.add_category_buffer.len(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if editor.add_field_focus == 2 {
                                editor.add_feed();
                            } else {
                                editor.add_field_focus = (editor.add_field_focus + 1) % 3;
                                editor.add_cursor = match editor.add_field_focus {
                                    0 => editor.add_url_buffer.len(),
                                    1 => editor.add_category_buffer.len(),
                                    _ => 0,
                                };
                            }
                        }
                        KeyCode::Char(c) if editor.add_field_focus < 2 => {
                            let buf = if editor.add_field_focus == 0 {
                                &mut editor.add_url_buffer
                            } else {
                                &mut editor.add_category_buffer
                            };
                            buf.insert(editor.add_cursor, c);
                            editor.add_cursor += 1;
                        }
                        KeyCode::Backspace if editor.add_field_focus < 2 => {
                            if editor.add_cursor > 0 {
                                let buf = if editor.add_field_focus == 0 {
                                    &mut editor.add_url_buffer
                                } else {
                                    &mut editor.add_category_buffer
                                };
                                editor.add_cursor -= 1;
                                buf.remove(editor.add_cursor);
                            }
                        }
                        KeyCode::Left if editor.add_field_focus < 2 => {
                            editor.add_cursor = editor.add_cursor.saturating_sub(1);
                        }
                        KeyCode::Right if editor.add_field_focus < 2 => {
                            let len = if editor.add_field_focus == 0 {
                                editor.add_url_buffer.len()
                            } else {
                                editor.add_category_buffer.len()
                            };
                            if editor.add_cursor < len {
                                editor.add_cursor += 1;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }

                // Edit mode
                if editor.editing {
                    match key.code {
                        KeyCode::Esc => {
                            editor.editing = false;
                            editor.error = None;
                        }
                        KeyCode::Enter => {
                            editor.confirm_edit();
                        }
                        KeyCode::Char(c) => {
                            editor.edit_buffer.insert(editor.edit_cursor, c);
                            editor.edit_cursor += 1;
                        }
                        KeyCode::Backspace => {
                            if editor.edit_cursor > 0 {
                                editor.edit_cursor -= 1;
                                editor.edit_buffer.remove(editor.edit_cursor);
                            }
                        }
                        KeyCode::Left => {
                            editor.edit_cursor = editor.edit_cursor.saturating_sub(1);
                        }
                        KeyCode::Right => {
                            if editor.edit_cursor < editor.edit_buffer.len() {
                                editor.edit_cursor += 1;
                            }
                        }
                        KeyCode::Home => {
                            editor.edit_cursor = 0;
                        }
                        KeyCode::End => {
                            editor.edit_cursor = editor.edit_buffer.len();
                        }
                        _ => {}
                    }
                    continue;
                }

                // Normal mode
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        if editor.dirty {
                            editor.confirm_quit = true;
                            editor.confirm_selection = ConfirmChoice::Save;
                        } else {
                            return Ok(());
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        let count = editor.field_count();
                        if count > 0 && editor.selected_field < count - 1 {
                            editor.selected_field += 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        editor.selected_field = editor.selected_field.saturating_sub(1);
                    }
                    KeyCode::Tab => {
                        let sections = ConfigSection::ALL;
                        let idx = editor.section.index();
                        editor.section = sections[(idx + 1) % sections.len()];
                        editor.selected_field = 0;
                    }
                    KeyCode::BackTab => {
                        let sections = ConfigSection::ALL;
                        let idx = editor.section.index();
                        editor.section = sections[(idx + sections.len() - 1) % sections.len()];
                        editor.selected_field = 0;
                    }
                    KeyCode::Enter | KeyCode::Char('e') => {
                        editor.start_edit();
                    }
                    KeyCode::Char(' ') => {
                        editor.toggle_bool_or_cycle_enum();
                    }
                    KeyCode::Char('s') => {
                        editor.save();
                    }
                    KeyCode::Char('a') if editor.section == ConfigSection::DefaultFeeds => {
                        editor.adding_feed = true;
                        editor.add_url_buffer.clear();
                        editor.add_category_buffer.clear();
                        editor.add_field_focus = 0;
                        editor.add_cursor = 0;
                        editor.error = None;
                    }
                    KeyCode::Char('d') if editor.section == ConfigSection::DefaultFeeds => {
                        editor.delete_feed();
                    }
                    _ => {}
                }
            }
        }
    }
}

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::str::FromStr;

/// What gets piped to stdin when a `pipe-to` macro step runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StdinKind {
    None,
    #[default]
    Body,
    Title,
    Url,
    Metadata,
}

impl FromStr for StdinKind {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "body" => Ok(Self::Body),
            "title" => Ok(Self::Title),
            "url" => Ok(Self::Url),
            "metadata" => Ok(Self::Metadata),
            _ => Err(()),
        }
    }
}

/// A single step inside a macro chain.
#[derive(Clone, Debug)]
pub enum MacroStep {
    /// Invoke an existing keybinding action (e.g. open-in-browser, toggle-star).
    Action(KeyAction),
    /// Run argv with article content piped to stdin.
    PipeTo {
        argv_template: Vec<String>,
        stdin: StdinKind,
    },
    /// Run argv with no stdin.
    Exec { argv_template: Vec<String> },
}

/// A macro: a key trigger + an ordered sequence of steps + optional description.
#[derive(Clone, Debug)]
pub struct MacroBinding {
    pub trigger: KeyBinding,
    pub steps: Vec<MacroStep>,
    pub description: Option<String>,
}

/// Top-level options for the macro engine.
#[derive(Clone, Debug)]
pub struct MacroOptions {
    pub prefix: KeyBinding,
    pub pipe_default_stdin: StdinKind,
}

impl Default for MacroOptions {
    fn default() -> Self {
        Self {
            prefix: KeyBinding::new(KeyCode::Char(',')),
            pipe_default_stdin: StdinKind::Body,
        }
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum KeyAction {
    // Global
    Quit,
    ForceQuit,
    Back,
    Home,
    ToggleTheme,
    Refresh,
    Help,
    OpenSearch,
    // Navigation
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    JumpTop,
    JumpBottom,
    Select,
    // Item actions
    AddFeed,
    DeleteFeed,
    ToggleRead,
    ToggleStar,
    MarkAllRead,
    OpenInBrowser,
    TogglePreview,
    // Filter/Category
    OpenFilter,
    CycleCategory,
    OpenCategoryManagement,
    AssignCategory,
    // Detail
    ExtractLinks,
    FetchFullText,
    ScrollPreviewUp,
    ScrollPreviewDown,
    // In-article find (detail view only)
    OpenArticleSearch,
    NextMatch,
    PrevMatch,
    // Tree
    ToggleExpand,
    // Tab
    NextTab,
    PrevTab,
}

impl KeyAction {
    /// Return the dashed lowercase name a user would write in their config
    /// (the inverse of `FromStr`, modulo `-` vs `_`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Quit => "quit",
            Self::ForceQuit => "force-quit",
            Self::Back => "back",
            Self::Home => "home",
            Self::ToggleTheme => "toggle-theme",
            Self::Refresh => "refresh",
            Self::Help => "help",
            Self::OpenSearch => "open-search",
            Self::MoveUp => "move-up",
            Self::MoveDown => "move-down",
            Self::PageUp => "page-up",
            Self::PageDown => "page-down",
            Self::JumpTop => "jump-top",
            Self::JumpBottom => "jump-bottom",
            Self::Select => "select",
            Self::AddFeed => "add-feed",
            Self::DeleteFeed => "delete-feed",
            Self::ToggleRead => "toggle-read",
            Self::ToggleStar => "toggle-star",
            Self::MarkAllRead => "mark-all-read",
            Self::OpenInBrowser => "open-in-browser",
            Self::TogglePreview => "toggle-preview",
            Self::OpenFilter => "open-filter",
            Self::CycleCategory => "cycle-category",
            Self::OpenCategoryManagement => "open-category-management",
            Self::AssignCategory => "assign-category",
            Self::ExtractLinks => "extract-links",
            Self::FetchFullText => "fetch-full-text",
            Self::ScrollPreviewUp => "scroll-preview-up",
            Self::ScrollPreviewDown => "scroll-preview-down",
            Self::OpenArticleSearch => "open-article-search",
            Self::NextMatch => "next-match",
            Self::PrevMatch => "prev-match",
            Self::ToggleExpand => "toggle-expand",
            Self::NextTab => "next-tab",
            Self::PrevTab => "prev-tab",
        }
    }
}

impl FromStr for KeyAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "quit" => Ok(Self::Quit),
            "force_quit" => Ok(Self::ForceQuit),
            "back" => Ok(Self::Back),
            "home" => Ok(Self::Home),
            "toggle_theme" => Ok(Self::ToggleTheme),
            "refresh" => Ok(Self::Refresh),
            "help" => Ok(Self::Help),
            "open_search" => Ok(Self::OpenSearch),
            "move_up" => Ok(Self::MoveUp),
            "move_down" => Ok(Self::MoveDown),
            "page_up" => Ok(Self::PageUp),
            "page_down" => Ok(Self::PageDown),
            "jump_top" => Ok(Self::JumpTop),
            "jump_bottom" => Ok(Self::JumpBottom),
            "select" => Ok(Self::Select),
            "add_feed" => Ok(Self::AddFeed),
            "delete_feed" => Ok(Self::DeleteFeed),
            "toggle_read" => Ok(Self::ToggleRead),
            "toggle_star" => Ok(Self::ToggleStar),
            "mark_all_read" => Ok(Self::MarkAllRead),
            "open_in_browser" => Ok(Self::OpenInBrowser),
            "toggle_preview" => Ok(Self::TogglePreview),
            "open_filter" => Ok(Self::OpenFilter),
            "cycle_category" => Ok(Self::CycleCategory),
            "open_category_management" => Ok(Self::OpenCategoryManagement),
            "assign_category" => Ok(Self::AssignCategory),
            "extract_links" => Ok(Self::ExtractLinks),
            "fetch_full_text" => Ok(Self::FetchFullText),
            "scroll_preview_up" => Ok(Self::ScrollPreviewUp),
            "scroll_preview_down" => Ok(Self::ScrollPreviewDown),
            "open_article_search" => Ok(Self::OpenArticleSearch),
            "next_match" => Ok(Self::NextMatch),
            "prev_match" => Ok(Self::PrevMatch),
            "toggle_expand" => Ok(Self::ToggleExpand),
            "next_tab" => Ok(Self::NextTab),
            "prev_tab" => Ok(Self::PrevTab),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    pub fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn with_ctrl(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::CONTROL,
        }
    }

    pub fn with_shift(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::SHIFT,
        }
    }

    pub fn matches(&self, key: &KeyEvent) -> bool {
        self.code == key.code && key.modifiers.contains(self.modifiers)
    }
}

pub type KeyBindingMap = HashMap<KeyAction, Vec<KeyBinding>>;

pub fn default_keybindings() -> KeyBindingMap {
    let mut map = KeyBindingMap::new();

    // Global
    map.insert(KeyAction::Quit, vec![KeyBinding::new(KeyCode::Char('q'))]);
    map.insert(
        KeyAction::ForceQuit,
        vec![KeyBinding::with_ctrl(KeyCode::Char('q'))],
    );
    map.insert(
        KeyAction::Back,
        vec![
            KeyBinding::new(KeyCode::Char('h')),
            KeyBinding::new(KeyCode::Esc),
            KeyBinding::new(KeyCode::Backspace),
        ],
    );
    map.insert(KeyAction::Home, vec![KeyBinding::new(KeyCode::Home)]);
    map.insert(
        KeyAction::ToggleTheme,
        vec![KeyBinding::new(KeyCode::Char('t'))],
    );
    map.insert(
        KeyAction::Refresh,
        vec![KeyBinding::new(KeyCode::Char('r'))],
    );
    map.insert(KeyAction::Help, vec![KeyBinding::new(KeyCode::Char('?'))]);
    map.insert(
        KeyAction::OpenSearch,
        vec![KeyBinding::new(KeyCode::Char('/'))],
    );

    // Navigation
    map.insert(
        KeyAction::MoveUp,
        vec![
            KeyBinding::new(KeyCode::Up),
            KeyBinding::new(KeyCode::Char('k')),
        ],
    );
    map.insert(
        KeyAction::MoveDown,
        vec![
            KeyBinding::new(KeyCode::Down),
            KeyBinding::new(KeyCode::Char('j')),
        ],
    );
    map.insert(
        KeyAction::PageUp,
        vec![
            KeyBinding::new(KeyCode::PageUp),
            KeyBinding::with_ctrl(KeyCode::Char('u')),
        ],
    );
    map.insert(
        KeyAction::PageDown,
        vec![
            KeyBinding::new(KeyCode::PageDown),
            KeyBinding::with_ctrl(KeyCode::Char('d')),
        ],
    );
    map.insert(
        KeyAction::JumpTop,
        vec![KeyBinding::new(KeyCode::Char('g'))],
    );
    map.insert(
        KeyAction::JumpBottom,
        vec![
            KeyBinding::new(KeyCode::Char('G')),
            KeyBinding::new(KeyCode::End),
        ],
    );
    map.insert(KeyAction::Select, vec![KeyBinding::new(KeyCode::Enter)]);

    // Item actions
    map.insert(
        KeyAction::AddFeed,
        vec![KeyBinding::new(KeyCode::Char('a'))],
    );
    map.insert(
        KeyAction::DeleteFeed,
        vec![KeyBinding::new(KeyCode::Char('d'))],
    );
    map.insert(
        KeyAction::ToggleRead,
        vec![KeyBinding::new(KeyCode::Char(' '))],
    );
    map.insert(
        KeyAction::ToggleStar,
        vec![KeyBinding::new(KeyCode::Char('s'))],
    );
    map.insert(
        KeyAction::MarkAllRead,
        vec![KeyBinding::new(KeyCode::Char('m'))],
    );
    map.insert(
        KeyAction::OpenInBrowser,
        vec![KeyBinding::new(KeyCode::Char('o'))],
    );
    map.insert(
        KeyAction::TogglePreview,
        vec![KeyBinding::new(KeyCode::Char('p'))],
    );

    // Filter/Category
    map.insert(
        KeyAction::OpenFilter,
        vec![KeyBinding::new(KeyCode::Char('f'))],
    );
    map.insert(
        KeyAction::CycleCategory,
        vec![KeyBinding::new(KeyCode::Char('c'))],
    );
    map.insert(
        KeyAction::OpenCategoryManagement,
        vec![KeyBinding::with_ctrl(KeyCode::Char('c'))],
    );
    map.insert(
        KeyAction::AssignCategory,
        vec![KeyBinding::new(KeyCode::Char('c'))],
    );

    // Detail
    map.insert(
        KeyAction::ExtractLinks,
        vec![KeyBinding::new(KeyCode::Char('l'))],
    );
    // Capital F (Shift+F). Matches both crossterm's `Char('F')` events and
    // `Char('F')+SHIFT` thanks to `KeyBinding::matches` using a subset check
    // on modifiers — same pattern as `JumpBottom` (G).
    map.insert(
        KeyAction::FetchFullText,
        vec![KeyBinding::new(KeyCode::Char('F'))],
    );
    map.insert(
        KeyAction::ScrollPreviewUp,
        vec![
            KeyBinding::with_shift(KeyCode::Char('K')),
            KeyBinding::with_shift(KeyCode::Up),
        ],
    );
    map.insert(
        KeyAction::ScrollPreviewDown,
        vec![
            KeyBinding::with_shift(KeyCode::Char('J')),
            KeyBinding::with_shift(KeyCode::Down),
        ],
    );

    // In-article find (detail view). `/` is also `OpenSearch` globally,
    // but the detail-view event arm checks `OpenArticleSearch` first.
    map.insert(
        KeyAction::OpenArticleSearch,
        vec![KeyBinding::new(KeyCode::Char('/'))],
    );
    map.insert(
        KeyAction::NextMatch,
        vec![KeyBinding::new(KeyCode::Char('n'))],
    );
    map.insert(
        KeyAction::PrevMatch,
        vec![KeyBinding::new(KeyCode::Char('N'))],
    );

    // Tree
    map.insert(
        KeyAction::ToggleExpand,
        vec![KeyBinding::new(KeyCode::Char(' '))],
    );

    // Tab
    map.insert(KeyAction::NextTab, vec![KeyBinding::new(KeyCode::Tab)]);
    map.insert(
        KeyAction::PrevTab,
        vec![KeyBinding::with_shift(KeyCode::Tab)],
    );

    map
}

/// Parse a key string like "q", "Ctrl+q", "Enter", "Space", "?", "F5", "Shift+Tab"
pub fn parse_key_string(s: &str) -> Option<KeyBinding> {
    let parts: Vec<&str> = s.split('+').collect();
    let mut modifiers = KeyModifiers::NONE;
    let key_part;

    if parts.len() == 1 {
        key_part = parts[0].trim();
    } else if parts.len() == 2 {
        let modifier = parts[0].trim().to_lowercase();
        key_part = parts[1].trim();
        match modifier.as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            "alt" => modifiers |= KeyModifiers::ALT,
            _ => return None,
        }
    } else {
        return None;
    }

    let code = match key_part.to_lowercase().as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        "backspace" | "bs" => KeyCode::Backspace,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        "delete" | "del" => KeyCode::Delete,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap();
            if modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_alphabetic() {
                KeyCode::Char(c.to_ascii_uppercase())
            } else {
                KeyCode::Char(c)
            }
        }
        _ => return None,
    };

    Some(KeyBinding { code, modifiers })
}

/// Build keybinding map by merging defaults with config overrides.
/// Config format in TOML: [keybindings] section with action_name = "key" or action_name = ["key1", "key2"]
/// Returns the map and a list of warnings for invalid config entries.
pub fn build_keybindings(
    config_keybindings: &HashMap<String, toml::Value>,
) -> (KeyBindingMap, Vec<String>) {
    let mut map = default_keybindings();
    let mut warnings = Vec::new();

    for (action_str, value) in config_keybindings {
        let action: KeyAction = match action_str.parse() {
            Ok(a) => a,
            Err(_) => {
                warnings.push(format!("unknown action '{}'", action_str));
                continue;
            }
        };

        // Parse key bindings
        let keys: Vec<String> = match value {
            toml::Value::String(s) => vec![s.clone()],
            toml::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => {
                warnings.push(format!(
                    "'{}' has invalid value (expected string or array)",
                    action_str
                ));
                continue;
            }
        };

        let mut bindings = Vec::new();
        for key_str in &keys {
            match parse_key_string(key_str) {
                Some(b) => bindings.push(b),
                None => warnings.push(format!(
                    "could not parse key '{}' for action '{}'",
                    key_str, action_str
                )),
            }
        }
        if !bindings.is_empty() {
            map.insert(action, bindings);
        }
    }

    (map, warnings)
}

// ── Macro parsing ─────────────────────────────────────────────────
//
// Newsboat-compatible string syntax:
//   <op>; <op> [args]; ... [-- "<description>"]
//
// Examples:
//   open-in-browser ; pipe-to "yt-dlp %u"
//   pipe-to "wallabag-cli add %u" -- "Save to Wallabag"
//   pipe-to "tee out.txt" stdin=metadata
//
// Recognized op names mirror existing KeyAction names but use dashes
// (newsboat convention), e.g. `open-in-browser` <-> `open_in_browser`.
// Two synthetic ops, `pipe-to` and `exec`, take a quoted command template
// followed by optional `key=value` modifiers (currently only `stdin=`).

/// Split a string on a single ASCII separator, ignoring occurrences inside
/// double-quoted spans. Backslash escapes the next character.
fn split_top_level(s: &str, sep: u8) -> Vec<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_quote = false;
    let mut escape = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if escape {
            escape = false;
            i += 1;
            continue;
        }
        if b == b'\\' {
            escape = true;
            i += 1;
            continue;
        }
        if b == b'"' {
            in_quote = !in_quote;
            i += 1;
            continue;
        }
        if !in_quote && b == sep {
            out.push(s[start..i].to_string());
            start = i + 1;
        }
        i += 1;
    }
    out.push(s[start..].to_string());
    out
}

/// Detect a trailing ` -- "description"` outside double-quoted spans.
fn split_description(s: &str) -> (&str, Option<String>) {
    let bytes = s.as_bytes();
    let mut in_quote = false;
    let mut escape = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if escape {
            escape = false;
            i += 1;
            continue;
        }
        if b == b'\\' {
            escape = true;
            i += 1;
            continue;
        }
        if b == b'"' {
            in_quote = !in_quote;
            i += 1;
            continue;
        }
        // Look for the literal byte sequence ` -- ` (space-dash-dash-space)
        // outside quoted spans. `b == b' '` is the leading space; the rest of
        // the separator is matched with a single slice compare.
        if !in_quote && b == b' ' && bytes.get(i + 1..=i + 3) == Some(b"-- ") {
            // i and i+4 are ASCII boundaries; safe to slice.
            let body = &s[..i];
            let desc_part = s[i + 4..].trim();
            let unquoted = desc_part
                .strip_prefix('"')
                .and_then(|d| d.strip_suffix('"'))
                .unwrap_or(desc_part);
            let desc = unquoted.to_string();
            return (body, if desc.is_empty() { None } else { Some(desc) });
        }
        i += 1;
    }
    (s, None)
}

fn split_op_and_rest(step: &str) -> Option<(&str, &str)> {
    let s = step.trim();
    if s.is_empty() {
        return None;
    }
    match s.find(char::is_whitespace) {
        Some(idx) => Some((&s[..idx], s[idx..].trim_start())),
        None => Some((s, "")),
    }
}

fn parse_pipe_or_exec_args(rest: &str) -> Result<(Vec<String>, Vec<String>), String> {
    // shlex::split returns None on unbalanced quotes
    let tokens = shlex::split(rest).ok_or_else(|| format!("could not tokenize: {}", rest))?;
    if tokens.is_empty() {
        return Err("missing command argument".into());
    }
    let argv = shlex::split(&tokens[0])
        .ok_or_else(|| format!("could not tokenize command: {}", tokens[0]))?;
    if argv.is_empty() {
        return Err("command is empty".into());
    }
    let modifiers = tokens[1..].to_vec();
    Ok((argv, modifiers))
}

/// Parse one macro definition string into a sequence of steps and
/// an optional human description.
pub fn parse_macro_string(
    raw: &str,
    default_stdin: StdinKind,
) -> Result<(Vec<MacroStep>, Option<String>), String> {
    let (body, description) = split_description(raw);
    let parts = split_top_level(body, b';');
    let mut steps = Vec::new();
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (op, rest) = split_op_and_rest(part).ok_or_else(|| "empty step".to_string())?;
        match op {
            "pipe-to" | "pipe_to" => {
                let (argv_template, modifiers) =
                    parse_pipe_or_exec_args(rest).map_err(|e| format!("pipe-to: {}", e))?;
                let mut stdin = default_stdin;
                for m in modifiers {
                    let (k, v) = m
                        .split_once('=')
                        .ok_or_else(|| format!("pipe-to: unknown modifier '{}'", m))?;
                    match k {
                        "stdin" => {
                            stdin = StdinKind::from_str(v)
                                .map_err(|_| format!("pipe-to: invalid stdin kind '{}'", v))?;
                        }
                        _ => return Err(format!("pipe-to: unknown modifier '{}'", k)),
                    }
                }
                steps.push(MacroStep::PipeTo {
                    argv_template,
                    stdin,
                });
            }
            "exec" => {
                let (argv_template, modifiers) =
                    parse_pipe_or_exec_args(rest).map_err(|e| format!("exec: {}", e))?;
                if !modifiers.is_empty() {
                    return Err(format!("exec takes no modifiers, got: {:?}", modifiers));
                }
                steps.push(MacroStep::Exec { argv_template });
            }
            _ => {
                let normalized = op.replace('-', "_");
                let action = KeyAction::from_str(&normalized)
                    .map_err(|_| format!("unknown action '{}'", op))?;
                if !rest.is_empty() {
                    return Err(format!("action '{}' takes no arguments, got: {}", op, rest));
                }
                steps.push(MacroStep::Action(action));
            }
        }
    }
    if steps.is_empty() {
        return Err("macro has no steps".into());
    }
    Ok((steps, description))
}

/// Build the parsed macro list from a config map of `key -> definition string`.
/// Returns the bindings and a list of warnings for invalid entries.
pub fn build_macros(
    config_macros: &HashMap<String, String>,
    options: &MacroOptions,
) -> (Vec<MacroBinding>, Vec<String>) {
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    let mut keys: Vec<&String> = config_macros.keys().collect();
    keys.sort();
    for key in keys {
        let body = &config_macros[key];
        let trigger = match parse_key_string(key) {
            Some(b) => b,
            None => {
                warnings.push(format!("could not parse macro key '{}'", key));
                continue;
            }
        };
        match parse_macro_string(body, options.pipe_default_stdin) {
            Ok((steps, description)) => out.push(MacroBinding {
                trigger,
                steps,
                description,
            }),
            Err(e) => warnings.push(format!("macro '{}': {}", key, e)),
        }
    }
    (out, warnings)
}

/// Resolve `MacroOptions` from raw string-typed config values.
pub fn resolve_macro_options(
    prefix_str: &str,
    pipe_default_stdin_str: &str,
) -> (MacroOptions, Vec<String>) {
    let mut options = MacroOptions::default();
    let mut warnings = Vec::new();
    match parse_key_string(prefix_str) {
        Some(b) => options.prefix = b,
        None => warnings.push(format!("invalid macro prefix '{}'", prefix_str)),
    }
    match StdinKind::from_str(pipe_default_stdin_str) {
        Ok(k) => options.pipe_default_stdin = k,
        Err(_) => warnings.push(format!(
            "invalid pipe_default_stdin '{}'",
            pipe_default_stdin_str
        )),
    }
    (options, warnings)
}

/// Format a single `KeyBinding` like `Ctrl+q`, `Shift+Tab`, `,`, `Space`.
pub fn binding_display(binding: &KeyBinding) -> String {
    let mut parts = Vec::new();
    if binding.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl".to_string());
    }
    if binding.modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("Shift".to_string());
    }
    if binding.modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt".to_string());
    }
    let key_name = match binding.code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Up => "\u{2191}".to_string(),
        KeyCode::Down => "\u{2193}".to_string(),
        KeyCode::Left => "\u{2190}".to_string(),
        KeyCode::Right => "\u{2192}".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::F(n) => format!("F{}", n),
        _ => "?".to_string(),
    };
    parts.push(key_name);
    parts.join("+")
}

/// Get display string for the first binding of an action
pub fn key_display(action: &KeyAction, map: &KeyBindingMap) -> String {
    map.get(action)
        .and_then(|bindings| bindings.first())
        .map(binding_display)
        .unwrap_or_else(|| "?".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_default_keybindings_contains_basics() {
        let map = default_keybindings();
        assert!(map.contains_key(&KeyAction::Quit));
        assert!(map.contains_key(&KeyAction::ForceQuit));
        assert!(map.contains_key(&KeyAction::MoveUp));
        assert!(map.contains_key(&KeyAction::MoveDown));
        assert!(map.contains_key(&KeyAction::Select));
    }

    #[test]
    fn test_key_binding_matches() {
        let binding = KeyBinding::new(KeyCode::Char('q'));
        let key = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(binding.matches(&key));

        let wrong_key = make_key(KeyCode::Char('w'), KeyModifiers::NONE);
        assert!(!binding.matches(&wrong_key));
    }

    #[test]
    fn test_key_binding_ctrl_matches() {
        let binding = KeyBinding::with_ctrl(KeyCode::Char('q'));
        let key = make_key(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert!(binding.matches(&key));

        let plain_key = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!binding.matches(&plain_key));
    }

    #[test]
    fn test_parse_key_string_simple() {
        let b = parse_key_string("q").unwrap();
        assert_eq!(b.code, KeyCode::Char('q'));
        assert_eq!(b.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_key_string_ctrl() {
        let b = parse_key_string("Ctrl+q").unwrap();
        assert_eq!(b.code, KeyCode::Char('q'));
        assert_eq!(b.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_key_string_special() {
        let b = parse_key_string("Enter").unwrap();
        assert_eq!(b.code, KeyCode::Enter);

        let b = parse_key_string("Space").unwrap();
        assert_eq!(b.code, KeyCode::Char(' '));

        let b = parse_key_string("Tab").unwrap();
        assert_eq!(b.code, KeyCode::Tab);
    }

    #[test]
    fn test_parse_key_string_shift() {
        let b = parse_key_string("Shift+Tab").unwrap();
        assert_eq!(b.code, KeyCode::Tab);
        assert_eq!(b.modifiers, KeyModifiers::SHIFT);
    }

    #[test]
    fn test_build_keybindings_override() {
        let mut overrides = HashMap::new();
        overrides.insert("quit".to_string(), toml::Value::String("x".to_string()));
        let (map, warnings) = build_keybindings(&overrides);

        // Quit should now be 'x'
        let bindings = map.get(&KeyAction::Quit).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].code, KeyCode::Char('x'));
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_build_keybindings_array_override() {
        let mut overrides = HashMap::new();
        overrides.insert(
            "quit".to_string(),
            toml::Value::Array(vec![
                toml::Value::String("x".to_string()),
                toml::Value::String("Ctrl+w".to_string()),
            ]),
        );
        let (map, warnings) = build_keybindings(&overrides);

        let bindings = map.get(&KeyAction::Quit).unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].code, KeyCode::Char('x'));
        assert_eq!(bindings[1].code, KeyCode::Char('w'));
        assert_eq!(bindings[1].modifiers, KeyModifiers::CONTROL);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_key_display() {
        let map = default_keybindings();
        let display = key_display(&KeyAction::Quit, &map);
        assert_eq!(display, "q");

        let display = key_display(&KeyAction::ForceQuit, &map);
        assert_eq!(display, "Ctrl+q");

        let display = key_display(&KeyAction::Select, &map);
        assert_eq!(display, "Enter");
    }

    #[test]
    fn test_unknown_action_warns() {
        let mut overrides = HashMap::new();
        overrides.insert(
            "nonexistent_action".to_string(),
            toml::Value::String("x".to_string()),
        );
        let (map, warnings) = build_keybindings(&overrides);
        assert!(map.contains_key(&KeyAction::Quit)); // defaults still present
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unknown action"));
        assert!(warnings[0].contains("nonexistent_action"));
    }

    #[test]
    fn test_unparseable_key_warns() {
        let mut overrides = HashMap::new();
        overrides.insert(
            "quit".to_string(),
            toml::Value::String("Crtl+q".to_string()), // typo
        );
        let (map, warnings) = build_keybindings(&overrides);
        // Default binding should remain since the override failed to parse
        let bindings = map.get(&KeyAction::Quit).unwrap();
        assert_eq!(bindings[0].code, KeyCode::Char('q'));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("could not parse"));
        assert!(warnings[0].contains("Crtl+q"));
    }

    #[test]
    fn test_invalid_value_type_warns() {
        let mut overrides = HashMap::new();
        overrides.insert("quit".to_string(), toml::Value::Integer(42));
        let (map, warnings) = build_keybindings(&overrides);
        // Default binding should remain
        let bindings = map.get(&KeyAction::Quit).unwrap();
        assert_eq!(bindings[0].code, KeyCode::Char('q'));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("invalid value"));
    }

    #[test]
    fn test_parse_key_string_invalid_inputs() {
        // Empty string
        assert!(parse_key_string("").is_none());
        // Too many parts
        assert!(parse_key_string("Ctrl+Shift+X").is_none());
        // Unknown modifier
        assert!(parse_key_string("Meta+q").is_none());
        // Trailing +
        assert!(parse_key_string("Ctrl+").is_none());
        // Multi-char key name that isn't a special key
        assert!(parse_key_string("abc").is_none());
    }

    // ── Macro parser ──────────────────────────────────────────────

    #[test]
    fn test_macro_single_action() {
        let (steps, desc) = parse_macro_string("open-in-browser", StdinKind::Body).unwrap();
        assert_eq!(steps.len(), 1);
        assert!(matches!(
            steps[0],
            MacroStep::Action(KeyAction::OpenInBrowser)
        ));
        assert!(desc.is_none());
    }

    #[test]
    fn test_macro_chain_with_pipe() {
        let (steps, desc) =
            parse_macro_string(r#"open-in-browser ; pipe-to "yt-dlp %u""#, StdinKind::Body)
                .unwrap();
        assert_eq!(steps.len(), 2);
        assert!(matches!(
            steps[0],
            MacroStep::Action(KeyAction::OpenInBrowser)
        ));
        match &steps[1] {
            MacroStep::PipeTo {
                argv_template,
                stdin,
            } => {
                assert_eq!(argv_template, &vec!["yt-dlp".to_string(), "%u".to_string()]);
                assert_eq!(*stdin, StdinKind::Body);
            }
            other => panic!("expected PipeTo, got {:?}", other),
        }
        assert!(desc.is_none());
    }

    #[test]
    fn test_macro_with_description() {
        let (steps, desc) =
            parse_macro_string(r#"toggle-star -- "Star current item""#, StdinKind::Body).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(desc, Some("Star current item".to_string()));
    }

    #[test]
    fn test_macro_pipe_with_stdin_modifier() {
        let (steps, _) =
            parse_macro_string(r#"pipe-to "tee out.txt" stdin=url"#, StdinKind::Body).unwrap();
        match &steps[0] {
            MacroStep::PipeTo {
                argv_template,
                stdin,
            } => {
                assert_eq!(
                    argv_template,
                    &vec!["tee".to_string(), "out.txt".to_string()]
                );
                assert_eq!(*stdin, StdinKind::Url);
            }
            other => panic!("expected PipeTo, got {:?}", other),
        }
    }

    #[test]
    fn test_macro_exec_no_stdin() {
        let (steps, _) =
            parse_macro_string(r#"exec "notify-send hello""#, StdinKind::Body).unwrap();
        match &steps[0] {
            MacroStep::Exec { argv_template } => {
                assert_eq!(
                    argv_template,
                    &vec!["notify-send".to_string(), "hello".to_string()]
                );
            }
            other => panic!("expected Exec, got {:?}", other),
        }
    }

    #[test]
    fn test_macro_unknown_action_errors() {
        assert!(parse_macro_string("frobnicate", StdinKind::Body).is_err());
    }

    #[test]
    fn test_macro_unbalanced_quote_errors() {
        assert!(parse_macro_string(r#"pipe-to "tee out.txt"#, StdinKind::Body).is_err());
    }

    #[test]
    fn test_macro_empty_errors() {
        assert!(parse_macro_string("", StdinKind::Body).is_err());
        assert!(parse_macro_string(" ; ; ", StdinKind::Body).is_err());
    }

    #[test]
    fn test_macro_semicolon_inside_quotes_not_split() {
        let (steps, _) = parse_macro_string(r#"pipe-to "echo a;b""#, StdinKind::Body).unwrap();
        assert_eq!(steps.len(), 1);
        match &steps[0] {
            MacroStep::PipeTo { argv_template, .. } => {
                // shlex splits "echo a;b" into ["echo", "a;b"]
                assert_eq!(argv_template, &vec!["echo".to_string(), "a;b".to_string()]);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_build_macros_warns_on_bad_key() {
        let mut macros = HashMap::new();
        macros.insert("notakey++".to_string(), "open-in-browser".to_string());
        let (out, warnings) = build_macros(&macros, &MacroOptions::default());
        assert!(out.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("could not parse macro key"));
    }

    #[test]
    fn test_build_macros_warns_on_bad_body() {
        let mut macros = HashMap::new();
        macros.insert("y".to_string(), "frobnicate".to_string());
        let (out, warnings) = build_macros(&macros, &MacroOptions::default());
        assert!(out.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unknown action"));
    }

    #[test]
    fn test_resolve_macro_options() {
        let (opts, warnings) = resolve_macro_options(",", "metadata");
        assert!(warnings.is_empty());
        assert_eq!(opts.pipe_default_stdin, StdinKind::Metadata);

        let (_, warnings) = resolve_macro_options("notakey++", "body");
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_build_keybindings_page_down_with_space() {
        // Reproducer for the bug: binding Space to page_down is accepted
        // by the parser but was never dispatched in any view.
        let mut overrides = HashMap::new();
        overrides.insert(
            "page_down".to_string(),
            toml::Value::Array(vec![
                toml::Value::String("Space".to_string()),
                toml::Value::String("PageDown".to_string()),
                toml::Value::String("Ctrl+d".to_string()),
            ]),
        );
        let (map, warnings) = build_keybindings(&overrides);
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);

        let bindings = map.get(&KeyAction::PageDown).unwrap();
        assert_eq!(bindings.len(), 3);

        // "Space" → KeyCode::Char(' '), no modifiers
        assert_eq!(bindings[0].code, KeyCode::Char(' '));
        assert_eq!(bindings[0].modifiers, KeyModifiers::NONE);

        // "PageDown" → KeyCode::PageDown, no modifiers
        assert_eq!(bindings[1].code, KeyCode::PageDown);
        assert_eq!(bindings[1].modifiers, KeyModifiers::NONE);

        // "Ctrl+d" → KeyCode::Char('d'), CONTROL
        assert_eq!(bindings[2].code, KeyCode::Char('d'));
        assert_eq!(bindings[2].modifiers, KeyModifiers::CONTROL);

        // Verify the binding actually matches a Space keypress
        let space_event = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert!(
            bindings.iter().any(|b| b.matches(&space_event)),
            "Space must match the PageDown binding"
        );
    }

    #[test]
    fn test_keyaction_as_str_roundtrip_with_fromstr() {
        // Every KeyAction must round-trip through as_str() -> "-"→"_" -> from_str().
        // If you add a new KeyAction variant, add it to both maps.
        let all = [
            KeyAction::Quit,
            KeyAction::ForceQuit,
            KeyAction::Back,
            KeyAction::Home,
            KeyAction::ToggleTheme,
            KeyAction::Refresh,
            KeyAction::Help,
            KeyAction::OpenSearch,
            KeyAction::MoveUp,
            KeyAction::MoveDown,
            KeyAction::PageUp,
            KeyAction::PageDown,
            KeyAction::JumpTop,
            KeyAction::JumpBottom,
            KeyAction::Select,
            KeyAction::AddFeed,
            KeyAction::DeleteFeed,
            KeyAction::ToggleRead,
            KeyAction::ToggleStar,
            KeyAction::MarkAllRead,
            KeyAction::OpenInBrowser,
            KeyAction::TogglePreview,
            KeyAction::OpenFilter,
            KeyAction::CycleCategory,
            KeyAction::OpenCategoryManagement,
            KeyAction::AssignCategory,
            KeyAction::ExtractLinks,
            KeyAction::FetchFullText,
            KeyAction::ScrollPreviewUp,
            KeyAction::ScrollPreviewDown,
            KeyAction::OpenArticleSearch,
            KeyAction::NextMatch,
            KeyAction::PrevMatch,
            KeyAction::ToggleExpand,
            KeyAction::NextTab,
            KeyAction::PrevTab,
        ];
        for a in all {
            let s = a.as_str();
            // Display form uses dashes for readability.
            assert!(!s.contains('_'), "as_str() should use dashes, got: {}", s);
            let parsed = KeyAction::from_str(&s.replace('-', "_")).unwrap_or_else(|_| {
                panic!("could not round-trip KeyAction::{:?} via as_str()={}", a, s)
            });
            assert_eq!(parsed, a);
        }
    }
}

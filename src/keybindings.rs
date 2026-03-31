use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::str::FromStr;

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
    ScrollPreviewUp,
    ScrollPreviewDown,
    // Tree
    ToggleExpand,
    // Tab
    NextTab,
    PrevTab,
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
            "scroll_preview_up" => Ok(Self::ScrollPreviewUp),
            "scroll_preview_down" => Ok(Self::ScrollPreviewDown),
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

/// Get display string for the first binding of an action
pub fn key_display(action: &KeyAction, map: &KeyBindingMap) -> String {
    if let Some(bindings) = map.get(action) {
        if let Some(binding) = bindings.first() {
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
            return parts.join("+");
        }
    }
    "?".to_string()
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
}

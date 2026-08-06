use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Keybindings {
    #[serde(default = "default_save")]
    pub save: String,
    #[serde(default = "default_compile")]
    pub compile: String,
    #[serde(default = "default_find")]
    pub find: String,
    #[serde(default = "default_quit")]
    pub quit: String,
    #[serde(default = "default_next_tab")]
    pub next_tab: String,
    #[serde(default = "default_prev_tab")]
    pub prev_tab: String,
    #[serde(default = "default_add_ref")]
    pub add_reference: String,
    #[serde(default = "default_git_sync")]
    pub git_sync: String,
    #[serde(default = "default_command_palette")]
    pub command_palette: String,
    #[serde(default = "default_shortcuts_help")]
    pub shortcuts_help: String,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            save: default_save(),
            compile: default_compile(),
            find: default_find(),
            quit: default_quit(),
            next_tab: default_next_tab(),
            prev_tab: default_prev_tab(),
            add_reference: default_add_ref(),
            git_sync: default_git_sync(),
            command_palette: default_command_palette(),
            shortcuts_help: default_shortcuts_help(),
        }
    }
}

fn default_save() -> String { "ctrl+s".to_string() }
fn default_compile() -> String { "ctrl+shift+p".to_string() }
fn default_find() -> String { "ctrl+f".to_string() }
fn default_quit() -> String { "ctrl+q".to_string() }
fn default_next_tab() -> String { "ctrl+tab".to_string() }
fn default_prev_tab() -> String { "ctrl+shift+tab".to_string() }
fn default_add_ref() -> String { "ctrl+shift+r".to_string() }
fn default_git_sync() -> String { "ctrl+shift+s".to_string() }
fn default_command_palette() -> String { "ctrl+k".to_string() }
fn default_shortcuts_help() -> String { "ctrl+shift+h".to_string() }

impl Keybindings {
    pub fn load() -> Self {
        let path = keybindings_path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(kb) = toml::from_str::<Keybindings>(&text) {
                return kb;
            }
        }
        Self::default()
    }

    pub fn write_default_if_missing() {
        let path = keybindings_path();
        if path.exists() {
            return;
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(
            &path,
            "# Zerkalo keybindings — edit keys using format \"ctrl+shift+x\"\n\
             # Valid modifiers: ctrl, shift, alt\n\
             # Use lowercase key names: a-z, 0-9, f1-f12, tab, etc.\n\
             \n\
             save = \"ctrl+s\"\n\
             compile = \"ctrl+shift+p\"\n\
             find = \"ctrl+f\"\n\
             quit = \"ctrl+q\"\n\
             next_tab = \"ctrl+tab\"\n\
             prev_tab = \"ctrl+shift+tab\"\n\
             add_reference = \"ctrl+shift+r\"\n\
             git_sync = \"ctrl+shift+s\"\n\
             command_palette = \"ctrl+k\"\n\
             shortcuts_help = \"ctrl+shift+h\"\n",
        );
    }
}

/// Parse a key string like "ctrl+shift+p" into (modifiers_set, key_name_lowercase).
pub fn parse_key(s: &str) -> Option<(bool, bool, bool, String)> {
    let parts: Vec<&str> = s.split('+').collect();
    if parts.is_empty() {
        return None;
    }
    let key_name = parts.last()?.to_lowercase();
    let ctrl = parts.iter().any(|p| p.eq_ignore_ascii_case("ctrl") || p.eq_ignore_ascii_case("control"));
    let shift = parts.iter().any(|p| p.eq_ignore_ascii_case("shift"));
    let alt = parts.iter().any(|p| p.eq_ignore_ascii_case("alt"));
    Some((ctrl, shift, alt, key_name))
}

/// Renders a binding string for display: `"ctrl+shift+p"` → `"Ctrl+Shift+P"`.
///
/// Menu rows and the help window both show shortcuts, and both used to hardcode
/// them — so rebinding anything in keybindings.toml left the UI advertising the
/// old key.
pub fn display_binding(binding: &str) -> String {
    binding
        .split('+')
        .map(|part| {
            let p = part.trim();
            match p.to_lowercase().as_str() {
                "ctrl" | "control" => "Ctrl".to_string(),
                "shift" => "Shift".to_string(),
                "alt" => "Alt".to_string(),
                "tab" => "Tab".to_string(),
                "escape" | "esc" => "Esc".to_string(),
                "return" | "enter" => "Enter".to_string(),
                "space" => "Space".to_string(),
                other => {
                    let mut chars = other.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Check if a pressed key matches a keybinding string.
pub fn matches_binding(
    binding: &str,
    pressed_ctrl: bool,
    pressed_shift: bool,
    pressed_alt: bool,
    pressed_key: gtk4::gdk::Key,
) -> bool {
    let Some((ctrl, shift, alt, key_name)) = parse_key(binding) else { return false };
    if ctrl != pressed_ctrl || shift != pressed_shift || alt != pressed_alt {
        return false;
    }
    let name = key_name.as_str();
    // Map common names to GDK keys and check
    let gdk_key = name_to_gdk_key(name);
    match gdk_key {
        Some(k) => pressed_key == k,
        None => {
            // Try matching against the key's display name
            if let Some(key_str) = pressed_key.name() {
                key_str.to_lowercase() == name
            } else {
                false
            }
        }
    }
}

fn name_to_gdk_key(name: &str) -> Option<gtk4::gdk::Key> {
    use gtk4::gdk::Key;
    Some(match name {
        "a" => Key::a, "b" => Key::b, "c" => Key::c, "d" => Key::d,
        "e" => Key::e, "f" => Key::f, "g" => Key::g, "h" => Key::h,
        "i" => Key::i, "j" => Key::j, "k" => Key::k, "l" => Key::l,
        "m" => Key::m, "n" => Key::n, "o" => Key::o, "p" => Key::p,
        "q" => Key::q, "r" => Key::r, "s" => Key::s, "t" => Key::t,
        "u" => Key::u, "v" => Key::v, "w" => Key::w, "x" => Key::x,
        "y" => Key::y, "z" => Key::z,
        "0" => Key::_0, "1" => Key::_1, "2" => Key::_2, "3" => Key::_3,
        "4" => Key::_4, "5" => Key::_5, "6" => Key::_6, "7" => Key::_7,
        "8" => Key::_8, "9" => Key::_9,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "return" | "enter" => Key::Return,
        "space" => Key::space,
        "f1" => Key::F1, "f2" => Key::F2, "f3" => Key::F3, "f4" => Key::F4,
        "f5" => Key::F5, "f6" => Key::F6, "f7" => Key::F7, "f8" => Key::F8,
        "f9" => Key::F9, "f10" => Key::F10, "f11" => Key::F11, "f12" => Key::F12,
        _ => return None,
    })
}

pub fn keybindings_path() -> PathBuf {
    let base = shellexpand::tilde("~/.config/zerkalo").into_owned();
    PathBuf::from(base).join("keybindings.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_key_extracts_modifiers_and_key() {
        let (ctrl, shift, alt, key) = parse_key("ctrl+shift+p").unwrap();
        assert!(ctrl);
        assert!(shift);
        assert!(!alt);
        assert_eq!(key, "p");
    }

    #[test]
    fn parse_key_single_key_no_modifiers() {
        let (ctrl, shift, alt, key) = parse_key("f1").unwrap();
        assert!(!ctrl && !shift && !alt);
        assert_eq!(key, "f1");
    }

    #[test]
    fn parse_key_accepts_control_alias() {
        let (ctrl, _, _, _) = parse_key("control+s").unwrap();
        assert!(ctrl);
    }

    #[test]
    fn parse_key_is_case_insensitive_for_modifiers() {
        let (ctrl, shift, alt, key) = parse_key("CTRL+SHIFT+ALT+K").unwrap();
        assert!(ctrl && shift && alt);
        assert_eq!(key, "k");
    }

    #[test]
    fn default_keybindings_match_documented_defaults() {
        let kb = Keybindings::default();
        assert_eq!(kb.save, "ctrl+s");
        assert_eq!(kb.command_palette, "ctrl+k");
        assert_eq!(kb.shortcuts_help, "ctrl+shift+h");
    }

    #[test]
    fn keybindings_deserialize_with_partial_toml_and_defaults() {
        let toml_str = "save = \"ctrl+alt+s\"\n";
        let kb: Keybindings = toml::from_str(toml_str).unwrap();
        assert_eq!(kb.save, "ctrl+alt+s");
        // Fields absent from the TOML fall back to their serde defaults.
        assert_eq!(kb.quit, "ctrl+q");
        assert_eq!(kb.command_palette, "ctrl+k");
    }
}

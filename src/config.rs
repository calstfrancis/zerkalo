use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

// ── Theme ─────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

// ── Global config ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(default = "default_work_dir", alias = "project_path")]
    pub work_dir: PathBuf,
    #[serde(default)]
    pub output_dir: Option<PathBuf>,
    #[serde(default)]
    pub recent_files: Vec<PathBuf>,
    #[serde(default)]
    pub bib_path: Option<PathBuf>,
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
    #[serde(default = "default_true")]
    pub auto_compile: bool,
    #[serde(default = "default_font_size")]
    pub editor_font_size: u32,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default = "default_font_family")]
    pub editor_font_family: String,
    #[serde(default)]
    pub editor_word_wrap: bool,
    #[serde(default)]
    pub editor_show_whitespace: bool,
    #[serde(default = "default_tab_width")]
    pub editor_tab_width: u32,
    #[serde(default = "default_preview_zoom")]
    pub preview_zoom: f64,
    #[serde(default = "default_true")]
    pub spell_enabled: bool,
    #[serde(default)]
    pub spell_autocorrect: bool,
    #[serde(default = "default_spell_language")]
    pub spell_language: String,
}

fn default_work_dir() -> PathBuf {
    PathBuf::from(shellexpand::tilde("~/Documents/Zerkalo").into_owned())
}

pub fn default_work_dir_pub() -> PathBuf {
    default_work_dir()
}
fn default_debounce_ms() -> u64 { 300 }
fn default_true() -> bool { true }
fn default_font_size() -> u32 { 13 }
fn default_font_family() -> String { "Monospace".to_string() }
fn default_tab_width() -> u32 { 2 }
fn default_preview_zoom() -> f64 { 1.0 }
fn default_spell_language() -> String { "en_US".to_string() }

impl Default for Config {
    fn default() -> Self {
        Self {
            work_dir: default_work_dir(),
            output_dir: None,
            recent_files: Vec::new(),
            bib_path: None,
            debounce_ms: 300,
            auto_compile: true,
            editor_font_size: 13,
            theme: Theme::default(),
            editor_font_family: default_font_family(),
            editor_word_wrap: false,
            editor_show_whitespace: false,
            editor_tab_width: 2,
            preview_zoom: 1.0,
            spell_enabled: true,
            spell_autocorrect: false,
            spell_language: default_spell_language(),
        }
    }
}

impl Config {
    pub fn push_recent(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        if self.recent_files.len() > 14 {
            self.recent_files.truncate(14);
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_file()?;
        let text = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&text)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_file()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, toml::to_string(self)?)?;
        Ok(())
    }

    fn config_file() -> Result<PathBuf> {
        let base = shellexpand::tilde("~/.config/zerkalo").into_owned();
        Ok(PathBuf::from(base).join("config.toml"))
    }
}

// ── Per-project config (.zerkalo/config.toml) ─────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProjectConfig {
    /// Overrides global bib_path for this project.
    #[serde(default)]
    pub bib_path: Option<PathBuf>,
    /// Extra arguments appended to `typst compile`.
    #[serde(default)]
    pub compiler_args: Vec<String>,
    /// Override the PDF/PNG output directory (default: /tmp/zerkalo_preview).
    #[serde(default)]
    pub output_dir: Option<PathBuf>,
}

impl ProjectConfig {
    pub fn load(project_root: &Path) -> Option<Self> {
        let path = project_root.join(".zerkalo").join("config.toml");
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }

    #[allow(dead_code)]
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = project_root.join(".zerkalo");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("config.toml"), toml::to_string(self)?)?;
        Ok(())
    }
}

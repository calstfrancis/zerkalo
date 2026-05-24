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
    pub project_path: PathBuf,
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
}

fn default_debounce_ms() -> u64 {
    500
}
fn default_true() -> bool {
    true
}
fn default_font_size() -> u32 {
    13
}

impl Default for Config {
    fn default() -> Self {
        let path = shellexpand::tilde("~/Documents/Zerkalo").into_owned();
        Self {
            project_path: PathBuf::from(path),
            bib_path: None,
            debounce_ms: 500,
            auto_compile: true,
            editor_font_size: 13,
            theme: Theme::default(),
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

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = project_root.join(".zerkalo");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("config.toml"), toml::to_string(self)?)?;
        Ok(())
    }
}

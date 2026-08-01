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

// ── Compilation profile ───────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CompileProfile {
    Draft,
    #[default]
    Final,
}

// ── Snippet ───────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Snippet {
    pub trigger: String,
    pub body: String,
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
    pub recent_projects: Vec<PathBuf>,
    #[serde(default)]
    pub bib_path: Option<PathBuf>,
    /// Path to a Skrizhal `cv-elements.yaml`. When resolved (here or via
    /// `ProjectConfig`), the document is in "CV mode": the citation panel and
    /// `!`/`@` popup switch to browsing/inserting CV entries instead of
    /// bibliography citations, and `cv-helpers.typ`'s `#cv-entry`/`#cv-section`
    /// become available at compile time.
    #[serde(default)]
    pub cv_elements_path: Option<PathBuf>,
    #[serde(default)]
    pub custom_csl_path: Option<PathBuf>,
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
    #[serde(default = "default_true")]
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
    #[serde(default = "default_spell_languages")]
    pub spell_languages: Vec<String>,
    #[serde(default = "default_line_spacing")]
    pub editor_line_spacing: u32,
    #[serde(default)]
    pub typewriter_scrolling: bool,
    #[serde(default)]
    pub high_contrast: bool,
    #[serde(default)]
    pub word_count_goal: u32,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: i32,
    #[serde(default = "default_preview_split")]
    pub preview_split: i32,
    #[serde(default)]
    pub developer_mode: bool,
    #[serde(default = "default_batch_import_concurrency")]
    pub batch_import_concurrency: u32,
    #[serde(default)]
    pub last_export_format: u32,
    #[serde(default)]
    pub compile_on_save: bool,
    #[serde(default)]
    pub manual_compile_only: bool,
    #[serde(default)]
    pub recent_searches: Vec<String>,
    #[serde(default)]
    pub active_profile: CompileProfile,
    #[serde(default = "default_auto_save_idle_ms")]
    pub auto_save_idle_ms: u64,
    /// Legacy plaintext PAT field, kept only so old config files still
    /// deserialize. Migrated into the system keyring on load and never
    /// written back out — see `Config::load` and `crate::secret_store`.
    #[serde(default, skip_serializing)]
    pub github_token: Option<String>,
    #[serde(default)]
    pub locked_author: String,
    #[serde(default)]
    pub locked_affiliation: String,
    #[serde(default = "default_true")]
    pub simple_mode: bool,
    #[serde(default)]
    pub shown_simple_intro: bool,
    #[serde(default = "default_true")]
    pub format_bar_visible: bool,
    #[serde(default)]
    pub last_used_advanced: bool,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
    /// Chosen during onboarding (Setup & Onboarding -> Default Fonts). Used to
    /// pre-select the font for new documents (serif for academic/CV-Serif
    /// styles, sans for CV body kind) and to format template gallery previews.
    /// FontManager soft-locks these two fonts: disabling either is blocked
    /// with a warning until the user picks a replacement default first.
    #[serde(default)]
    pub default_sans_font: String,
    #[serde(default)]
    pub default_serif_font: String,
}

fn default_work_dir() -> PathBuf {
    PathBuf::from(shellexpand::tilde("~/Documents/Zerkalo").into_owned())
}

pub fn default_work_dir_pub() -> PathBuf {
    default_work_dir()
}
fn default_debounce_ms() -> u64 { 800 }
fn default_true() -> bool { true }
fn default_font_size() -> u32 { 13 }
fn default_font_family() -> String { "Monospace".to_string() }
fn default_tab_width() -> u32 { 2 }
fn default_preview_zoom() -> f64 { 1.0 }
fn default_spell_languages() -> Vec<String> { vec!["en_CA".to_string()] }
fn default_line_spacing() -> u32 { 2 }
fn default_sidebar_width() -> i32 { 220 }
fn default_preview_split() -> i32 { 600 }
fn default_auto_save_idle_ms() -> u64 { 30_000 }
fn default_batch_import_concurrency() -> u32 { 2 }

impl Default for Config {
    fn default() -> Self {
        Self {
            work_dir: default_work_dir(),
            output_dir: None,
            recent_files: Vec::new(),
            recent_projects: Vec::new(),
            bib_path: None,
            cv_elements_path: None,
            custom_csl_path: None,
            debounce_ms: 800,
            auto_compile: true,
            editor_font_size: 13,
            theme: Theme::default(),
            editor_font_family: default_font_family(),
            editor_word_wrap: true,
            editor_show_whitespace: false,
            editor_tab_width: 2,
            preview_zoom: 1.0,
            spell_enabled: true,
            spell_autocorrect: false,
            spell_languages: default_spell_languages(),
            editor_line_spacing: default_line_spacing(),
            typewriter_scrolling: false,
            high_contrast: false,
            word_count_goal: 0,
            sidebar_width: default_sidebar_width(),
            preview_split: default_preview_split(),
            developer_mode: false,
            batch_import_concurrency: default_batch_import_concurrency(),
            last_export_format: 0,
            compile_on_save: false,
            manual_compile_only: false,
            recent_searches: Vec::new(),
            active_profile: CompileProfile::default(),
            auto_save_idle_ms: default_auto_save_idle_ms(),
            github_token: None,
            locked_author: String::new(),
            locked_affiliation: String::new(),
            simple_mode: true,
            shown_simple_intro: false,
            format_bar_visible: true,
            last_used_advanced: false,
            snippets: Vec::new(),
            default_sans_font: String::new(),
            default_serif_font: String::new(),
        }
    }
}

impl Config {
    pub fn push_recent_search(&mut self, query: String) {
        self.recent_searches.retain(|s| s != &query);
        self.recent_searches.insert(0, query);
        if self.recent_searches.len() > 10 {
            self.recent_searches.truncate(10);
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

    #[allow(dead_code)]
    pub fn push_recent_project(&mut self, path: PathBuf) {
        self.recent_projects.retain(|p| p != &path);
        self.recent_projects.insert(0, path);
        if self.recent_projects.len() > 8 {
            self.recent_projects.truncate(8);
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_file()?;
        let text = std::fs::read_to_string(&path)?;
        let mut cfg: Self = toml::from_str(&text)?;
        if let Some(tok) = cfg.github_token.take() {
            if crate::secret_store::save_github_token(&tok).is_ok() {
                let _ = cfg.save();
            } else {
                cfg.github_token = Some(tok);
            }
        }
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_file()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        crate::error::atomic_write(&path, toml::to_string(self)?.as_bytes())?;
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
    /// Overrides global cv_elements_path for this project.
    #[serde(default)]
    pub cv_elements_path: Option<PathBuf>,
    /// Extra arguments appended to `typst compile`.
    #[serde(default)]
    pub compiler_args: Vec<String>,
    /// Override the PDF/PNG output directory (default: /tmp/zerkalo_preview).
    #[serde(default)]
    pub output_dir: Option<PathBuf>,
    /// User-defined file display order (filenames relative to project root).
    /// Files not listed appear after those that are.
    #[serde(default)]
    pub file_order: Vec<String>,
    /// Explicit compilation root (path relative to project root).
    /// When set, overrides the auto-detected root file.
    #[serde(default)]
    pub root_file: Option<PathBuf>,
    /// prefix → completion name last chosen for it in this project, so the
    /// inline suggestion learns the vocabulary of the work in hand rather than
    /// re-guessing from a static ranking every time (VS Code calls the same
    /// idea suggestSelection: recentlyUsedByPrefix).
    #[serde(default)]
    pub completion_picks: std::collections::HashMap<String, String>,
    /// Set when the user dismisses the root-file controls for this project.
    /// A single-file document has no root to choose, so the controls and the
    /// "main.typ detected" banner are just clutter; this keeps them shut and
    /// stops the banner reappearing. The "project" toggle stays in the header,
    /// so turning them back on is one click.
    #[serde(default)]
    pub root_controls_dismissed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_round_trip() {
        let cfg = Config::default();
        let toml_str = toml::to_string(&cfg).expect("serialize");
        let loaded: Config = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(cfg.debounce_ms, loaded.debounce_ms);
        assert_eq!(cfg.editor_font_size, loaded.editor_font_size);
        assert_eq!(cfg.editor_tab_width, loaded.editor_tab_width);
        assert_eq!(cfg.auto_compile, loaded.auto_compile);
        assert_eq!(cfg.spell_languages, loaded.spell_languages);
    }

    #[test]
    fn push_recent_deduplicates() {
        let mut cfg = Config::default();
        let p = PathBuf::from("/tmp/a.typ");
        cfg.push_recent(p.clone());
        cfg.push_recent(p.clone());
        assert_eq!(cfg.recent_files.len(), 1);
        assert_eq!(cfg.recent_files[0], p);
    }

    #[test]
    fn push_recent_project_caps_at_eight() {
        let mut cfg = Config::default();
        for i in 0..10 {
            cfg.push_recent_project(PathBuf::from(format!("/tmp/proj{i}")));
        }
        assert_eq!(cfg.recent_projects.len(), 8);
        // Most recent is at index 0
        assert_eq!(cfg.recent_projects[0], PathBuf::from("/tmp/proj9"));
    }

    #[test]
    fn config_with_bib_path_round_trip() {
        let mut cfg = Config::default();
        cfg.bib_path = Some(PathBuf::from("/home/user/refs.bib"));
        let toml_str = toml::to_string(&cfg).expect("serialize");
        let loaded: Config = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(loaded.bib_path, cfg.bib_path);
    }

    #[test]
    fn config_with_cv_elements_path_round_trip() {
        let mut cfg = Config::default();
        cfg.cv_elements_path = Some(PathBuf::from("/home/user/cv-elements.yaml"));
        let toml_str = toml::to_string(&cfg).expect("serialize");
        let loaded: Config = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(loaded.cv_elements_path, cfg.cv_elements_path);
    }

    #[test]
    fn project_config_cv_elements_path_round_trip() {
        let mut cfg = ProjectConfig::default();
        cfg.cv_elements_path = Some(PathBuf::from("cv-elements.yaml"));
        let toml_str = toml::to_string(&cfg).expect("serialize");
        let loaded: ProjectConfig = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(loaded.cv_elements_path, cfg.cv_elements_path);
    }
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
        crate::error::atomic_write(&dir.join("config.toml"), toml::to_string(self)?.as_bytes())?;
        Ok(())
    }
}

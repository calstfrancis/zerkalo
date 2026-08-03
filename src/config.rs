use std::cell::{OnceCell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

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

// ── Print ─────────────────────────────────────────────────────────────────────

/// How the printer handles two-sided output. The print portal owns the real
/// setting; this only decides what its dialog opens on.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DuplexPref {
    /// Leave the printer's own default alone.
    #[default]
    Printer,
    OneSided,
    /// Two-sided, flipped along the long edge — the usual choice for portrait
    /// documents, and the one a folded booklet needs.
    LongEdge,
    ShortEdge,
}

/// Print settings remembered between runs.
///
/// The portal hands its dialog a fresh `Settings` every time and keeps nothing
/// of its own, so without this every print starts from the desktop defaults —
/// re-picking two-sided and grayscale on every run of the same job.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PrintPrefs {
    /// One of "off", "two-up", "four-up", "booklet"; parsed by
    /// `crate::print_layout::Imposition`. Stored as a string so an unknown
    /// value from a newer version degrades to the default instead of failing
    /// the whole config load.
    #[serde(default = "default_imposition")]
    pub imposition: String,
    #[serde(default = "default_copies")]
    pub copies: u32,
    #[serde(default)]
    pub duplex: DuplexPref,
    #[serde(default = "default_true")]
    pub color: bool,
    /// Whether the last job collated its copies.
    #[serde(default = "default_true")]
    pub collate: bool,
}

impl Default for PrintPrefs {
    fn default() -> Self {
        Self {
            imposition: default_imposition(),
            copies: default_copies(),
            duplex: DuplexPref::default(),
            color: true,
            collate: true,
        }
    }
}

fn default_imposition() -> String { "off".to_string() }
fn default_copies() -> u32 { 1 }

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
    #[serde(default)]
    pub print: PrintPrefs,
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
            print: PrintPrefs::default(),
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

// ── The one live Config ───────────────────────────────────────────────────────

thread_local! {
    static SHARED: OnceCell<Rc<RefCell<Config>>> = const { OnceCell::new() };
    /// Set when the config file existed but could not be parsed, so the UI can
    /// tell the user once it has a window to say it in.
    static LOAD_PROBLEM: RefCell<Option<LoadProblem>> = const { RefCell::new(None) };
}

/// A config file that failed to parse, and where the original was preserved.
#[derive(Clone, Debug)]
pub struct LoadProblem {
    pub error: String,
    pub backup: PathBuf,
}

/// Takes the parse problem from startup, if there was one. Returns `Some` at
/// most once — it is reported to the user and then cleared.
pub fn take_load_problem() -> Option<LoadProblem> {
    LOAD_PROBLEM.with(|c| c.borrow_mut().take())
}

/// The process's single live `Config`.
///
/// Every settings read and write goes through this. Previously each dialog did
/// its own `Config::load().unwrap_or_default()` → mutate → `save()` against
/// disk while the main window held a separate in-memory copy, so whichever
/// wrote last silently reverted the other's changes — changing fonts in the
/// setup wizard and then toggling anything in the main window put the fonts
/// back.
pub fn shared() -> Rc<RefCell<Config>> {
    SHARED.with(|cell| {
        cell.get_or_init(|| Rc::new(RefCell::new(Config::load_or_recover())))
            .clone()
    })
}

/// Mutates the shared config and persists it. The closure sees the same
/// instance every other part of the app is holding.
pub fn update(f: impl FnOnce(&mut Config)) -> Result<()> {
    let shared = shared();
    let mut cfg = shared.borrow_mut();
    f(&mut cfg);
    cfg.save()
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

    /// Loads the config, and if the file exists but cannot be parsed, moves it
    /// aside to a timestamped backup before falling back to defaults.
    ///
    /// Without the backup step, one malformed field meant the whole file parsed
    /// as `Err`, every caller silently substituted `Config::default()`, and the
    /// next save overwrote the user's real settings permanently.
    fn load_or_recover() -> Self {
        match Self::load() {
            Ok(cfg) => cfg,
            Err(e) => {
                // A missing file is the normal first-run case, not a problem.
                let missing = matches!(
                    &e,
                    crate::error::ZerkaloError::Io(io)
                        if io.kind() == std::io::ErrorKind::NotFound
                );
                if !missing {
                    if let Ok(path) = Self::config_file() {
                        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                        let mut name = path.file_name().unwrap_or_default().to_os_string();
                        name.push(format!(".bak-{stamp}"));
                        let backup = path.with_file_name(name);
                        if std::fs::copy(&path, &backup).is_ok() {
                            tracing::error!(
                                "config.toml could not be parsed ({e}); backed up to {}",
                                backup.display()
                            );
                            LOAD_PROBLEM.with(|c| {
                                *c.borrow_mut() = Some(LoadProblem {
                                    error: e.to_string(),
                                    backup,
                                });
                            });
                        }
                    }
                }
                Self::default()
            }
        }
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
        let cfg = Config { bib_path: Some(PathBuf::from("/home/user/refs.bib")), ..Default::default() };
        let toml_str = toml::to_string(&cfg).expect("serialize");
        let loaded: Config = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(loaded.bib_path, cfg.bib_path);
    }

    #[test]
    fn config_with_cv_elements_path_round_trip() {
        let cfg = Config { cv_elements_path: Some(PathBuf::from("/home/user/cv-elements.yaml")), ..Default::default() };
        let toml_str = toml::to_string(&cfg).expect("serialize");
        let loaded: Config = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(loaded.cv_elements_path, cfg.cv_elements_path);
    }

    #[test]
    fn project_config_cv_elements_path_round_trip() {
        let cfg = ProjectConfig { cv_elements_path: Some(PathBuf::from("cv-elements.yaml")), ..Default::default() };
        let toml_str = toml::to_string(&cfg).expect("serialize");
        let loaded: ProjectConfig = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(loaded.cv_elements_path, cfg.cv_elements_path);
    }
}

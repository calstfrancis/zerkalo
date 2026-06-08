/// Project template definitions and file generation.

use std::path::Path;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectTemplate {
    Blank,
    Essay,
    JournalThesis,
    TheologicalJournal,
}

impl ProjectTemplate {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Blank            => "Blank",
            Self::Essay            => "Essay",
            Self::JournalThesis    => "Journal / Thesis",
            Self::TheologicalJournal => "Theological Journal",
        }
    }

    pub fn all() -> &'static [ProjectTemplate] {
        &[
            Self::Blank,
            Self::Essay,
            Self::JournalThesis,
            Self::TheologicalJournal,
        ]
    }

    /// Description shown in the dialog.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Blank => "An empty document. Start from scratch.",
            Self::Essay => "Single-file essay with title block and bibliography.",
            Self::JournalThesis => "Multi-chapter document: title page, intro chapter, bibliography.",
            Self::TheologicalJournal => "Journal issue: front matter, article stub, bibliography.",
        }
    }

    /// Root file relative to project folder (always "main.typ").
    pub fn root_file(&self) -> &'static str {
        "main.typ"
    }

    /// Generate all files into `project_dir`. The directory must already exist.
    pub fn generate(&self, project_dir: &Path, project_name: &str) -> Result<()> {
        match self {
            Self::Blank            => gen_blank(project_dir, project_name),
            Self::Essay            => gen_essay(project_dir, project_name),
            Self::JournalThesis    => gen_journal_thesis(project_dir, project_name),
            Self::TheologicalJournal => gen_theological_journal(project_dir, project_name),
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn write(dir: &Path, filename: &str, content: &str) -> Result<()> {
    std::fs::write(dir.join(filename), content)?;
    Ok(())
}

// ── Blank ──────────────────────────────────────────────────────────────────────

fn gen_blank(dir: &Path, name: &str) -> Result<()> {
    write(dir, "main.typ", &format!(
        "#set document(title: \"{name}\")\n\
         #set page(margin: 1in)\n\
         #set text(font: \"Linux Libertine\", size: 12pt)\n\n\
         // Begin writing here.\n"
    ))
}

// ── Essay ──────────────────────────────────────────────────────────────────────

fn gen_essay(dir: &Path, name: &str) -> Result<()> {
    write(dir, "main.typ", &format!(
        "#set document(title: \"{name}\", author: \"Author Name\")\n\
         #set page(margin: 1in)\n\
         #set text(font: \"Linux Libertine\", size: 12pt)\n\
         #set par(leading: 0.65em, justify: true)\n\n\
         #align(center)[\n\
           #text(size: 16pt, weight: \"bold\")[{name}]\n\
           \\v(0.4em)\n\
           Author Name\n\
           \\v(0.2em)\n\
           #datetime.today().display()\n\
         ]\n\n\
         #v(1em)\n\n\
         = Introduction\n\n\
         Your essay begins here.\n\n\
         = Conclusion\n\n\
         Conclusion goes here.\n\n\
         #bibliography(\"bibliography.bib\")\n"
    ))?;
    write(dir, "bibliography.bib", EMPTY_BIB)
}

// ── Journal / Thesis ───────────────────────────────────────────────────────────

fn gen_journal_thesis(dir: &Path, name: &str) -> Result<()> {
    write(dir, "main.typ", &format!(
        "#set document(title: \"{name}\", author: \"Author Name\")\n\
         #set page(margin: 1in)\n\
         #set text(font: \"Linux Libertine\", size: 12pt)\n\
         #set par(leading: 0.65em, justify: true)\n\n\
         #include \"title.typ\"\n\
         #pagebreak()\n\n\
         #include \"ch01-introduction.typ\"\n\n\
         #bibliography(\"bibliography.bib\")\n"
    ))?;
    write(dir, "title.typ", &format!(
        "#align(center + horizon)[\n\
           #text(size: 24pt, weight: \"bold\")[{name}]\n\
           \\v(1em)\n\
           #text(size: 14pt)[Author Name]\n\
           \\v(0.5em)\n\
           #text(size: 12pt, fill: luma(80))[\n\
             Atlantic School of Theology\n\
           ]\n\
           \\v(0.5em)\n\
           #datetime.today().display()\n\
         ]\n"
    ))?;
    write(dir, "ch01-introduction.typ",
        "= Introduction\n\n\
         This is the opening chapter.\n"
    )?;
    write(dir, "bibliography.bib", EMPTY_BIB)
}

// ── Theological Journal ────────────────────────────────────────────────────────

fn gen_theological_journal(dir: &Path, name: &str) -> Result<()> {
    write(dir, "main.typ", &format!(
        "#set document(title: \"{name}\")\n\
         #set page(margin: 1in)\n\
         #set text(font: \"Linux Libertine\", size: 12pt)\n\
         #set par(leading: 0.65em, justify: true)\n\n\
         #include \"front-matter.typ\"\n\
         #pagebreak()\n\n\
         #include \"article-01.typ\"\n\n\
         #bibliography(\"bibliography.bib\")\n"
    ))?;
    write(dir, "front-matter.typ", &format!(
        "#align(center)[\n\
           #text(size: 20pt, weight: \"bold\")[{name}]\n\
           \\v(0.3em)\n\
           #text(size: 12pt, fill: luma(80))[Volume 1, Issue 1]\n\
         ]\n\n\
         #line(length: 100%)\n\n\
         == Editorial Note\n\n\
         Editorial note goes here.\n\n\
         #line(length: 100%)\n"
    ))?;
    write(dir, "article-01.typ",
        "== Article Title\n\
         _Author Name_\n\n\
         === Abstract\n\n\
         Abstract goes here.\n\n\
         === Introduction\n\n\
         Article body begins here.\n"
    )?;
    write(dir, "bibliography.bib", EMPTY_BIB)
}

const EMPTY_BIB: &str = "% Bibliography — add entries here\n\
                          % Example:\n\
                          % @book{smith2020,\n\
                          %   author = {Smith, John},\n\
                          %   title  = {A Good Book},\n\
                          %   year   = {2020},\n\
                          % }\n";

/// Turn a project name into a safe folder name: lowercase, spaces → hyphens,
/// non-alphanumeric (except hyphens) stripped.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

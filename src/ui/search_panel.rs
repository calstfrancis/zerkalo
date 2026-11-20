use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::text_search::{Options, Query};
use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, MenuButton, Orientation, Popover,
    ScrolledWindow, SelectionMode, Separator, ToggleButton,
};

#[derive(Clone)]
pub struct SearchPanel {
    widget: GtkBox,
    entry: Entry,
    replace_entry: Entry,
    results: ListBox,
    work_dir: Rc<RefCell<PathBuf>>,
    on_result: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
    /// The open documents' current text. Replace all changes those in the
    /// editor (one undo step each, unsaved typing kept) instead of on disk.
    open_documents: Rc<RefCell<Option<Box<dyn Fn() -> Vec<(PathBuf, String)>>>>>,
    on_replace_open: Rc<RefCell<Option<Box<dyn Fn(PathBuf, String)>>>>,
    on_search: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    recent_searches: Rc<RefCell<Vec<String>>>,
    recent_popover_box: GtkBox,
    count_lbl: Label,
    search_gen: Rc<RefCell<u64>>,
}

impl SearchPanel {
    pub fn new(work_dir: PathBuf) -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);
        widget.set_vexpand(false);
        widget.set_visible(false);

        // ── Search bar ────────────────────────────────────────────────────────
        let bar = GtkBox::new(Orientation::Horizontal, 4);
        bar.set_margin_start(8);
        bar.set_margin_end(8);
        bar.set_margin_top(5);
        bar.set_margin_bottom(5);

        let replace_toggle = ToggleButton::new();
        replace_toggle.set_icon_name("edit-find-replace-symbolic");
        replace_toggle.set_tooltip_text(Some("Show or hide the replace box"));
        replace_toggle.add_css_class("flat");

        let entry = Entry::new();
        entry.set_placeholder_text(Some("Search in project (.typ files)…"));
        entry.set_hexpand(true);

        // Recent searches dropdown
        let recent_popover_box = GtkBox::new(Orientation::Vertical, 2);
        recent_popover_box.set_margin_top(4);
        recent_popover_box.set_margin_bottom(4);
        let recent_popover = Popover::new();
        recent_popover.set_child(Some(&recent_popover_box));
        let recent_btn = MenuButton::new();
        recent_btn.set_icon_name("pan-down-symbolic");
        recent_btn.set_tooltip_text(Some("Recent searches"));
        recent_btn.add_css_class("flat");
        recent_btn.set_popover(Some(&recent_popover));

        let count_lbl = Label::new(None);
        count_lbl.add_css_class("dim-label");
        count_lbl.add_css_class("caption");

        let close_btn = Button::from_icon_name("window-close-symbolic");
        close_btn.add_css_class("flat");
        close_btn.set_tooltip_text(Some("Close search"));
        close_btn.update_property(&[gtk4::accessible::Property::Label("Close search")]);

        bar.append(&replace_toggle);
        bar.append(&entry);
        bar.append(&recent_btn);
        bar.append(&count_lbl);
        bar.append(&close_btn);

        // ── Replace bar ────────────────────────────────────────────────────────
        let replace_bar = GtkBox::new(Orientation::Horizontal, 4);
        replace_bar.set_margin_start(8);
        replace_bar.set_margin_end(8);
        replace_bar.set_margin_top(0);
        replace_bar.set_margin_bottom(5);
        replace_bar.set_visible(false);

        let replace_entry = Entry::new();
        replace_entry.set_placeholder_text(Some("Replace with…"));
        replace_entry.set_hexpand(true);

        let replace_all_btn = Button::with_label("Replace all");
        replace_all_btn.add_css_class("destructive-action");

        replace_bar.append(&replace_entry);
        replace_bar.append(&replace_all_btn);

        // ── Results list ───────────────────────────────────────────────────────
        let scroll = ScrolledWindow::new();
        scroll.set_max_content_height(220);
        scroll.set_propagate_natural_height(true);
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);

        let results = ListBox::new();
        results.set_selection_mode(SelectionMode::Single);
        results.add_css_class("boxed-list-separate");
        scroll.set_child(Some(&results));

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&bar);
        widget.append(&replace_bar);
        widget.append(&scroll);

        let panel = Self {
            widget,
            entry: entry.clone(),
            replace_entry: replace_entry.clone(),
            results: results.clone(),
            work_dir: Rc::new(RefCell::new(work_dir)),
            on_result: Rc::new(RefCell::new(None)),
            open_documents: Rc::new(RefCell::new(None)),
            on_replace_open: Rc::new(RefCell::new(None)),
            on_search: Rc::new(RefCell::new(None)),
            recent_searches: Rc::new(RefCell::new(Vec::new())),
            recent_popover_box: recent_popover_box.clone(),
            count_lbl: count_lbl.clone(),
            search_gen: Rc::new(RefCell::new(0)),
        };

        // Replace mode toggle
        let rb = replace_bar.clone();
        replace_toggle.connect_toggled(move |btn| {
            rb.set_visible(btn.is_active());
        });

        // Search on Enter
        let p = panel.clone();
        entry.connect_activate(move |e| {
            let text = e.text().to_string();
            p.do_search(&text);
        });

        // Live search on text change, from the same three letters as the
        // editor's find bar; Enter searches for anything shorter.
        let p2 = panel.clone();
        entry.connect_changed(move |e| {
            let p3 = p2.clone();
            let text = e.text().to_string();
            let n = text.chars().count();
            if n > 0 && n < crate::ui::find_bar::MIN_TYPED {
                p3.clear_results();
                p3.count_lbl.set_text("Enter to search");
                return;
            }
            glib::idle_add_local_once(move || {
                p3.do_search(&text);
            });
        });

        // Close button
        let w = panel.widget.clone();
        close_btn.connect_clicked(move |_| w.set_visible(false));

        // Replace all
        let p_rep = panel.clone();
        replace_all_btn.connect_clicked(move |_| {
            let query = p_rep.entry.text().to_string();
            let replacement = p_rep.replace_entry.text().to_string();
            if query.is_empty() {
                return;
            }
            let work_dir = p_rep.work_dir.borrow().clone();
            *p_rep.search_gen.borrow_mut() += 1;
            let my_gen = *p_rep.search_gen.borrow();

            let open: Vec<(PathBuf, String)> = p_rep
                .open_documents
                .borrow()
                .as_ref()
                .map(|f| f())
                .unwrap_or_default();
            let Ok(q) = Query::new(&query, Options::default()) else {
                return;
            };
            let open_paths: std::collections::HashSet<PathBuf> =
                open.iter().map(|(p, _)| p.clone()).collect();
            let mut open_changed = 0usize;
            for (path, text) in &open {
                let (new_text, n) = replace_all(&q, text, &replacement);
                if n > 0 {
                    open_changed += 1;
                    if let Some(f) = p_rep.on_replace_open.borrow().as_ref() {
                        f(path.clone(), new_text);
                    }
                }
            }

            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let gitignore = load_gitignore(&work_dir);
                let mut written: Vec<PathBuf> = Vec::new();
                for file in typ_files(&work_dir, &gitignore)
                    .iter()
                    .filter(|f| !open_paths.contains(*f))
                {
                    let Ok(content) = std::fs::read_to_string(file) else {
                        continue;
                    };
                    let (new_content, n) = replace_all(&q, &content, &replacement);
                    if n > 0 && crate::error::atomic_write(file, new_content.as_bytes()).is_ok() {
                        written.push(file.clone());
                    }
                }
                let _ = tx.send(written);
            });

            let p_rep = p_rep.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                match rx.try_recv() {
                    Ok(written) => {
                        let n = written.len() + open_changed;
                        p_rep.count_lbl.set_text(&if open_changed > 0 {
                            format!(
                                "Replaced in {n} {} — Ctrl+Z undoes it in an open document",
                                if n == 1 { "file" } else { "files" }
                            )
                        } else {
                            format!("Replaced in {n} {}", if n == 1 { "file" } else { "files" })
                        });
                        // Only refresh results if no newer search/replace superseded
                        // this one in the meantime — otherwise this stale query would
                        // clobber whatever the newer operation already rendered.
                        if *p_rep.search_gen.borrow() == my_gen {
                            p_rep.do_search(&query);
                        }
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                }
            });
        });

        // Activate result row
        let p4 = panel.clone();
        results.connect_row_activated(move |_, row| {
            let name = row.widget_name().to_string();
            if let Some((file, line)) = decode_row_name(&name) {
                if let Some(f) = p4.on_result.borrow().as_ref() {
                    f(file, line);
                }
            }
        });

        panel
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn toggle(&self) {
        let visible = self.widget.is_visible();
        self.widget.set_visible(!visible);
        if !visible {
            self.entry.grab_focus();
        }
    }

    pub fn set_on_result(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_result.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_open_documents(&self, f: impl Fn() -> Vec<(PathBuf, String)> + 'static) {
        *self.open_documents.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_replace_open(&self, f: impl Fn(PathBuf, String) + 'static) {
        *self.on_replace_open.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_search(&self, f: impl Fn(String) + 'static) {
        *self.on_search.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_recent_searches(&self, searches: Vec<String>) {
        *self.recent_searches.borrow_mut() = searches.clone();
        self.rebuild_recent_popover(&searches);
    }

    fn rebuild_recent_popover(&self, searches: &[String]) {
        while let Some(child) = self.recent_popover_box.first_child() {
            self.recent_popover_box.remove(&child);
        }
        for search in searches {
            let btn = Button::new();
            btn.set_label(search);
            btn.set_halign(gtk4::Align::Start);
            btn.add_css_class("flat");
            btn.set_size_request(200, -1);
            let entry_c = self.entry.clone();
            let p = self.clone();
            let text = search.clone();
            btn.connect_clicked(move |_| {
                entry_c.set_text(&text);
                p.do_search(&text);
            });
            self.recent_popover_box.append(&btn);
        }
    }

    /// Opens the panel searching for `text` (the editor's find bar hands its
    /// search over this way).
    pub fn show_with(&self, text: &str) {
        self.widget.set_visible(true);
        if self.entry.text() == text {
            self.do_search(text);
        } else {
            self.entry.set_text(text);
        }
        self.entry.grab_focus();
    }

    fn clear_results(&self) {
        *self.search_gen.borrow_mut() += 1;
        while let Some(child) = self.results.first_child() {
            self.results.remove(&child);
        }
    }

    fn do_search(&self, query: &str) {
        self.clear_results();
        let my_gen = *self.search_gen.borrow();
        if query.is_empty() {
            self.count_lbl.set_text("");
            return;
        }
        // Notify on_search for history tracking (only on non-empty, user-driven searches)
        if let Some(f) = self.on_search.borrow().as_ref() {
            f(query.to_string());
        }
        let Ok(q) = Query::new(query, Options::default()) else {
            return;
        };
        let work_dir = self.work_dir.borrow().clone();
        // What's on screen, not what was last saved: unsaved typing is found
        // and its line numbers are the ones the editor shows.
        let open: Vec<(PathBuf, String)> = self
            .open_documents
            .borrow()
            .as_ref()
            .map(|f| f())
            .unwrap_or_default();

        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let gitignore = load_gitignore(&work_dir);
            let files = typ_files(&work_dir, &gitignore);
            let _ = tx.send(search_files(&files, &open, &q, RESULT_LIMIT));
        });

        let panel = self.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            if *panel.search_gen.borrow() != my_gen {
                return glib::ControlFlow::Break;
            }
            match rx.try_recv() {
                Ok(matches) => {
                    panel.render_results(matches);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
    }

    fn render_results(&self, matches: Vec<Match>) {
        if matches.is_empty() {
            self.count_lbl.set_text("No results");
            let row = ListBoxRow::new();
            row.set_activatable(false);
            let lbl = Label::new(Some("No results"));
            lbl.add_css_class("dim-label");
            lbl.set_margin_top(10);
            lbl.set_margin_bottom(10);
            row.set_child(Some(&lbl));
            self.results.append(&row);
            return;
        }

        let file_count = {
            let mut seen = std::collections::HashSet::new();
            for m in &matches {
                seen.insert(m.file.clone());
            }
            seen.len()
        };
        let truncated = matches.len() >= RESULT_LIMIT;
        let files = if file_count == 1 { "file" } else { "files" };
        let count_text = if truncated {
            format!("Over {RESULT_LIMIT} lines in {file_count} {files}")
        } else {
            let n = matches.len();
            let lines = if n == 1 { "line" } else { "lines" };
            format!("{n} {lines} in {file_count} {files}")
        };
        self.count_lbl.set_text(&count_text);

        for m in matches {
            let row = ListBoxRow::new();
            row.set_widget_name(&encode_row_name(&m.file, m.line));

            let rb = GtkBox::new(Orientation::Horizontal, 8);
            rb.set_margin_start(8);
            rb.set_margin_end(8);
            rb.set_margin_top(4);
            rb.set_margin_bottom(4);

            let file_name = m.file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            let loc = Label::new(Some(&format!("{}:{}", file_name, m.line)));
            loc.add_css_class("monospace");
            loc.add_css_class("caption");
            loc.set_width_chars(22);
            loc.set_xalign(0.0);

            let preview = Label::new(None);
            preview.set_markup(&line_markup(&m.preview, m.bold.0, m.bold.1));
            preview.set_hexpand(true);
            preview.set_xalign(0.0);
            preview.set_ellipsize(EllipsizeMode::End);
            preview.add_css_class("dim-label");

            rb.append(&loc);
            rb.append(&preview);
            row.set_child(Some(&rb));
            self.results.append(&row);
        }
    }
}

// ── Gitignore support ─────────────────────────────────────────────────────────

fn load_gitignore(work_dir: &Path) -> Vec<String> {
    let path = work_dir.join(".gitignore");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

fn is_gitignored(path: &Path, work_dir: &Path, patterns: &[String]) -> bool {
    let rel = match path.strip_prefix(work_dir) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    for raw in patterns {
        let pat = raw.trim_start_matches('/');
        if pat.ends_with('/') {
            let dir_pat = pat.trim_end_matches('/');
            for comp in rel.components() {
                if comp.as_os_str() == dir_pat {
                    return true;
                }
            }
        } else if pat.contains('*') {
            if glob_match_name(pat, name) {
                return true;
            }
        } else if pat.contains('/') {
            if rel.to_string_lossy().as_ref() == pat {
                return true;
            }
        } else if name == pat {
            return true;
        }
    }
    false
}

fn glob_match_name(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(ext) = pattern.strip_prefix("*.") {
        return name.ends_with(&format!(".{ext}"));
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    pattern == name
}

// ── Match highlighting ────────────────────────────────────────────────────────

/// `line` with bytes `start..end` in bold, snapped to character boundaries.
fn line_markup(line: &str, start: usize, end: usize) -> String {
    let mut start = start.min(line.len());
    let mut end = end.clamp(start, line.len());
    while start > 0 && !line.is_char_boundary(start) {
        start -= 1;
    }
    while end < line.len() && !line.is_char_boundary(end) {
        end += 1;
    }
    let before = glib::markup_escape_text(&line[..start]);
    let matched = glib::markup_escape_text(&line[start..end]);
    let after = glib::markup_escape_text(&line[end..]);
    format!("{before}<b>{matched}</b>{after}")
}

// ── File search ───────────────────────────────────────────────────────────────

const RESULT_LIMIT: usize = 200;

#[derive(Clone)]
struct Match {
    file: PathBuf,
    line: u32,
    preview: String,
    /// The match's bytes within `preview`.
    bold: (usize, usize),
}

fn replace_all(q: &Query, text: &str, with: &str) -> (String, usize) {
    let found = q.find(text);
    (q.replace_found(text, &found, with), found.len())
}

/// The project's `.typ` files, by name, leaving out hidden folders, `target`
/// and anything `.gitignore` names.
fn typ_files(work_dir: &Path, gitignore: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    visit_dir(work_dir, work_dir, &mut out, gitignore);
    out
}

fn visit_dir(dir: &Path, work_dir: &Path, out: &mut Vec<PathBuf>, gitignore: &[String]) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" || is_gitignored(&path, work_dir, gitignore) {
            continue;
        }
        if path.is_dir() {
            visit_dir(&path, work_dir, out, gitignore);
        } else if path.extension().and_then(|e| e.to_str()) == Some("typ") {
            out.push(path);
        }
    }
}

/// Every line in `files` with a match, reading an open document's text from
/// `open` rather than from disk.
fn search_files(
    files: &[PathBuf],
    open: &[(PathBuf, String)],
    q: &Query,
    limit: usize,
) -> Vec<Match> {
    let mut out = Vec::new();
    for file in files {
        let owned;
        let text = match open.iter().find(|(p, _)| p == file) {
            Some((_, t)) => t.as_str(),
            None => match std::fs::read_to_string(file) {
                Ok(t) => {
                    owned = t;
                    owned.as_str()
                }
                Err(_) => continue,
            },
        };
        let (mut line, mut counted_to) = (1u32, 0usize);
        let mut last_line = 0u32;
        for f in q.find(text) {
            line += text[counted_to..f.byte_start].matches('\n').count() as u32;
            counted_to = f.byte_start;
            // One row per line, bolding its first match.
            if line == last_line {
                continue;
            }
            last_line = line;
            let line_start = text[..f.byte_start].rfind('\n').map_or(0, |i| i + 1);
            let line_end = text[f.byte_start..]
                .find('\n')
                .map_or(text.len(), |i| f.byte_start + i);
            let raw = &text[line_start..line_end];
            let trimmed = raw.trim_start();
            let cut = raw.len() - trimmed.len();
            let preview = trimmed.trim_end().to_string();
            let bold_start = (f.byte_start - line_start).saturating_sub(cut);
            let bold_end = (f.byte_end.min(line_end) - line_start).saturating_sub(cut);
            out.push(Match {
                file: file.clone(),
                line,
                bold: (bold_start, bold_end.min(preview.len())),
                preview,
            });
            if out.len() >= limit {
                return out;
            }
        }
    }
    out
}

fn encode_row_name(file: &Path, line: u32) -> String {
    format!("{}||{}", file.display(), line)
}

fn decode_row_name(name: &str) -> Option<(PathBuf, u32)> {
    let mut parts = name.rsplitn(2, "||");
    let line: u32 = parts.next()?.parse().ok()?;
    let file = PathBuf::from(parts.next()?);
    Some((file, line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_markup_bolds_the_match_and_escapes_the_rest() {
        assert_eq!(line_markup("hello world", 6, 11), "hello <b>world</b>");
        assert!(line_markup("<tag> world", 6, 11).contains("&lt;tag&gt;"));
        // An offset inside a character is snapped, never sliced.
        assert!(line_markup("stanİ日", 0, 5).contains("<b>"));
    }

    #[test]
    fn open_documents_are_searched_as_typed_with_accents_folded() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.typ");
        let b = dir.path().join("b.typ");
        std::fs::write(&a, "nothing here\n").unwrap();
        std::fs::write(&b, "first line\n  Žižek on the\nfield\n").unwrap();
        let files = typ_files(dir.path(), &[]);
        assert_eq!(files, vec![a.clone(), b.clone()]);
        let open = vec![(a.clone(), "unsaved\nzizek typed zizek\n".to_string())];
        let q = Query::new("zizek", Options::default()).unwrap();
        let found = search_files(&files, &open, &q, 10);
        let lines: Vec<_> = found.iter().map(|m| (m.file.clone(), m.line)).collect();
        assert_eq!(lines, vec![(a, 2), (b.clone(), 2)]);
        assert_eq!(found[1].preview, "Žižek on the");
        assert_eq!(&found[1].preview[found[1].bold.0..found[1].bold.1], "Žižek");
        let across = Query::new("the field", Options::default()).unwrap();
        assert_eq!(search_files(&files, &[], &across, 10)[0].line, 2);
    }
}

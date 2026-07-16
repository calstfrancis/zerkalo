use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, MenuButton, Orientation,
    Popover, ScrolledWindow, SelectionMode, Separator, ToggleButton,
};

#[derive(Clone)]
pub struct SearchPanel {
    widget: GtkBox,
    entry: Entry,
    replace_entry: Entry,
    results: ListBox,
    work_dir: Rc<RefCell<PathBuf>>,
    on_result: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
    on_replace_done: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
    on_search: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    recent_searches: Rc<RefCell<Vec<String>>>,
    recent_popover_box: GtkBox,
    count_lbl: Label,
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
        replace_toggle.set_tooltip_text(Some("Toggle replace mode"));
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

        let replace_all_btn = Button::with_label("Replace All");
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
            on_replace_done: Rc::new(RefCell::new(None)),
            on_search: Rc::new(RefCell::new(None)),
            recent_searches: Rc::new(RefCell::new(Vec::new())),
            recent_popover_box: recent_popover_box.clone(),
            count_lbl: count_lbl.clone(),
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

        // Live search on text change
        let p2 = panel.clone();
        entry.connect_changed(move |e| {
            let p3 = p2.clone();
            let text = e.text().to_string();
            glib::idle_add_local_once(move || {
                p3.do_search(&text);
            });
        });

        // Close button
        let w = panel.widget.clone();
        close_btn.connect_clicked(move |_| w.set_visible(false));

        // Replace All
        let p_rep = panel.clone();
        replace_all_btn.connect_clicked(move |_| {
            let query = p_rep.entry.text().to_string();
            let replacement = p_rep.replace_entry.text().to_string();
            if query.is_empty() { return; }
            let work_dir = p_rep.work_dir.borrow().clone();
            let gitignore = load_gitignore(&work_dir);
            let matches = search_typ_files(&work_dir, &query.to_lowercase(), usize::MAX, &gitignore);
            let mut files_changed: std::collections::HashSet<PathBuf> = Default::default();
            for m in &matches {
                files_changed.insert(m.file.clone());
            }
            let mut replaced = 0usize;
            for file in &files_changed {
                let Ok(content) = std::fs::read_to_string(file) else { continue };
                let new_content = content.replace(&query, &replacement);
                if new_content != content {
                    let _ = std::fs::write(file, &new_content);
                    replaced += new_content.matches(&replacement).count();
                    if let Some(f) = p_rep.on_replace_done.borrow().as_ref() {
                        f(file.clone());
                    }
                }
            }
            p_rep.count_lbl.set_text(&format!("Replaced in {} files", files_changed.len()));
            p_rep.do_search(&query);
            let _ = replaced;
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

    pub fn set_on_replace_done(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_replace_done.borrow_mut() = Some(Box::new(f));
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

    fn do_search(&self, query: &str) {
        while let Some(child) = self.results.first_child() {
            self.results.remove(&child);
        }
        if query.is_empty() {
            self.count_lbl.set_text("");
            return;
        }
        // Notify on_search for history tracking (only on non-empty, user-driven searches)
        if let Some(f) = self.on_search.borrow().as_ref() {
            f(query.to_string());
        }
        let work_dir = self.work_dir.borrow().clone();
        let query_lower = query.to_lowercase();
        let gitignore = load_gitignore(&work_dir);
        let matches = search_typ_files(&work_dir, &query_lower, 200, &gitignore);

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
            for m in &matches { seen.insert(m.file.clone()); }
            seen.len()
        };
        let truncated = matches.len() >= 200;
        let count_text = if truncated {
            format!("200+ matches in {} files", file_count)
        } else {
            format!("{} matches in {} files", matches.len(), file_count)
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
            preview.set_markup(&highlight_markup(&m.preview, query));
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
    let Ok(content) = std::fs::read_to_string(&path) else { return Vec::new() };
    content.lines()
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
    if pattern == "*" { return true; }
    if let Some(ext) = pattern.strip_prefix("*.") {
        return name.ends_with(&format!(".{ext}"));
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    pattern == name
}

// ── Match highlighting ────────────────────────────────────────────────────────

fn highlight_markup(line: &str, query: &str) -> String {
    let lower_line = line.to_lowercase();
    let lower_query = query.to_lowercase();
    let Some(pos) = lower_line.find(&lower_query) else {
        return glib::markup_escape_text(line).to_string();
    };
    let end = pos + lower_query.len();

    // `pos`/`end` are byte offsets into `lower_line`, not `line` — case
    // folding can change a character's byte length (Turkish İ, Kelvin sign
    // K, German ß, …), so they aren't guaranteed to land on char boundaries
    // in the original (or even be in range) when such characters appear
    // before or within the match. Clamp into range, then snap outward to
    // the nearest valid boundary, rather than assuming they already align —
    // slicing at a non-boundary offset panics.
    let mut pos = pos.min(line.len());
    let mut end = end.min(line.len());
    while pos > 0 && !line.is_char_boundary(pos) { pos -= 1; }
    while end < line.len() && !line.is_char_boundary(end) { end += 1; }
    if end < pos { end = pos; }

    let before = glib::markup_escape_text(&line[..pos]);
    let matched = glib::markup_escape_text(&line[pos..end]);
    let after = glib::markup_escape_text(&line[end..]);
    format!("{before}<b>{matched}</b>{after}")
}

// ── File search ───────────────────────────────────────────────────────────────

struct Match {
    file: PathBuf,
    line: u32,
    preview: String,
}

fn search_typ_files(work_dir: &PathBuf, query: &str, limit: usize, gitignore: &[String]) -> Vec<Match> {
    let mut out = Vec::new();
    visit_dir(work_dir, work_dir, query, &mut out, limit, gitignore);
    out
}

fn visit_dir(dir: &PathBuf, work_dir: &Path, query: &str, out: &mut Vec<Match>, limit: usize, gitignore: &[String]) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= limit {
            return;
        }
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" {
            continue;
        }
        if is_gitignored(&path, work_dir, gitignore) {
            continue;
        }
        if path.is_dir() {
            visit_dir(&path, work_dir, query, out, limit, gitignore);
        } else if path.extension().and_then(|e| e.to_str()) == Some("typ") {
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            for (i, line) in content.lines().enumerate() {
                if out.len() >= limit {
                    return;
                }
                if line.to_lowercase().contains(query) {
                    out.push(Match {
                        file: path.clone(),
                        line: (i + 1) as u32,
                        preview: line.trim().to_string(),
                    });
                }
            }
        }
    }
}

fn encode_row_name(file: &PathBuf, line: u32) -> String {
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
    fn highlight_markup_wraps_the_match_in_bold() {
        let out = highlight_markup("hello world", "world");
        assert_eq!(out, "hello <b>world</b>");
    }

    #[test]
    fn highlight_markup_is_case_insensitive() {
        let out = highlight_markup("Hello World", "world");
        assert_eq!(out, "Hello <b>World</b>");
    }

    #[test]
    fn highlight_markup_does_not_panic_when_lowercasing_shifts_byte_offsets_mid_character() {
        // "İ" (U+0130) lowercases to "i̇" (i + combining dot above), which is
        // 1 byte longer — so a byte offset found in the lowercased copy can
        // land in the middle of "İ"'s 2-byte UTF-8 sequence in the original
        // string. This used to panic on `line[pos..end]`; now it must not.
        let line = "stanİ日";
        let out = highlight_markup(line, "stan");
        assert!(out.contains("<b>"), "should still produce a highlighted result: {out}");
    }

    #[test]
    fn highlight_markup_escapes_html_in_surrounding_text() {
        let out = highlight_markup("<tag> world", "world");
        assert!(out.contains("&lt;tag&gt;"));
    }
}

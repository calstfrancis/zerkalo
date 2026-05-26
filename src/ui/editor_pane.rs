use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, CssProvider, EventControllerKey, Label, Notebook, Orientation,
    PropagationPhase, ScrolledWindow, Separator, TextSearchFlags, TextTag, TextWindowType,
};
use libadwaita as adw;
use sourceview5::prelude::*;
use sourceview5::{Buffer, LanguageManager, StyleSchemeManager, View};

use crate::bibliography::BibEntry;
use crate::lsp::CompletionItem;
use super::bib_popup::BibPopup;
use super::find_bar::FindBar;
use super::lsp_popup::LspPopup;

// Minimal Typst language definition for GtkSourceView
const TYPST_LANG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<language id="typst" name="Typst" version="2.0" _section="Markup">
  <metadata>
    <property name="mimetypes">text/x-typst</property>
    <property name="globs">*.typ</property>
    <property name="line-comment-start">//</property>
    <property name="block-comment-start">/*</property>
    <property name="block-comment-end">*/</property>
  </metadata>
  <styles>
    <style id="comment"  name="Comment"  map-to="def:comment"/>
    <style id="string"   name="String"   map-to="def:string"/>
    <style id="keyword"  name="Keyword"  map-to="def:keyword"/>
    <style id="function" name="Function" map-to="def:identifier"/>
    <style id="heading"  name="Heading"  map-to="def:type"/>
    <style id="markup"   name="Markup"   map-to="def:special-char"/>
    <style id="math"     name="Math"     map-to="def:number"/>
  </styles>
  <definitions>
    <context id="typst">
      <include>
        <context id="line-comment" style-ref="comment" end-at-line-end="true">
          <start>//</start>
        </context>
        <context id="block-comment" style-ref="comment">
          <start>/\*</start>
          <end>\*/</end>
        </context>
        <context id="heading" style-ref="heading" end-at-line-end="true">
          <start>^=+\s</start>
        </context>
        <context id="string" style-ref="string" end-at-line-end="false">
          <start>"</start>
          <end>"</end>
        </context>
        <context id="math-inline" style-ref="math">
          <start>\$</start>
          <end>\$</end>
        </context>
        <context id="function-call" style-ref="function">
          <match>#[a-zA-Z][a-zA-Z0-9_-]*</match>
        </context>
        <context id="citation" style-ref="markup">
          <match>@[a-zA-Z][a-zA-Z0-9:_-]*</match>
        </context>
        <context id="label-def" style-ref="markup">
          <match>&lt;[a-zA-Z][a-zA-Z0-9:_-]*&gt;</match>
        </context>
        <context id="keywords" style-ref="keyword">
          <keyword>let</keyword>
          <keyword>set</keyword>
          <keyword>show</keyword>
          <keyword>if</keyword>
          <keyword>else</keyword>
          <keyword>for</keyword>
          <keyword>in</keyword>
          <keyword>while</keyword>
          <keyword>break</keyword>
          <keyword>continue</keyword>
          <keyword>return</keyword>
          <keyword>import</keyword>
          <keyword>include</keyword>
          <keyword>none</keyword>
          <keyword>auto</keyword>
          <keyword>true</keyword>
          <keyword>false</keyword>
        </context>
      </include>
    </context>
  </definitions>
</language>
"#;

// ── Built-in academic snippets ────────────────────────────────────────────────
// (match_key, display_label, insert_text_with_leading_#)
const ACADEMIC_SNIPPETS: &[(&str, &str, &str)] = &[
    ("figure", "Figure",
     "#figure(\n  image(\"\", width: 80%),\n  caption: [Caption text],\n) <fig:label>"),
    ("table", "Table",
     "#figure(\n  table(\n    columns: (auto, auto),\n    table.header([*Column 1*], [*Column 2*]),\n    [Cell 1], [Cell 2],\n  ),\n  caption: [Table title],\n) <tab:label>"),
    ("footnote", "Footnote", "#footnote[Note text]"),
    ("bibliography", "Bibliography", "#bibliography(\"refs.bib\")"),
    ("pagebreak", "Page break", "#pagebreak()"),
    ("outline", "Table of Contents", "#outline(title: [Contents], depth: 3)"),
    ("lorem", "Lorem ipsum", "#lorem(100)"),
    ("set", "Set rule", "#set text(size: 11pt, font: \"Liberation Serif\")"),
    ("show", "Show rule", "#show heading: it => strong(it)"),
    ("block", "Block / quote", "#block(inset: (left: 2em))[\n  Quoted text\n]"),
];

// ── Internal types ────────────────────────────────────────────────────────────

struct EditorTab {
    buffer: Buffer,
    view: View,
    scroll_window: ScrolledWindow,
    modified: bool,
    dot_label: Label,
    lsp_popup: LspPopup,
}

struct EditorState {
    tabs: HashMap<PathBuf, EditorTab>,
}

// ── Public API ────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct EditorPane {
    outer: GtkBox,
    notebook: Notebook,
    state: Rc<RefCell<EditorState>>,
    on_change: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_page_switch: Rc<RefCell<Option<Box<dyn Fn(String, PathBuf)>>>>,
    on_file_opened: Rc<RefCell<Option<Box<dyn Fn(PathBuf, String)>>>>,
    on_completion_needed: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32, u32)>>>>,
    on_cursor_heading: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>,
    bib_entries: Rc<RefCell<Vec<BibEntry>>>,
    font_provider: Rc<CssProvider>,
    font_size: Rc<RefCell<u32>>,
    font_family: Rc<RefCell<String>>,
    word_wrap: Rc<RefCell<bool>>,
    show_whitespace: Rc<RefCell<bool>>,
    tab_width: Rc<RefCell<u32>>,
    find_bar: FindBar,
    word_count_label: Label,
    cursor_label: Label,
}

impl EditorPane {
    pub fn new() -> Self {
        let notebook = Notebook::new();
        notebook.set_scrollable(true);
        notebook.set_hexpand(true);
        notebook.set_vexpand(true);
        notebook.set_show_tabs(false);

        let state = Rc::new(RefCell::new(EditorState {
            tabs: HashMap::new(),
        }));

        let font_provider = CssProvider::new();
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &font_provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        // Install Typst language definition
        let lang_dir = glib::user_data_dir().join("zerkalo/language-specs");
        let lang_file = lang_dir.join("typst.lang");
        if !lang_file.exists() {
            if std::fs::create_dir_all(&lang_dir).is_ok() {
                let _ = std::fs::write(&lang_file, TYPST_LANG);
            }
        }
        let lang_manager = LanguageManager::default();
        let dir_str = lang_dir.to_string_lossy().to_string();
        let existing: Vec<String> = lang_manager
            .search_path()
            .iter()
            .map(|s| s.to_string())
            .collect();
        if !existing.contains(&dir_str) {
            let mut paths: Vec<&str> = vec![dir_str.as_str()];
            paths.extend(existing.iter().map(|s| s.as_str()));
            lang_manager.set_search_path(&paths);
        }

        let find_bar = FindBar::new();

        let status_bar = GtkBox::new(Orientation::Horizontal, 0);
        status_bar.set_hexpand(true);

        let cursor_label = Label::new(Some("Ln 1, Col 1"));
        cursor_label.add_css_class("dim-label");
        cursor_label.add_css_class("caption");
        cursor_label.set_margin_start(12);
        cursor_label.set_margin_top(3);
        cursor_label.set_margin_bottom(3);
        status_bar.append(&cursor_label);

        let word_count_label = Label::new(Some(""));
        word_count_label.add_css_class("dim-label");
        word_count_label.add_css_class("caption");
        word_count_label.set_hexpand(true);
        word_count_label.set_xalign(1.0);
        word_count_label.set_margin_end(12);
        word_count_label.set_margin_top(3);
        word_count_label.set_margin_bottom(3);
        status_bar.append(&word_count_label);

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.set_hexpand(true);
        outer.set_vexpand(true);
        outer.append(&notebook);
        outer.append(&Separator::new(Orientation::Horizontal));
        outer.append(find_bar.widget());
        outer.append(&Separator::new(Orientation::Horizontal));
        outer.append(&status_bar);

        let on_change: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_page_switch: Rc<RefCell<Option<Box<dyn Fn(String, PathBuf)>>>> =
            Rc::new(RefCell::new(None));
        let on_file_opened: Rc<RefCell<Option<Box<dyn Fn(PathBuf, String)>>>> =
            Rc::new(RefCell::new(None));
        let on_completion_needed: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32, u32)>>>> =
            Rc::new(RefCell::new(None));
        let on_cursor_heading: Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>> =
            Rc::new(RefCell::new(None));

        let font_size: Rc<RefCell<u32>> = Rc::new(RefCell::new(13));
        let font_family: Rc<RefCell<String>> = Rc::new(RefCell::new("Monospace".to_string()));
        let word_wrap: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let show_whitespace: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let tab_width: Rc<RefCell<u32>> = Rc::new(RefCell::new(2));

        {
            let state2 = state.clone();
            let wc = word_count_label.clone();
            let ps = on_page_switch.clone();
            notebook.connect_switch_page(move |nb, _, page_num| {
                let bstate = state2.borrow();
                for (path, tab) in &bstate.tabs {
                    if nb.page_num(&tab.scroll_window) == Some(page_num) {
                        let (s, e) = tab.buffer.bounds();
                        let content = tab.buffer.text(&s, &e, false).to_string();
                        set_wc_text(&wc, &content);
                        if let Some(f) = ps.borrow().as_ref() {
                            f(content, path.clone());
                        }
                        break;
                    }
                }
            });
        }

        let ep = Self {
            outer,
            notebook,
            state,
            on_change,
            on_page_switch,
            on_file_opened,
            on_completion_needed,
            on_cursor_heading,
            bib_entries: Rc::new(RefCell::new(Vec::new())),
            font_provider: Rc::new(font_provider),
            font_size,
            font_family,
            word_wrap,
            show_whitespace,
            tab_width,
            find_bar,
            word_count_label,
            cursor_label,
        };

        {
            let ep2 = ep.clone();
            ep.find_bar.set_on_search(move |text, forward, whole_word| ep2.do_find(text, forward, whole_word));
        }
        {
            let ep2 = ep.clone();
            ep.find_bar.set_on_replace_one(move |find, replace, whole_word| ep2.do_replace_one(find, replace, whole_word));
        }
        {
            let ep2 = ep.clone();
            ep.find_bar.set_on_replace_all(move |find, replace, whole_word| ep2.do_replace_all(find, replace, whole_word));
        }

        ep
    }

    pub fn widget(&self) -> &GtkBox {
        &self.outer
    }

    // ── Settings ──────────────────────────────────────────────────────────────

    pub fn set_bib_entries(&self, entries: Vec<BibEntry>) {
        *self.bib_entries.borrow_mut() = entries;
    }

    pub fn apply_font_size(&self, size: u32) {
        *self.font_size.borrow_mut() = size;
        self.rebuild_font_css();
    }

    pub fn apply_font_family(&self, family: &str) {
        *self.font_family.borrow_mut() = family.to_string();
        self.rebuild_font_css();
    }

    fn rebuild_font_css(&self) {
        let size = *self.font_size.borrow();
        let family = self.font_family.borrow().clone();
        let css = if size > 0 {
            format!("textview {{ font-family: '{family}'; font-size: {size}pt; }}")
        } else {
            format!("textview {{ font-family: '{family}'; }}")
        };
        self.font_provider.load_from_data(&css);
    }

    pub fn apply_word_wrap(&self, enabled: bool) {
        *self.word_wrap.borrow_mut() = enabled;
        let mode = if enabled { gtk4::WrapMode::Word } else { gtk4::WrapMode::None };
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            tab.view.set_wrap_mode(mode);
        }
    }

    pub fn apply_show_whitespace(&self, enabled: bool) {
        *self.show_whitespace.borrow_mut() = enabled;
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            apply_space_drawer(&tab.view, enabled);
        }
    }

    pub fn apply_tab_width(&self, width: u32) {
        *self.tab_width.borrow_mut() = width;
        let w = width.max(1);
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            tab.view.set_tab_width(w);
            tab.view.set_indent_width(w as i32);
        }
    }

    pub fn apply_style_scheme(&self, is_dark: bool) {
        let scheme_id = if is_dark { "Adwaita-dark" } else { "Adwaita" };
        let mgr = StyleSchemeManager::default();
        let scheme = mgr.scheme(scheme_id);
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            tab.buffer.set_style_scheme(scheme.as_ref());
        }
    }

    // ── Find & Replace ────────────────────────────────────────────────────────

    pub fn toggle_find(&self) {
        self.find_bar.show();
    }

    pub fn do_find(&self, text: &str, forward: bool, whole_word: bool) {
        if text.is_empty() {
            self.find_bar.set_result("");
            return;
        }
        let Some((view, buffer)) = self.active_view_buffer() else { return };
        let flags = TextSearchFlags::TEXT_ONLY | TextSearchFlags::CASE_INSENSITIVE;
        let cursor_pos = buffer.cursor_position();

        let mut search_start = buffer.iter_at_offset(cursor_pos);
        let mut wrapped = false;

        loop {
            let result = if forward {
                search_start.forward_search(text, flags, None)
            } else {
                search_start.backward_search(text, flags, None)
            };

            match result {
                None => {
                    if wrapped {
                        self.find_bar.set_result("No results");
                        return;
                    }
                    wrapped = true;
                    search_start = if forward { buffer.start_iter() } else { buffer.end_iter() };
                }
                Some((start, end)) => {
                    if whole_word && !is_whole_word(&start, &end) {
                        search_start = if forward { end.clone() } else { start.clone() };
                        // Prevent infinite wrap-around
                        let cur = if forward { end.offset() } else { start.offset() };
                        if wrapped && (if forward { cur >= cursor_pos } else { cur <= cursor_pos }) {
                            self.find_bar.set_result("No results");
                            return;
                        }
                        continue;
                    }
                    if forward {
                        buffer.select_range(&end, &start);
                    } else {
                        buffer.select_range(&start, &end);
                    }
                    view.scroll_to_iter(&mut start.clone(), 0.1, false, 0.0, 0.5);
                    self.find_bar.set_result("");
                    return;
                }
            }
        }
    }

    pub fn do_replace_one(&self, find: &str, replace: &str, whole_word: bool) {
        if find.is_empty() {
            return;
        }
        let Some((_view, buffer)) = self.active_view_buffer() else { return };
        if let Some((sel_start, sel_end)) = buffer.selection_bounds() {
            let selected = buffer.text(&sel_start, &sel_end, false).to_string();
            if selected.to_lowercase() == find.to_lowercase() {
                if !whole_word || is_whole_word(&sel_start, &sel_end) {
                    let offset = sel_start.offset();
                    let mut s = sel_start;
                    let mut e = sel_end;
                    buffer.begin_user_action();
                    buffer.delete(&mut s, &mut e);
                    let mut ins = buffer.iter_at_offset(offset);
                    buffer.insert(&mut ins, replace);
                    buffer.end_user_action();
                }
            }
        }
        self.do_find(find, true, whole_word);
    }

    pub fn do_replace_all(&self, find: &str, replace: &str, whole_word: bool) {
        if find.is_empty() {
            return;
        }
        let Some((_view, buffer)) = self.active_view_buffer() else { return };
        let flags = TextSearchFlags::TEXT_ONLY | TextSearchFlags::CASE_INSENSITIVE;
        let mut count: usize = 0;
        buffer.begin_user_action();
        let mut iter = buffer.start_iter();
        loop {
            match iter.forward_search(find, flags, None) {
                Some((mut start, mut end)) => {
                    if whole_word && !is_whole_word(&start, &end) {
                        iter = end;
                        continue;
                    }
                    let offset = start.offset();
                    buffer.delete(&mut start, &mut end);
                    let mut ins = buffer.iter_at_offset(offset);
                    buffer.insert(&mut ins, replace);
                    iter = buffer.iter_at_offset(offset + replace.chars().count() as i32);
                    count += 1;
                }
                None => break,
            }
        }
        buffer.end_user_action();
        self.find_bar.set_result(&format!("Replaced {count}"));
    }

    // ── LSP completions ───────────────────────────────────────────────────────

    /// Called from app_window when a completion response arrives. Shows the
    /// popup on the currently-active tab's view.
    pub fn show_lsp_completions(&self, items: Vec<CompletionItem>) {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return,
        };
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if self.notebook.page_num(&tab.scroll_window) == Some(current) {
                let prefix = lsp_hash_prefix(&tab.buffer);
                let mut all_items: Vec<CompletionItem> = ACADEMIC_SNIPPETS
                    .iter()
                    .filter(|(key, _, _)| prefix.is_empty() || key.starts_with(prefix.as_str()))
                    .map(|(key, label, body)| CompletionItem {
                        label: format!("{label}  ·  snippet"),
                        kind: 15,
                        detail: Some(key.to_string()),
                        insert_text: Some(body.to_string()),
                    })
                    .collect();
                all_items.extend(items);

                let cursor = tab.buffer.iter_at_offset(tab.buffer.cursor_position());
                let loc = tab.view.iter_location(&cursor);
                let (wx, wy) = tab.view.buffer_to_window_coords(
                    TextWindowType::Widget,
                    loc.x(),
                    loc.y() + loc.height(),
                );
                tab.lsp_popup.show_items(all_items, wx, wy);
                break;
            }
        }
    }

    // ── Inline diagnostic marks ───────────────────────────────────────────────

    /// Apply underline squiggles for the given diagnostics. Each entry is
    /// (file, 1-based line, is_error). Call after compile or LSP diagnostics.
    pub fn mark_diagnostics(&self, diagnostics: &[(PathBuf, u32, bool)]) {
        let state = self.state.borrow();
        for (path, tab) in &state.tabs {
            let (buf_start, buf_end) = tab.buffer.bounds();
            tab.buffer.remove_tag_by_name("zerkalo-diag-error", &buf_start, &buf_end);
            tab.buffer.remove_tag_by_name("zerkalo-diag-warning", &buf_start, &buf_end);
            ensure_diag_tags(&tab.buffer);
            for (err_file, err_line, is_error) in diagnostics {
                if err_file != path {
                    continue;
                }
                let line_idx = err_line.saturating_sub(1) as i32;
                if let Some(line_start) = tab.buffer.iter_at_line(line_idx) {
                    let mut line_end = line_start;
                    line_end.forward_to_line_end();
                    let tag = if *is_error { "zerkalo-diag-error" } else { "zerkalo-diag-warning" };
                    tab.buffer.apply_tag_by_name(tag, &line_start, &line_end);
                }
            }
        }
    }

    pub fn clear_diagnostic_marks(&self) {
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            let (start, end) = tab.buffer.bounds();
            tab.buffer.remove_tag_by_name("zerkalo-diag-error", &start, &end);
            tab.buffer.remove_tag_by_name("zerkalo-diag-warning", &start, &end);
        }
    }

    // ── Callbacks ─────────────────────────────────────────────────────────────

    pub fn set_on_change(&self, f: impl Fn() + 'static) {
        *self.on_change.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_page_switch(&self, f: impl Fn(String, PathBuf) + 'static) {
        *self.on_page_switch.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_file_opened(&self, f: impl Fn(PathBuf, String) + 'static) {
        *self.on_file_opened.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_completion_needed(&self, f: impl Fn(PathBuf, u32, u32) + 'static) {
        *self.on_completion_needed.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_cursor_heading(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_cursor_heading.borrow_mut() = Some(Box::new(f));
    }

    pub fn apply_style(&self, style_code: &str, bib_style: &str, bib_title: &str) {
        let Some(path) = self.get_active_path() else { return };
        let Some(content) = self.get_active_content() else { return };
        let new_content = crate::styles::apply_to(&content, style_code, bib_style, bib_title);
        if new_content != content {
            // Clone the buffer before dropping the borrow; set_text fires
            // connect_changed which calls borrow_mut — holding the borrow here
            // causes a RefCell double-borrow panic.
            let buffer_opt = {
                let state = self.state.borrow();
                state.tabs.get(&path).map(|tab| tab.buffer.clone())
            };
            if let Some(buffer) = buffer_opt {
                buffer.set_text(&new_content);
            }
        }
    }

    pub fn insert_at_cursor(&self, text: &str) {
        if let Some((view, buffer)) = self.active_view_buffer() {
            buffer.begin_user_action();
            buffer.insert_at_cursor(text);
            buffer.end_user_action();
            view.grab_focus();
        }
    }

    // ── File management ───────────────────────────────────────────────────────

    pub fn open_file(&self, path: PathBuf, content: &str) {
        {
            let state = self.state.borrow();
            if let Some(tab) = state.tabs.get(&path) {
                if let Some(n) = self.notebook.page_num(&tab.scroll_window) {
                    self.notebook.set_current_page(Some(n));
                }
                return;
            }
        }

        let display_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();

        let buffer = Buffer::new(None::<&gtk4::TextTagTable>);

        let lang_manager = LanguageManager::default();
        if let Some(path_str) = path.to_str() {
            if let Some(lang) = lang_manager.guess_language(Some(path_str), None) {
                buffer.set_language(Some(&lang));
                buffer.set_highlight_syntax(true);
            }
        }
        let scheme_id = if adw::StyleManager::default().is_dark() {
            "Adwaita-dark"
        } else {
            "Adwaita"
        };
        if let Some(scheme) = StyleSchemeManager::default().scheme(scheme_id) {
            buffer.set_style_scheme(Some(&scheme));
        }

        buffer.set_text(content);

        let view = View::with_buffer(&buffer);
        view.set_show_line_numbers(true);
        view.set_auto_indent(true);
        view.set_smart_backspace(true);
        view.set_insert_spaces_instead_of_tabs(true);
        let tw = *self.tab_width.borrow();
        view.set_tab_width(tw.max(1));
        view.set_indent_width(tw as i32);
        view.set_monospace(true);
        view.set_highlight_current_line(true);
        let wrap_mode = if *self.word_wrap.borrow() { gtk4::WrapMode::Word } else { gtk4::WrapMode::None };
        view.set_wrap_mode(wrap_mode);
        apply_space_drawer(&view, *self.show_whitespace.borrow());

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&view));
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);

        // ── Tab label ─────────────────────────────────────────────────────────

        let tab_box = GtkBox::new(Orientation::Horizontal, 4);
        let name_label = Label::new(Some(&display_name));
        let dot_label = Label::new(Some("●"));
        dot_label.set_visible(false);
        let close_btn = Button::new();
        close_btn.set_label("✕");
        close_btn.add_css_class("flat");

        tab_box.append(&name_label);
        tab_box.append(&dot_label);
        tab_box.append(&close_btn);

        let state_for_close = self.state.clone();
        let notebook_for_close = self.notebook.clone();
        let path_for_close = path.clone();
        let scroll_for_close = scroll.clone();
        close_btn.connect_clicked(move |_| {
            if let Some(n) = notebook_for_close.page_num(&scroll_for_close) {
                notebook_for_close.remove_page(Some(n));
            }
            state_for_close.borrow_mut().tabs.remove(&path_for_close);
        });

        // ── Modified flag + word count ────────────────────────────────────────

        let state_for_change = self.state.clone();
        let path_for_change = path.clone();
        let dot_for_change = dot_label.clone();
        let on_change_cb = self.on_change.clone();
        let wc_for_change = self.word_count_label.clone();
        buffer.connect_changed(move |buf| {
            {
                let mut state = state_for_change.borrow_mut();
                if let Some(tab) = state.tabs.get_mut(&path_for_change) {
                    if !tab.modified {
                        tab.modified = true;
                        dot_for_change.set_visible(true);
                    }
                }
            }
            let (s, e) = buf.bounds();
            let text = buf.text(&s, &e, false);
            set_wc_text(&wc_for_change, &text);
            if let Some(f) = on_change_cb.borrow().as_ref() {
                f();
            }
        });

        // ── Cursor position tracking + heading detection ──────────────────────

        let cursor_lbl = self.cursor_label.clone();
        let on_heading_cb = self.on_cursor_heading.clone();
        let path_for_heading = path.clone();
        let last_heading_line: Rc<RefCell<u32>> = Rc::new(RefCell::new(u32::MAX));
        buffer.connect_mark_set(move |buf, _iter, mark| {
            if mark.name().as_deref() == Some("insert") {
                let cursor = buf.iter_at_mark(mark);
                let line = cursor.line() + 1;
                let col = cursor.line_offset() + 1;
                cursor_lbl.set_text(&format!("Ln {line}, Col {col}"));

                // Scan backward from cursor line for a heading
                if let Some(cb) = on_heading_cb.borrow().as_ref() {
                    let heading_line = find_heading_line_for(buf, cursor.line());
                    if heading_line != *last_heading_line.borrow() {
                        *last_heading_line.borrow_mut() = heading_line;
                        if heading_line != u32::MAX {
                            cb(path_for_heading.clone(), heading_line);
                        }
                    }
                }
            }
        });

        // ── @-citation autocomplete ───────────────────────────────────────────

        let bib_popup = BibPopup::new(&view, self.bib_entries.clone());
        let ac_mark: Rc<RefCell<Option<gtk4::TextMark>>> = Rc::new(RefCell::new(None));
        let completing: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let buf_complete = buffer.clone();
        let view_complete = view.clone();
        let mark_complete = ac_mark.clone();
        let completing_complete = completing.clone();
        let popup_complete = bib_popup.clone();
        bib_popup.set_on_complete(move |key| {
            *completing_complete.borrow_mut() = true;
            let mark_opt = mark_complete.borrow().clone();
            if let Some(ref m) = mark_opt {
                let mut start = buf_complete.iter_at_mark(m);
                let mut end = buf_complete.iter_at_offset(buf_complete.cursor_position());
                buf_complete.begin_user_action();
                buf_complete.delete(&mut start, &mut end);
                buf_complete.insert_at_cursor(&format!("@{key}"));
                buf_complete.end_user_action();
                buf_complete.delete_mark(m);
            }
            *mark_complete.borrow_mut() = None;
            popup_complete.hide();
            view_complete.grab_focus();
            *completing_complete.borrow_mut() = false;
        });

        let view_ac = view.clone();
        let popup_ac = bib_popup.clone();
        let mark_ac = ac_mark.clone();
        let completing_ac = completing.clone();
        buffer.connect_changed(move |buf| {
            if *completing_ac.borrow() {
                return;
            }
            let cursor_pos = buf.cursor_position();
            let cursor_iter = buf.iter_at_offset(cursor_pos);
            let mut temp = cursor_iter.clone();
            let mut found_at = false;
            let mut at_iter = cursor_iter.clone();
            loop {
                if !temp.backward_char() {
                    break;
                }
                let ch = temp.char();
                if ch == '@' {
                    found_at = true;
                    at_iter = temp.clone();
                    break;
                }
                if !(ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ':') {
                    break;
                }
            }
            if !found_at {
                dismiss_popup(buf, &popup_ac, &mark_ac);
                return;
            }
            let prev_is_word = {
                let mut prev = at_iter.clone();
                if prev.backward_char() {
                    let ch = prev.char();
                    ch.is_alphanumeric() || ch == '_'
                } else {
                    false
                }
            };
            if prev_is_word {
                dismiss_popup(buf, &popup_ac, &mark_ac);
                return;
            }
            let query = buf.text(&at_iter, &cursor_iter, false);
            let query = query.trim_start_matches('@');
            {
                let mut mark_ref = mark_ac.borrow_mut();
                match mark_ref.as_ref() {
                    Some(m) => buf.move_mark(m, &at_iter),
                    None => *mark_ref = Some(buf.create_mark(None::<&str>, &at_iter, true)),
                }
            }
            let loc = view_ac.iter_location(&cursor_iter);
            let (wx, wy) = view_ac.buffer_to_window_coords(
                TextWindowType::Widget,
                loc.x(),
                loc.y() + loc.height(),
            );
            popup_ac.show_filtered(query, wx, wy);
        });

        // ── #-function LSP autocomplete ───────────────────────────────────────

        let lsp_popup = LspPopup::new(&view);
        let lsp_mark: Rc<RefCell<Option<gtk4::TextMark>>> = Rc::new(RefCell::new(None));
        let lsp_completing: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let lsp_comp_gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));

        // LSP on_complete: replace #prefix with the chosen insertion text
        {
            let buf2 = buffer.clone();
            let view2 = view.clone();
            let mark2 = lsp_mark.clone();
            let comp2 = lsp_completing.clone();
            let popup2 = lsp_popup.clone();
            lsp_popup.set_on_complete(move |item| {
                *comp2.borrow_mut() = true;
                let mark_opt = mark2.borrow().clone();
                if let Some(ref m) = mark_opt {
                    let mut start = buf2.iter_at_mark(m); // position of '#'
                    let mut end = buf2.iter_at_offset(buf2.cursor_position());
                    let insert_text = item
                        .insert_text
                        .as_deref()
                        .unwrap_or(&item.label);
                    let insert_text = strip_snippets(insert_text);
                    let final_text = if insert_text.starts_with('#') {
                        insert_text
                    } else {
                        format!("#{insert_text}")
                    };
                    buf2.begin_user_action();
                    buf2.delete(&mut start, &mut end);
                    buf2.insert_at_cursor(&final_text);
                    buf2.end_user_action();
                    buf2.delete_mark(m);
                }
                *mark2.borrow_mut() = None;
                popup2.hide();
                view2.grab_focus();
                *comp2.borrow_mut() = false;
            });
        }

        // Detect #word context and fire on_completion_needed
        {
            let lsp_mark3 = lsp_mark.clone();
            let lsp_popup3 = lsp_popup.clone();
            let lsp_completing3 = lsp_completing.clone();
            let lsp_gen3 = lsp_comp_gen.clone();
            let on_comp_cb = self.on_completion_needed.clone();
            let path_for_lsp = path.clone();
            buffer.connect_changed(move |buf| {
                if *lsp_completing3.borrow() {
                    return;
                }
                let cursor_pos = buf.cursor_position();
                let cursor_iter = buf.iter_at_offset(cursor_pos);
                let mut temp = cursor_iter.clone();
                let mut found_hash = false;
                let mut hash_iter = cursor_iter.clone();

                loop {
                    if !temp.backward_char() {
                        break;
                    }
                    let ch = temp.char();
                    if ch == '#' {
                        found_hash = true;
                        hash_iter = temp.clone();
                        break;
                    }
                    if !(ch.is_alphanumeric() || ch == '_' || ch == '-') {
                        break;
                    }
                }

                if found_hash {
                    // Track the '#' position
                    {
                        let mut mark_ref = lsp_mark3.borrow_mut();
                        match mark_ref.as_ref() {
                            Some(m) => buf.move_mark(m, &hash_iter),
                            None => {
                                *mark_ref =
                                    Some(buf.create_mark(None::<&str>, &hash_iter, true))
                            }
                        }
                    }

                    let line = cursor_iter.line() as u32 + 1;
                    let col = cursor_iter.line_offset() as u32 + 1;

                    *lsp_gen3.borrow_mut() += 1;
                    let my_gen = *lsp_gen3.borrow();
                    let gen4 = lsp_gen3.clone();
                    let ocb = on_comp_cb.clone();
                    let p = path_for_lsp.clone();

                    glib::timeout_add_local(Duration::from_millis(150), move || {
                        if *gen4.borrow() == my_gen {
                            if let Some(f) = ocb.borrow().as_ref() {
                                f(p.clone(), line, col);
                            }
                        }
                        glib::ControlFlow::Break
                    });
                } else {
                    // No longer in # context — clear mark and hide popup
                    if let Some(m) = lsp_mark3.borrow_mut().take() {
                        buf.delete_mark(&m);
                    }
                    lsp_popup3.hide();
                }
            });
        }

        // ── Key controller ────────────────────────────────────────────────────

        let bib_popup_key = bib_popup.clone();
        let lsp_popup_key = lsp_popup.clone();
        let buf_key = buffer.clone();
        let mark_key = ac_mark.clone();
        let lsp_mark_key = lsp_mark.clone();
        let completing_key = completing.clone();
        let lsp_completing_key = lsp_completing.clone();
        let view_key = view.clone();

        let key_ctrl = EventControllerKey::new();
        key_ctrl.set_propagation_phase(PropagationPhase::Capture);
        key_ctrl.connect_key_pressed(move |_, key, _, _mods| {
            use gtk4::gdk::Key;

            // LSP popup takes priority
            if lsp_popup_key.is_visible() {
                return match key {
                    Key::Escape => {
                        if let Some(m) = lsp_mark_key.borrow_mut().take() {
                            buf_key.delete_mark(&m);
                        }
                        lsp_popup_key.hide();
                        glib::Propagation::Stop
                    }
                    Key::Tab => {
                        let item = lsp_popup_key
                            .selected_item()
                            .or_else(|| lsp_popup_key.first_item());
                        if let Some(i) = item {
                            do_lsp_complete(
                                &buf_key,
                                &lsp_mark_key,
                                &lsp_completing_key,
                                &lsp_popup_key,
                                &view_key,
                                i,
                            );
                        }
                        glib::Propagation::Stop
                    }
                    Key::Return => {
                        if let Some(i) = lsp_popup_key.selected_item() {
                            do_lsp_complete(
                                &buf_key,
                                &lsp_mark_key,
                                &lsp_completing_key,
                                &lsp_popup_key,
                                &view_key,
                                i,
                            );
                            glib::Propagation::Stop
                        } else {
                            glib::Propagation::Proceed
                        }
                    }
                    Key::Down => {
                        lsp_popup_key.move_selection(1);
                        glib::Propagation::Stop
                    }
                    Key::Up => {
                        lsp_popup_key.move_selection(-1);
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                };
            }

            // Bib popup
            if !bib_popup_key.is_visible() {
                return glib::Propagation::Proceed;
            }
            match key {
                Key::Escape => {
                    dismiss_popup_only(&bib_popup_key, &buf_key, &mark_key);
                    glib::Propagation::Stop
                }
                Key::Tab => {
                    let chosen = bib_popup_key
                        .selected_key()
                        .or_else(|| bib_popup_key.first_filtered_key());
                    if let Some(k) = chosen {
                        do_bib_complete(
                            &buf_key, &mark_key, &completing_key, &bib_popup_key, &view_key, &k,
                        );
                    }
                    glib::Propagation::Stop
                }
                Key::Return => {
                    if let Some(k) = bib_popup_key.selected_key() {
                        do_bib_complete(
                            &buf_key, &mark_key, &completing_key, &bib_popup_key, &view_key, &k,
                        );
                        glib::Propagation::Stop
                    } else {
                        glib::Propagation::Proceed
                    }
                }
                Key::Down => {
                    bib_popup_key.move_selection(1);
                    glib::Propagation::Stop
                }
                Key::Up => {
                    bib_popup_key.move_selection(-1);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        view.add_controller(key_ctrl);

        // ── Insert into notebook ──────────────────────────────────────────────

        let page_index = self.notebook.append_page(&scroll, Some(&tab_box));
        self.notebook.set_tab_reorderable(&scroll, true);

        let path_for_callback = path.clone();
        let content_for_callback = content.to_string();

        self.state.borrow_mut().tabs.insert(
            path,
            EditorTab {
                buffer,
                view,
                scroll_window: scroll,
                modified: false,
                dot_label,
                lsp_popup,
            },
        );

        self.notebook.set_current_page(Some(page_index));
        set_wc_text(&self.word_count_label, content);

        // Explicitly fire page_switch so title/outline update even when this is
        // the first tab (connect_switch_page fires before the tab is in state.tabs).
        if let Some(f) = self.on_page_switch.borrow().as_ref() {
            f(content_for_callback.clone(), path_for_callback.clone());
        }

        if let Some(f) = self.on_file_opened.borrow().as_ref() {
            f(path_for_callback, content_for_callback);
        }
    }

    #[allow(dead_code)]
    pub fn close_file(&self, path: &PathBuf) {
        let mut state = self.state.borrow_mut();
        if let Some(tab) = state.tabs.remove(path) {
            if let Some(n) = self.notebook.page_num(&tab.scroll_window) {
                self.notebook.remove_page(Some(n));
            }
        }
    }

    pub fn get_active_content(&self) -> Option<String> {
        let current = self.notebook.current_page()?;
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if let Some(n) = self.notebook.page_num(&tab.scroll_window) {
                if n == current {
                    let (start, end) = tab.buffer.bounds();
                    return Some(tab.buffer.text(&start, &end, false).to_string());
                }
            }
        }
        None
    }

    #[allow(dead_code)]
    pub fn set_active_content(&self, text: &str) {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return,
        };
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if let Some(n) = self.notebook.page_num(&tab.scroll_window) {
                if n == current {
                    tab.buffer.set_text(text);
                    return;
                }
            }
        }
    }

    pub fn switch_to_file(&self, path: &PathBuf) {
        let state = self.state.borrow();
        if let Some(tab) = state.tabs.get(path) {
            if let Some(n) = self.notebook.page_num(&tab.scroll_window) {
                self.notebook.set_current_page(Some(n));
            }
        }
    }

    pub fn mark_saved(&self, path: &PathBuf) {
        let mut state = self.state.borrow_mut();
        if let Some(tab) = state.tabs.get_mut(path) {
            tab.modified = false;
            tab.dot_label.set_visible(false);
        }
    }

    pub fn get_open_paths_ordered(&self) -> Vec<PathBuf> {
        let state = self.state.borrow();
        let mut pages: Vec<(u32, PathBuf)> = state
            .tabs
            .iter()
            .filter_map(|(path, tab)| {
                self.notebook.page_num(&tab.scroll_window).map(|n| (n, path.clone()))
            })
            .collect();
        pages.sort_by_key(|(n, _)| *n);
        pages.into_iter().map(|(_, p)| p).collect()
    }

    pub fn get_cursor_positions(&self) -> std::collections::HashMap<PathBuf, i32> {
        let state = self.state.borrow();
        state.tabs.iter().map(|(path, tab)| {
            (path.clone(), tab.buffer.cursor_position())
        }).collect()
    }

    pub fn restore_cursor(&self, path: &PathBuf, offset: i32) {
        let state = self.state.borrow();
        if let Some(tab) = state.tabs.get(path) {
            let clamped = offset.min(tab.buffer.char_count());
            let iter = tab.buffer.iter_at_offset(clamped);
            tab.buffer.place_cursor(&iter);
        }
    }

    pub fn get_active_path(&self) -> Option<PathBuf> {
        let current = self.notebook.current_page()?;
        let state = self.state.borrow();
        for (path, tab) in &state.tabs {
            if let Some(n) = self.notebook.page_num(&tab.scroll_window) {
                if n == current {
                    return Some(path.clone());
                }
            }
        }
        None
    }

    pub fn save_all_modified(&self) {
        let mut state = self.state.borrow_mut();
        for (path, tab) in state.tabs.iter_mut() {
            if !tab.modified {
                continue;
            }
            let (start, end) = tab.buffer.bounds();
            let content = tab.buffer.text(&start, &end, false);
            if std::fs::write(path, content.as_bytes()).is_ok() {
                tab.modified = false;
                tab.dot_label.set_visible(false);
            }
        }
    }

    pub fn save_current(&self) -> Option<PathBuf> {
        let path = self.get_active_path()?;
        let content = self.get_active_content()?;
        std::fs::write(&path, content.as_bytes()).ok()?;
        self.mark_saved(&path);
        Some(path)
    }

    pub fn next_tab(&self) {
        let n = self.notebook.n_pages();
        if n < 2 {
            return;
        }
        let current = self.notebook.current_page().unwrap_or(0);
        self.notebook.set_current_page(Some((current + 1) % n));
    }

    pub fn prev_tab(&self) {
        let n = self.notebook.n_pages();
        if n < 2 {
            return;
        }
        let current = self.notebook.current_page().unwrap_or(0);
        let prev = if current == 0 { n - 1 } else { current - 1 };
        self.notebook.set_current_page(Some(prev));
    }

    pub fn jump_to_line(&self, path: &PathBuf, line: u32) {
        self.switch_to_file(path);
        let state = self.state.borrow();
        if let Some(tab) = state.tabs.get(path) {
            let line_idx = line.saturating_sub(1) as i32;
            let line_start = tab.buffer.iter_at_line(line_idx).unwrap_or_else(|| {
                let (_, end) = tab.buffer.bounds();
                end
            });
            let mut line_end = line_start;
            line_end.forward_to_line_end();
            // Select the heading text so it's visually highlighted
            tab.buffer.select_range(&line_start, &line_end);
            // Scroll so the heading is vertically centered
            let mut scroll_iter = line_start;
            tab.view.scroll_to_iter(&mut scroll_iter, 0.0, true, 0.0, 0.5);
        }
    }

    fn active_view_buffer(&self) -> Option<(View, Buffer)> {
        let current = self.notebook.current_page()?;
        let state = self.state.borrow();
        for tab in state.tabs.values() {
            if self.notebook.page_num(&tab.scroll_window) == Some(current) {
                return Some((tab.view.clone(), tab.buffer.clone()));
            }
        }
        None
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

fn ensure_diag_tags(buffer: &Buffer) {
    let table = buffer.tag_table();
    if table.lookup("zerkalo-diag-error").is_none() {
        let tag = TextTag::new(Some("zerkalo-diag-error"));
        tag.set_underline(gtk4::pango::Underline::Error);
        tag.set_underline_rgba(Some(&gtk4::gdk::RGBA::new(0.9, 0.2, 0.2, 1.0)));
        table.add(&tag);
    }
    if table.lookup("zerkalo-diag-warning").is_none() {
        let tag = TextTag::new(Some("zerkalo-diag-warning"));
        tag.set_underline(gtk4::pango::Underline::SingleLine);
        tag.set_underline_rgba(Some(&gtk4::gdk::RGBA::new(0.85, 0.72, 0.1, 1.0)));
        table.add(&tag);
    }
}

fn lsp_hash_prefix(buffer: &Buffer) -> String {
    let cursor = buffer.iter_at_offset(buffer.cursor_position());
    let mut temp = cursor.clone();
    loop {
        if !temp.backward_char() {
            break;
        }
        let ch = temp.char();
        if ch == '#' {
            return buffer
                .text(&temp, &cursor, false)
                .to_string()
                .trim_start_matches('#')
                .to_lowercase();
        }
        if !(ch.is_alphanumeric() || ch == '_' || ch == '-') {
            break;
        }
    }
    String::new()
}

fn set_wc_text(label: &Label, text: &str) {
    let words = count_content_words(text);
    let reading = if words < 200 { "< 1 min".to_string() } else { format!("{} min", words / 200) };
    label.set_text(&format!("{words} words · {reading} read"));
}

fn count_content_words(text: &str) -> usize {
    strip_typst_markup(text).split_whitespace().count()
}

fn strip_typst_markup(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    let mut in_raw_block = false;
    let mut in_block_comment = false;

    while i < n {
        let c = chars[i];

        if in_raw_block {
            if c == '`' && chars.get(i + 1) == Some(&'`') && chars.get(i + 2) == Some(&'`') {
                in_raw_block = false;
                i += 3;
            } else {
                i += 1;
            }
            continue;
        }

        if in_block_comment {
            if c == '*' && chars.get(i + 1) == Some(&'/') {
                in_block_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        // Open raw block ```
        if c == '`' && chars.get(i + 1) == Some(&'`') && chars.get(i + 2) == Some(&'`') {
            in_raw_block = true;
            i += 3;
            while i < n && chars[i] != '\n' { i += 1; }
            continue;
        }

        // Line comment //
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < n && chars[i] != '\n' { i += 1; }
            continue;
        }

        // Block comment /*
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            in_block_comment = true;
            i += 2;
            continue;
        }

        // Heading lines starting with =
        let at_line_start = out.is_empty() || out.ends_with('\n');
        if at_line_start && c == '=' {
            while i < n && chars[i] != '\n' { i += 1; }
            continue;
        }

        // Inline raw `...`
        if c == '`' {
            i += 1;
            while i < n && chars[i] != '`' && chars[i] != '\n' { i += 1; }
            if i < n && chars[i] == '`' { i += 1; }
            out.push(' ');
            continue;
        }

        // Math $...$
        if c == '$' {
            i += 1;
            while i < n && chars[i] != '$' { i += 1; }
            if i < n { i += 1; }
            out.push(' ');
            continue;
        }

        // Citation reference @key
        if c == '@' {
            i += 1;
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-' || chars[i] == ':') {
                i += 1;
            }
            continue;
        }

        // Hash function calls: skip #ident and (...){...} args, but KEEP text in [...] args
        if c == '#' {
            i += 1;
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-' || chars[i] == '.') {
                i += 1;
            }
            while i < n && matches!(chars[i], '[' | '(' | '{') {
                if chars[i] == '[' {
                    // Content block — recursively strip and keep the text
                    let end = skip_balanced_typst(&chars, i, n);
                    if end > i + 1 {
                        let inner: String = chars[i + 1..end - 1].iter().collect();
                        out.push_str(&strip_typst_markup(&inner));
                    }
                    out.push(' ');
                    i = end;
                } else {
                    i = skip_balanced_typst(&chars, i, n);
                }
            }
            continue;
        }

        // Label syntax <label>
        if c == '<' {
            while i < n && chars[i] != '>' && chars[i] != '\n' { i += 1; }
            if i < n && chars[i] == '>' { i += 1; }
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

fn skip_balanced_typst(chars: &[char], start: usize, n: usize) -> usize {
    let open = chars[start];
    let close = match open { '[' => ']', '(' => ')', '{' => '}', _ => return start + 1 };
    let mut i = start + 1;
    let mut depth = 1usize;
    while i < n && depth > 0 {
        if chars[i] == open { depth += 1; }
        else if chars[i] == close { depth -= 1; }
        i += 1;
    }
    i
}

fn strip_snippets(s: &str) -> String {
    // Remove LSP snippet placeholders: $0, $1, ${1:...}, etc.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' {
            match chars.peek() {
                Some('{') => {
                    chars.next(); // consume '{'
                    // skip until matching '}'
                    for c in chars.by_ref() {
                        if c == '}' {
                            break;
                        }
                    }
                }
                Some(c) if c.is_ascii_digit() => {
                    // consume digits
                    while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        chars.next();
                    }
                }
                _ => out.push(ch),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_whole_word(start: &gtk4::TextIter, end: &gtk4::TextIter) -> bool {
    let before_ok = if start.offset() == 0 {
        true
    } else {
        let mut before = start.clone();
        before.backward_char();
        !is_word_char(before.char())
    };
    let after_ok = end.is_end() || !is_word_char(end.char());
    before_ok && after_ok
}

// Returns the 1-based line number of the nearest heading at or above `line_idx`,
// or u32::MAX if none found.
fn find_heading_line_for(buf: &sourceview5::Buffer, line_idx: i32) -> u32 {
    let mut check = line_idx;
    while check >= 0 {
        if let Some(iter) = buf.iter_at_line(check) {
            let mut end = iter.clone();
            end.forward_to_line_end();
            let text = buf.text(&iter, &end, false);
            if text.starts_with('=') {
                return (check + 1) as u32;
            }
        }
        check -= 1;
    }
    u32::MAX
}

fn dismiss_popup(
    buf: &Buffer,
    popup: &BibPopup,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
) {
    if let Some(m) = mark.borrow_mut().take() {
        buf.delete_mark(&m);
    }
    popup.hide();
}

fn dismiss_popup_only(
    popup: &BibPopup,
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
) {
    if let Some(m) = mark.borrow_mut().take() {
        buf.delete_mark(&m);
    }
    popup.hide();
}

fn do_bib_complete(
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
    completing: &Rc<RefCell<bool>>,
    popup: &BibPopup,
    view: &View,
    key: &str,
) {
    *completing.borrow_mut() = true;
    let mark_opt = mark.borrow().clone();
    if let Some(ref m) = mark_opt {
        let mut start = buf.iter_at_mark(m);
        let mut end = buf.iter_at_offset(buf.cursor_position());
        buf.begin_user_action();
        buf.delete(&mut start, &mut end);
        buf.insert_at_cursor(&format!("@{key}"));
        buf.end_user_action();
        buf.delete_mark(m);
    }
    *mark.borrow_mut() = None;
    popup.hide();
    view.grab_focus();
    *completing.borrow_mut() = false;
}

fn apply_space_drawer(view: &View, enabled: bool) {
    let sd = view.space_drawer();
    sd.set_enable_matrix(enabled);
    if enabled {
        sd.set_types_for_locations(
            sourceview5::SpaceLocationFlags::ALL,
            sourceview5::SpaceTypeFlags::SPACE | sourceview5::SpaceTypeFlags::TAB,
        );
    } else {
        sd.set_types_for_locations(
            sourceview5::SpaceLocationFlags::ALL,
            sourceview5::SpaceTypeFlags::empty(),
        );
    }
}

fn do_lsp_complete(
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
    completing: &Rc<RefCell<bool>>,
    popup: &LspPopup,
    view: &View,
    item: CompletionItem,
) {
    *completing.borrow_mut() = true;
    let mark_opt = mark.borrow().clone();
    if let Some(ref m) = mark_opt {
        let mut start = buf.iter_at_mark(m);
        let mut end = buf.iter_at_offset(buf.cursor_position());
        let raw = item.insert_text.as_deref().unwrap_or(&item.label);
        let cleaned = strip_snippets(raw);
        let final_text = if cleaned.starts_with('#') {
            cleaned
        } else {
            format!("#{cleaned}")
        };
        buf.begin_user_action();
        buf.delete(&mut start, &mut end);
        buf.insert_at_cursor(&final_text);
        buf.end_user_action();
        buf.delete_mark(m);
    }
    *mark.borrow_mut() = None;
    popup.hide();
    view.grab_focus();
    *completing.borrow_mut() = false;
}

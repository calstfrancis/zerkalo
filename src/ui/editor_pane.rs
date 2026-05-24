use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, CssProvider, EventControllerKey, Label, Notebook, Orientation,
    ScrolledWindow, TextMark, TextWindowType,
};
use sourceview5::prelude::*;
use sourceview5::{Buffer, LanguageManager, View};

use crate::bibliography::BibEntry;
use super::bib_popup::BibPopup;

struct EditorTab {
    buffer: Buffer,
    view: View,
    scroll_window: ScrolledWindow,
    modified: bool,
    dot_label: Label,
}

struct EditorState {
    tabs: HashMap<PathBuf, EditorTab>,
}

#[derive(Clone)]
pub struct EditorPane {
    notebook: Notebook,
    state: Rc<RefCell<EditorState>>,
    on_change: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    bib_entries: Rc<RefCell<Vec<BibEntry>>>,
    font_provider: Rc<CssProvider>,
}

impl EditorPane {
    pub fn new() -> Self {
        let notebook = Notebook::new();
        notebook.set_scrollable(true);
        notebook.set_hexpand(true);
        notebook.set_vexpand(true);

        let state = Rc::new(RefCell::new(EditorState {
            tabs: HashMap::new(),
        }));

        // Single CSS provider registered once; updated in apply_font_size.
        let font_provider = CssProvider::new();
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &font_provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        Self {
            notebook,
            state,
            on_change: Rc::new(RefCell::new(None)),
            bib_entries: Rc::new(RefCell::new(Vec::new())),
            font_provider: Rc::new(font_provider),
        }
    }

    pub fn widget(&self) -> &Notebook {
        &self.notebook
    }

    pub fn set_bib_entries(&self, entries: Vec<BibEntry>) {
        *self.bib_entries.borrow_mut() = entries;
    }

    /// Update the editor font size globally (0 resets to system default).
    pub fn apply_font_size(&self, size: u32) {
        let css = if size > 0 {
            format!("textview {{ font-size: {size}pt; }}")
        } else {
            String::new()
        };
        self.font_provider.load_from_data(&css);
    }

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
        buffer.set_text(content);

        let view = View::with_buffer(&buffer);
        view.set_show_line_numbers(true);
        view.set_auto_indent(true);
        view.set_smart_backspace(true);
        view.set_insert_spaces_instead_of_tabs(true);
        view.set_tab_width(2);
        view.set_indent_width(2);
        view.set_monospace(true);

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&view));
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);

        // ── Tab label ────────────────────────────────────────────────────────

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

        // ── Modified-flag + debounce ─────────────────────────────────────────

        let state_for_change = self.state.clone();
        let path_for_change = path.clone();
        let dot_for_change = dot_label.clone();
        let on_change_cb = self.on_change.clone();
        buffer.connect_changed(move |_| {
            {
                let mut state = state_for_change.borrow_mut();
                if let Some(tab) = state.tabs.get_mut(&path_for_change) {
                    if !tab.modified {
                        tab.modified = true;
                        dot_for_change.set_visible(true);
                    }
                }
            }
            if let Some(f) = on_change_cb.borrow().as_ref() {
                f();
            }
        });

        // ── Autocomplete ─────────────────────────────────────────────────────

        let popup = BibPopup::new(&view, self.bib_entries.clone());

        // Per-tab: mark tracking the '@' position and a re-entry guard
        let ac_mark: Rc<RefCell<Option<TextMark>>> = Rc::new(RefCell::new(None));
        let completing: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        // on_complete: replace @prefix with @key
        let buf_complete = buffer.clone();
        let view_complete = view.clone();
        let mark_complete = ac_mark.clone();
        let completing_complete = completing.clone();
        let popup_complete = popup.clone();
        popup.set_on_complete(move |key| {
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

        // buffer.connect_changed: detect @prefix context
        let view_ac = view.clone();
        let popup_ac = popup.clone();
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

            // Reject email-like: char before @ must not be a word char
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

            // Update or create the mark at the '@' position
            {
                let mut mark_ref = mark_ac.borrow_mut();
                match mark_ref.as_ref() {
                    Some(m) => buf.move_mark(m, &at_iter),
                    None => *mark_ref = Some(buf.create_mark(None::<&str>, &at_iter, true)),
                }
            }

            // Cursor screen position (below the current line)
            let loc = view_ac.iter_location(&cursor_iter);
            let (wx, wy) = view_ac.buffer_to_window_coords(
                TextWindowType::Widget,
                loc.x(),
                loc.y() + loc.height(),
            );

            popup_ac.show_filtered(query, wx, wy);
        });

        // Key controller: Tab = confirm first match, Escape = dismiss,
        // Down/Up = navigate popup list
        let popup_key = popup.clone();
        let buf_key = buffer.clone();
        let mark_key = ac_mark.clone();
        let completing_key = completing.clone();
        let view_key = view.clone();
        let key_ctrl = EventControllerKey::new();
        key_ctrl.connect_key_pressed(move |_, key, _, _mods| {
            use gtk4::gdk::Key;

            if !popup_key.is_visible() {
                return glib::Propagation::Proceed;
            }

            match key {
                Key::Escape => {
                    dismiss_popup_only(&popup_key, &buf_key, &mark_key);
                    glib::Propagation::Stop
                }
                Key::Tab => {
                    let chosen = popup_key.selected_key()
                        .or_else(|| popup_key.first_filtered_key());
                    if let Some(k) = chosen {
                        do_complete(&buf_key, &mark_key, &completing_key, &popup_key, &view_key, &k);
                    }
                    glib::Propagation::Stop
                }
                Key::Return => {
                    if let Some(k) = popup_key.selected_key() {
                        do_complete(&buf_key, &mark_key, &completing_key, &popup_key, &view_key, &k);
                        glib::Propagation::Stop
                    } else {
                        glib::Propagation::Proceed
                    }
                }
                Key::Down => {
                    popup_key.move_selection(1);
                    glib::Propagation::Stop
                }
                Key::Up => {
                    popup_key.move_selection(-1);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        view.add_controller(key_ctrl);

        // ── Insert into notebook ─────────────────────────────────────────────

        let page_index = self.notebook.append_page(&scroll, Some(&tab_box));
        self.notebook.set_tab_reorderable(&scroll, true);

        self.state.borrow_mut().tabs.insert(
            path,
            EditorTab {
                buffer,
                view,
                scroll_window: scroll,
                modified: false,
                dot_label,
            },
        );

        self.notebook.set_current_page(Some(page_index));
    }

    pub fn set_on_change(&self, f: impl Fn() + 'static) {
        *self.on_change.borrow_mut() = Some(Box::new(f));
    }

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

    /// Save all modified buffers to disk and clear their modified flags.
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

    /// Scroll to and place the cursor at `line` (1-based) in the tab for `path`.
    pub fn jump_to_line(&self, path: &PathBuf, line: u32) {
        self.switch_to_file(path);
        let state = self.state.borrow();
        if let Some(tab) = state.tabs.get(path) {
            let line_idx = line.saturating_sub(1) as i32;
            let mut iter = tab.buffer.iter_at_line(line_idx).unwrap_or_else(|| {
                let (_, end) = tab.buffer.bounds();
                end
            });
            tab.buffer.place_cursor(&iter);
            tab.view.scroll_to_iter(&mut iter, 0.1, true, 0.0, 0.3);
        }
    }
}

// ── Autocomplete helpers ──────────────────────────────────────────────────────

fn dismiss_popup(
    buf: &Buffer,
    popup: &BibPopup,
    mark: &Rc<RefCell<Option<TextMark>>>,
) {
    if let Some(m) = mark.borrow_mut().take() {
        buf.delete_mark(&m);
    }
    popup.hide();
}

fn dismiss_popup_only(
    popup: &BibPopup,
    buf: &Buffer,
    mark: &Rc<RefCell<Option<TextMark>>>,
) {
    if let Some(m) = mark.borrow_mut().take() {
        buf.delete_mark(&m);
    }
    popup.hide();
}

fn do_complete(
    buf: &Buffer,
    mark: &Rc<RefCell<Option<TextMark>>>,
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

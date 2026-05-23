use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Notebook, Orientation, ScrolledWindow};
use sourceview5::prelude::*;
use sourceview5::{Buffer, LanguageManager, View};

struct EditorTab {
    buffer: Buffer,
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

        Self { notebook, state }
    }

    pub fn widget(&self) -> &Notebook {
        &self.notebook
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

        let state_for_change = self.state.clone();
        let path_for_change = path.clone();
        let dot_for_change = dot_label.clone();
        buffer.connect_changed(move |_| {
            let mut state = state_for_change.borrow_mut();
            if let Some(tab) = state.tabs.get_mut(&path_for_change) {
                if !tab.modified {
                    tab.modified = true;
                    dot_for_change.set_visible(true);
                }
            }
        });

        let page_index = self.notebook.append_page(&scroll, Some(&tab_box));
        self.notebook.set_tab_reorderable(&scroll, true);

        self.state.borrow_mut().tabs.insert(
            path,
            EditorTab {
                buffer,
                scroll_window: scroll,
                modified: false,
                dot_label,
            },
        );

        self.notebook.set_current_page(Some(page_index));
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
}

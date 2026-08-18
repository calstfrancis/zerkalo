use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Entry, EventControllerKey, Label, ListBox, Orientation, PolicyType,
    PropagationPhase, ScrolledWindow, SelectionMode,
};
use libadwaita as adw;

#[derive(Clone)]
pub struct PaletteItem {
    pub id: String,
    pub label: String,
    pub subtitle: String,
}

#[derive(Clone)]
pub struct CommandPalette {
    window: adw::Window,
    entry: Entry,
    list: ListBox,
    items: Rc<RefCell<Vec<PaletteItem>>>,
    on_activate: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
}

impl CommandPalette {
    pub fn new(parent: &impl IsA<gtk4::Window>) -> Self {
        let window = adw::Window::builder()
            .transient_for(parent)
            .modal(true)
            .default_width(520)
            .default_height(420)
            .title("Command Palette")
            .resizable(false)
            .build();
        window.set_hide_on_close(true);

        let header = adw::HeaderBar::new();
        header.add_css_class("fond-chrome");
        header.set_show_end_title_buttons(false);
        header.set_show_start_title_buttons(false);

        let entry = Entry::new();
        entry.set_placeholder_text(Some("Search commands, headings, files…"));
        entry.set_hexpand(true);
        header.set_title_widget(Some(&entry));

        let list = ListBox::new();
        list.set_selection_mode(SelectionMode::Browse);
        list.add_css_class("navigation-sidebar");

        let scroll = ScrolledWindow::new();
        scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
        scroll.set_child(Some(&list));
        scroll.set_vexpand(true);

        let toolbar = adw::ToolbarView::new();
        toolbar.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&scroll));
        window.set_content(Some(&toolbar));

        let items: Rc<RefCell<Vec<PaletteItem>>> = Rc::new(RefCell::new(Vec::new()));
        let on_activate: Rc<RefCell<Option<Box<dyn Fn(&str)>>>> = Rc::new(RefCell::new(None));

        // Filter list as user types
        {
            let list_c = list.clone();
            let items_c = items.clone();
            entry.connect_changed(move |e| {
                let query = e.text().to_lowercase();
                rebuild_list(&list_c, &items_c.borrow(), &query);
            });
        }

        // Activate on Enter
        {
            let win_c = window.clone();
            let list_c = list.clone();
            let on_act = on_activate.clone();
            entry.connect_activate(move |_| {
                activate_selected(&list_c, &on_act, &win_c);
            });
        }

        // Activate on row click
        {
            let win_c = window.clone();
            let on_act = on_activate.clone();
            list.connect_row_activated(move |_, row| {
                let id = row.widget_name().to_string();
                if !id.is_empty() {
                    if let Some(f) = on_act.borrow().as_ref() {
                        f(&id);
                    }
                    win_c.close();
                }
            });
        }

        // Arrow keys navigate the list
        {
            let list_c = list.clone();
            let kc = EventControllerKey::new();
            kc.set_propagation_phase(PropagationPhase::Capture);
            kc.connect_key_pressed(move |_, key, _, _| {
                use gtk4::gdk::Key;
                match key {
                    Key::Down => {
                        move_selection(&list_c, 1);
                        glib::Propagation::Stop
                    }
                    Key::Up => {
                        move_selection(&list_c, -1);
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            });
            entry.add_controller(kc);
        }

        // Escape closes
        {
            let win_c = window.clone();
            let kc2 = EventControllerKey::new();
            kc2.connect_key_pressed(move |_, key, _, _| {
                if key == gtk4::gdk::Key::Escape {
                    win_c.close();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            window.add_controller(kc2);
        }

        Self {
            window,
            entry,
            list,
            items,
            on_activate,
        }
    }

    pub fn set_on_activate(&self, f: impl Fn(&str) + 'static) {
        *self.on_activate.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_close(&self, f: impl Fn() + 'static) {
        self.window.connect_hide(move |_| f());
    }

    pub fn set_items(&self, items: Vec<PaletteItem>) {
        *self.items.borrow_mut() = items;
    }

    pub fn show(&self) {
        let query = self.entry.text().to_lowercase();
        rebuild_list(&self.list, &self.items.borrow(), &query);
        self.window.present();
        self.entry.grab_focus();
        // Select first item
        if let Some(row) = self.list.row_at_index(0) {
            self.list.select_row(Some(&row));
        }
    }
}

fn rebuild_list(list: &ListBox, items: &[PaletteItem], query: &str) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let mut first = true;
    for item in items {
        if !query.is_empty()
            && !item.label.to_lowercase().contains(query)
            && !item.subtitle.to_lowercase().contains(query)
        {
            continue;
        }

        let row = make_row(item, query);
        list.append(&row);

        if first {
            list.select_row(Some(&row));
            first = false;
        }
    }
}

fn highlight_match(text: &str, query: &str) -> String {
    if query.is_empty() {
        return glib::markup_escape_text(text).to_string();
    }
    let lower_text = text.to_lowercase();
    if let Some(start) = lower_text.find(query) {
        let end = start + query.len();

        // `start`/`end` are byte offsets into `lower_text`, not `text` — case
        // folding can change a character's byte length (Turkish İ, Kelvin
        // sign K, German ß, …), so they aren't guaranteed to land on char
        // boundaries in the original (or even be in range) when such
        // characters appear before or within the match. Clamp into range,
        // then snap outward to the nearest valid boundary — slicing at a
        // non-boundary offset panics.
        let mut start = start.min(text.len());
        let mut end = end.min(text.len());
        while start > 0 && !text.is_char_boundary(start) {
            start -= 1;
        }
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }
        if end < start {
            end = start;
        }

        let prefix = glib::markup_escape_text(&text[..start]);
        let matched = glib::markup_escape_text(&text[start..end]);
        let suffix = glib::markup_escape_text(&text[end..]);
        format!("{}<b>{}</b>{}", prefix, matched, suffix)
    } else {
        glib::markup_escape_text(text).to_string()
    }
}

fn make_row(item: &PaletteItem, query: &str) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    row.set_widget_name(&item.id);

    let hbox = GtkBox::new(Orientation::Horizontal, 8);
    hbox.set_margin_top(6);
    hbox.set_margin_bottom(6);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);

    let vbox = GtkBox::new(Orientation::Vertical, 2);
    vbox.set_hexpand(true);

    let lbl = Label::new(None);
    lbl.set_use_markup(true);
    lbl.set_markup(&highlight_match(&item.label, query));
    lbl.set_xalign(0.0);
    vbox.append(&lbl);

    if !item.subtitle.is_empty() {
        let sub = Label::new(None);
        sub.set_use_markup(true);
        sub.set_markup(&highlight_match(&item.subtitle, query));
        sub.set_xalign(0.0);
        sub.add_css_class("dim-label");
        sub.add_css_class("caption");
        vbox.append(&sub);
    }

    hbox.append(&vbox);
    row.set_child(Some(&hbox));
    row
}

fn activate_selected(
    list: &ListBox,
    on_act: &Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    win: &adw::Window,
) {
    if let Some(row) = list.selected_row() {
        let id = row.widget_name().to_string();
        if !id.is_empty() {
            if let Some(f) = on_act.borrow().as_ref() {
                f(&id);
            }
        }
        win.close();
    }
}

fn move_selection(list: &ListBox, delta: i32) {
    let current = list.selected_row().map(|r| r.index()).unwrap_or(0);
    let next = (current + delta).max(0);
    if let Some(row) = list.row_at_index(next) {
        list.select_row(Some(&row));
        row.grab_focus();
    }
}

// ── Default command items ─────────────────────────────────────────────────────

pub fn default_commands() -> Vec<PaletteItem> {
    vec![
        PaletteItem {
            id: "new_file".into(),
            label: "New File".into(),
            subtitle: "Create a new document in the work folder".into(),
        },
        PaletteItem {
            id: "open_file".into(),
            label: "Open File…".into(),
            subtitle: "Browse to open a file".into(),
        },
        PaletteItem {
            id: "save".into(),
            label: "Save".into(),
            subtitle: "Save the active document (Ctrl+S)".into(),
        },
        PaletteItem {
            id: "export".into(),
            label: "Export…".into(),
            subtitle: "Export to PDF, HTML, DOCX, ODT or LaTeX".into(),
        },
        PaletteItem {
            id: "print".into(),
            label: "Print…".into(),
            subtitle: "Page range, layout and printer (Ctrl+P)".into(),
        },
        PaletteItem {
            id: "toggle_find".into(),
            label: "Find & Replace".into(),
            subtitle: "Toggle the find/replace bar (Ctrl+F)".into(),
        },
        PaletteItem {
            id: "find_in_files".into(),
            label: "Find in Files\u{2026}".into(),
            subtitle: "Search across all project files (Ctrl+Shift+F)".into(),
        },
        PaletteItem {
            id: "project_outline".into(),
            label: "Project Outline".into(),
            subtitle: "Jump to any heading in the current document".into(),
        },
        PaletteItem {
            id: "git_sync".into(),
            label: "Save a Version & Back It Up".into(),
            subtitle: "Save this version and send it to all backup locations (Ctrl+Shift+S)".into(),
        },
        PaletteItem {
            id: "toggle_profile".into(),
            label: "Toggle Profile".into(),
            subtitle: "Switch the preview between Final (full quality) and Draft (fast)".into(),
        },
        PaletteItem {
            id: "browse_snapshots".into(),
            label: "Browse Snapshots\u{2026}".into(),
            subtitle: "Local backups saved automatically every time you save".into(),
        },
        PaletteItem {
            id: "browse_history".into(),
            label: "File History\u{2026}".into(),
            subtitle: "Synced history of earlier versions and what changed".into(),
        },
        PaletteItem {
            id: "settings".into(),
            label: "Settings…".into(),
            subtitle: "Open the settings dialog".into(),
        },
        PaletteItem {
            id: "toggle_preview".into(),
            label: "Toggle Preview".into(),
            subtitle: "Show or hide the live preview pane".into(),
        },
        PaletteItem {
            id: "toggle_sidebar".into(),
            label: "Toggle Sidebar".into(),
            subtitle: "Show or hide the sidebar".into(),
        },
        PaletteItem {
            id: "template".into(),
            label: "New from Template…".into(),
            subtitle: "Choose a document template".into(),
        },
        PaletteItem {
            id: "help".into(),
            label: "Help & Shortcuts".into(),
            subtitle: "Open the help window (Ctrl+?)".into(),
        },
        PaletteItem {
            id: "focus_mode".into(),
            label: "Toggle Focus Mode".into(),
            subtitle: "Dim the sidebar for distraction-free writing".into(),
        },
    ]
}

pub fn heading_items(content: &str, path: &Path) -> Vec<PaletteItem> {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document");
    content
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let trimmed = line.trim_start_matches('=');
            if trimmed.len() < line.len() && line.starts_with('=') {
                let level = line.len() - trimmed.len();
                let title = trimmed.trim().to_string();
                if !title.is_empty() {
                    return Some(PaletteItem {
                        id: format!("heading:{}:{}", i + 1, path.display()),
                        label: format!("{} {}", "=".repeat(level), title),
                        subtitle: format!("Line {} · {}", i + 1, filename),
                    });
                }
            }
            None
        })
        .collect()
}

#[allow(dead_code)]
pub fn recent_file_items(files: &[PathBuf]) -> Vec<PaletteItem> {
    files
        .iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?.to_string();
            let parent = p
                .parent()
                .and_then(|d| d.to_str())
                .unwrap_or("")
                .to_string();
            Some(PaletteItem {
                id: format!("file:{}", p.display()),
                label: name,
                subtitle: parent,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_match_wraps_the_match_in_bold() {
        // Callers always lowercase `query` before calling this (see filter_items).
        let out = highlight_match("hello world", "world");
        assert_eq!(out, "hello <b>world</b>");
    }

    #[test]
    fn highlight_match_does_not_panic_when_lowercasing_shifts_byte_offsets_mid_character() {
        // Same scenario as search_panel's equivalent test: "İ" lowercases to
        // a longer byte sequence, so a naive byte-offset reuse against the
        // original string can slice mid-character and panic.
        let text = "stanİ日";
        let out = highlight_match(text, "stan");
        assert!(
            out.contains("<b>"),
            "should still produce a highlighted result: {out}"
        );
    }

    #[test]
    fn highlight_match_returns_escaped_text_unchanged_when_no_match() {
        let out = highlight_match("<tag> hello", "zzz");
        assert_eq!(out, "&lt;tag&gt; hello");
    }
}

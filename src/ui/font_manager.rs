use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, Entry, Label, Orientation, ScrolledWindow, Separator,
};
use libadwaita as adw;
use adw::prelude::*;

const PREFS_FILE: &str = "font-preferences.toml";

pub struct FontManager {
    window: adw::Window,
}

impl FontManager {
    pub fn new(parent: &adw::ApplicationWindow) -> Self {
        let fonts = list_system_fonts();
        let prefs: Rc<RefCell<BTreeMap<String, bool>>> =
            Rc::new(RefCell::new(load_prefs()));

        // ── Search entry ────────────────────────────────────────────────────
        let search_entry = Entry::new();
        search_entry.set_placeholder_text(Some("Search fonts…"));
        search_entry.set_hexpand(true);
        search_entry.set_margin_start(12);
        search_entry.set_margin_end(12);
        search_entry.set_margin_top(10);
        search_entry.set_margin_bottom(6);

        // ── Scrollable font list ────────────────────────────────────────────
        let list_box = GtkBox::new(Orientation::Vertical, 2);
        list_box.set_margin_start(12);
        list_box.set_margin_end(12);
        list_box.set_margin_bottom(12);

        let prefs_for_list = prefs.clone();
        let fonts_for_filter = fonts.clone();
        let list_box_for_rebuild = list_box.clone();

        let rebuild: Rc<dyn Fn(&str)> = Rc::new(move |query: &str| {
            while let Some(child) = list_box_for_rebuild.first_child() {
                list_box_for_rebuild.remove(&child);
            }
            let q = query.to_lowercase();
            for font in &fonts_for_filter {
                if !q.is_empty() && !font.to_lowercase().contains(&q) {
                    continue;
                }
                let enabled = prefs_for_list.borrow().get(font).copied().unwrap_or(true);
                let row = GtkBox::new(Orientation::Horizontal, 8);
                row.set_margin_top(3);
                row.set_margin_bottom(3);

                let cb = CheckButton::new();
                cb.set_active(enabled);
                cb.set_valign(Align::Center);

                let lbl = Label::new(Some(font));
                lbl.set_xalign(0.0);
                lbl.set_hexpand(true);
                lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                row.append(&cb);
                row.append(&lbl);
                list_box_for_rebuild.append(&row);

                let prefs_c = prefs_for_list.clone();
                let font_c = font.clone();
                cb.connect_toggled(move |btn| {
                    prefs_c.borrow_mut().insert(font_c.clone(), btn.is_active());
                });
            }
        });

        rebuild("");

        let rebuild_for_search = rebuild.clone();
        search_entry.connect_changed(move |entry| {
            rebuild_for_search(&entry.text());
        });

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&list_box));
        scroll.set_vexpand(true);
        scroll.set_min_content_height(480);

        // ── Buttons bar ─────────────────────────────────────────────────────
        let enable_all_btn = Button::with_label("Enable All");
        enable_all_btn.add_css_class("flat");
        let prefs_for_all = prefs.clone();
        let fonts_for_all = fonts.clone();
        let rebuild_for_all = rebuild.clone();
        let search_for_all = search_entry.clone();
        enable_all_btn.connect_clicked(move |_| {
            for font in &fonts_for_all {
                prefs_for_all.borrow_mut().insert(font.clone(), true);
            }
            rebuild_for_all(&search_for_all.text());
        });

        let disable_all_btn = Button::with_label("Disable All");
        disable_all_btn.add_css_class("flat");
        let prefs_for_none = prefs.clone();
        let fonts_for_none = fonts.clone();
        let rebuild_for_none = rebuild.clone();
        let search_for_none = search_entry.clone();
        disable_all_btn.connect_clicked(move |_| {
            for font in &fonts_for_none {
                prefs_for_none.borrow_mut().insert(font.clone(), false);
            }
            rebuild_for_none(&search_for_none.text());
        });

        let btn_row = GtkBox::new(Orientation::Horizontal, 8);
        btn_row.set_margin_start(12);
        btn_row.set_margin_end(12);
        btn_row.set_margin_top(6);
        btn_row.set_margin_bottom(6);
        btn_row.append(&enable_all_btn);
        btn_row.append(&disable_all_btn);

        // ── Header bar ──────────────────────────────────────────────────────
        let header = adw::HeaderBar::new();
        let save_btn = Button::with_label("Save");
        save_btn.add_css_class("suggested-action");
        header.pack_end(&save_btn);

        // ── Layout ──────────────────────────────────────────────────────────
        let content_box = GtkBox::new(Orientation::Vertical, 0);
        content_box.append(&search_entry);
        content_box.append(&Separator::new(Orientation::Horizontal));
        content_box.append(&scroll);
        content_box.append(&Separator::new(Orientation::Horizontal));
        content_box.append(&btn_row);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content_box));

        let win = adw::Window::new();
        win.set_title(Some("Font Management"));
        win.set_default_width(420);
        win.set_default_height(640);
        win.set_transient_for(Some(parent));
        win.set_modal(true);
        win.set_content(Some(&toolbar));

        let prefs_for_save = prefs.clone();
        let win_for_save = win.clone();
        save_btn.connect_clicked(move |_| {
            save_prefs(&prefs_for_save.borrow());
            win_for_save.close();
        });

        Self { window: win }
    }

    pub fn present(&self) {
        self.window.present();
    }

    /// Returns the list of system fonts the user has not explicitly disabled.
    pub fn enabled_fonts() -> Vec<String> {
        let prefs = load_prefs();
        let mut fonts = list_system_fonts();
        if prefs.is_empty() {
            return fonts;
        }
        fonts.retain(|f| prefs.get(f).copied().unwrap_or(true));
        fonts
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn list_system_fonts() -> Vec<String> {
    let output = std::process::Command::new("fc-list")
        .args([":", "family"])
        .output()
        .ok();
    let mut fonts: Vec<String> = output
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .lines()
        .flat_map(|line| line.split(','))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    fonts.sort();
    fonts.dedup();
    fonts
}

fn prefs_path() -> PathBuf {
    glib::user_config_dir().join("zerkalo").join(PREFS_FILE)
}

fn load_prefs() -> BTreeMap<String, bool> {
    let path = prefs_path();
    let content = std::fs::read_to_string(path).unwrap_or_default();
    toml::from_str(&content).unwrap_or_default()
}

fn save_prefs(prefs: &BTreeMap<String, bool>) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = toml::to_string(prefs) {
        let _ = std::fs::write(path, content);
    }
}

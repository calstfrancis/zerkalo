use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Button};
use libadwaita as adw;

use crate::config::{Config, Theme};

pub struct SettingsDialog {
    window: adw::Window,
    on_save: Rc<RefCell<Option<Box<dyn Fn(Config)>>>>,
}

impl SettingsDialog {
    pub fn new(parent: &impl IsA<gtk4::Window>, current: &Config) -> Self {
        let window = adw::Window::builder()
            .title("Settings")
            .transient_for(parent)
            .modal(true)
            .default_width(480)
            .resizable(false)
            .build();

        let on_save: Rc<RefCell<Option<Box<dyn Fn(Config)>>>> = Rc::new(RefCell::new(None));

        // ── Header bar ──────────────────────────────────────────────────────

        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);

        let cancel_btn = Button::with_label("Cancel");
        header.pack_start(&cancel_btn);

        let save_btn = Button::with_label("Save");
        save_btn.add_css_class("suggested-action");
        header.pack_end(&save_btn);

        // ── Bibliography group ───────────────────────────────────────────────

        let bib_group = adw::PreferencesGroup::new();
        bib_group.set_title("Bibliography");

        let bib_row = adw::EntryRow::new();
        bib_row.set_title("Bib file");
        if let Some(ref p) = current.bib_path {
            bib_row.set_text(p.to_str().unwrap_or(""));
        }

        let browse_btn = Button::from_icon_name("document-open-symbolic");
        browse_btn.set_valign(Align::Center);
        browse_btn.add_css_class("flat");
        let bib_row_browse = bib_row.clone();
        let window_browse = window.clone();
        browse_btn.connect_clicked(move |_| {
            let row = bib_row_browse.clone();
            let fd = gtk4::FileDialog::new();
            fd.open(
                Some(&window_browse),
                None::<&gtk4::gio::Cancellable>,
                move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            row.set_text(path.to_str().unwrap_or(""));
                        }
                    }
                },
            );
        });
        bib_row.add_suffix(&browse_btn);
        bib_group.add(&bib_row);

        // ── Editor group ─────────────────────────────────────────────────────

        let editor_group = adw::PreferencesGroup::new();
        editor_group.set_title("Editor");

        let font_spin = adw::SpinRow::with_range(8.0, 72.0, 1.0);
        font_spin.set_title("Font size");
        font_spin.set_subtitle("Points");
        font_spin.set_value(current.editor_font_size as f64);

        let theme_model = gtk4::StringList::new(&["System", "Light", "Dark"]);
        let theme_row = adw::ComboRow::new();
        theme_row.set_title("Theme");
        theme_row.set_model(Some(&theme_model));
        let theme_idx = match current.theme {
            Theme::System => 0u32,
            Theme::Light => 1u32,
            Theme::Dark => 2u32,
        };
        theme_row.set_selected(theme_idx);

        editor_group.add(&font_spin);
        editor_group.add(&theme_row);

        // ── Compilation group ────────────────────────────────────────────────

        let compile_group = adw::PreferencesGroup::new();
        compile_group.set_title("Compilation");

        let debounce_spin = adw::SpinRow::with_range(100.0, 5000.0, 50.0);
        debounce_spin.set_title("Debounce");
        debounce_spin.set_subtitle("Milliseconds before recompile");
        debounce_spin.set_value(current.debounce_ms as f64);

        let auto_row = adw::SwitchRow::new();
        auto_row.set_title("Auto-compile");
        auto_row.set_subtitle("Recompile automatically on change");
        auto_row.set_active(current.auto_compile);

        compile_group.add(&debounce_spin);
        compile_group.add(&auto_row);

        // ── Preferences page ─────────────────────────────────────────────────

        let page = adw::PreferencesPage::new();
        page.add(&bib_group);
        page.add(&editor_group);
        page.add(&compile_group);

        // ── Toolbar view ─────────────────────────────────────────────────────

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&page));
        window.set_content(Some(&toolbar_view));

        // ── Wiring ──────────────────────────────────────────────────────────

        let win_cancel = window.clone();
        cancel_btn.connect_clicked(move |_| win_cancel.close());

        let on_save_cb = on_save.clone();
        let project_path = current.project_path.clone();
        let win_save = window.clone();
        save_btn.connect_clicked(move |_| {
            let bib_path_text = bib_row.text().trim().to_string();
            let bib_path: Option<PathBuf> = if bib_path_text.is_empty() {
                None
            } else {
                Some(PathBuf::from(bib_path_text))
            };

            let theme = match theme_row.selected() {
                1 => Theme::Light,
                2 => Theme::Dark,
                _ => Theme::System,
            };

            let new_cfg = Config {
                project_path: project_path.clone(),
                bib_path,
                debounce_ms: debounce_spin.value() as u64,
                auto_compile: auto_row.is_active(),
                editor_font_size: font_spin.value() as u32,
                theme,
            };

            if let Err(e) = new_cfg.save() {
                eprintln!("Failed to save config: {e}");
            }

            if let Some(f) = on_save_cb.borrow().as_ref() {
                f(new_cfg);
            }

            win_save.close();
        });

        Self { window, on_save }
    }

    pub fn set_on_save(&self, f: impl Fn(Config) + 'static) {
        *self.on_save.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.window.present();
    }
}

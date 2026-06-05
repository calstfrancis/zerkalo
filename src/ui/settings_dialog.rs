use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Button, Label, Notebook};
use libadwaita as adw;
use adw::prelude::*;

use crate::config::{Config, Theme};

pub struct SettingsDialog {
    window: adw::Window,
    on_save: Rc<RefCell<Option<Box<dyn Fn(Config)>>>>,
    on_preview: Rc<RefCell<Option<Box<dyn Fn(Config)>>>>,
}

impl SettingsDialog {
    pub fn new(parent: &impl IsA<gtk4::Window>, current: &Config) -> Self {
        let window = adw::Window::builder()
            .title("Settings")
            .transient_for(parent)
            .modal(true)
            .default_width(480)
            .default_height(520)
            .resizable(false)
            .build();

        let on_save: Rc<RefCell<Option<Box<dyn Fn(Config)>>>> = Rc::new(RefCell::new(None));
        let on_preview: Rc<RefCell<Option<Box<dyn Fn(Config)>>>> = Rc::new(RefCell::new(None));

        // ── Header bar ──────────────────────────────────────────────────────

        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);

        let cancel_btn = Button::with_label("Cancel");
        header.pack_start(&cancel_btn);

        let save_btn = Button::with_label("Save");
        save_btn.add_css_class("suggested-action");
        header.pack_end(&save_btn);

        // ── Groups ───────────────────────────────────────────────────────────

        // Folders
        let folders_group = adw::PreferencesGroup::new();
        folders_group.set_title("Folders");

        let work_dir_row = adw::EntryRow::new();
        work_dir_row.set_title("Work folder");
        work_dir_row.set_text(current.work_dir.to_str().unwrap_or(""));

        let work_dir_btn = Button::from_icon_name("document-open-symbolic");
        work_dir_btn.set_valign(Align::Center);
        work_dir_btn.add_css_class("flat");
        let work_dir_row_c = work_dir_row.clone();
        let win_c = window.clone();
        work_dir_btn.connect_clicked(move |_| {
            let row = work_dir_row_c.clone();
            let fd = gtk4::FileDialog::new();
            fd.select_folder(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        row.set_text(path.to_str().unwrap_or(""));
                    }
                }
            });
        });
        work_dir_row.add_suffix(&work_dir_btn);
        folders_group.add(&work_dir_row);

        let output_dir_row = adw::EntryRow::new();
        output_dir_row.set_title("Output folder");
        output_dir_row.set_text(
            current.output_dir.as_deref().and_then(|p| p.to_str()).unwrap_or(""),
        );

        let output_dir_btn = Button::from_icon_name("document-open-symbolic");
        output_dir_btn.set_valign(Align::Center);
        output_dir_btn.add_css_class("flat");
        let output_dir_row_c = output_dir_row.clone();
        let win_c2 = window.clone();
        output_dir_btn.connect_clicked(move |_| {
            let row = output_dir_row_c.clone();
            let fd = gtk4::FileDialog::new();
            fd.select_folder(Some(&win_c2), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        row.set_text(path.to_str().unwrap_or(""));
                    }
                }
            });
        });
        output_dir_row.add_suffix(&output_dir_btn);
        folders_group.add(&output_dir_row);

        // Compilation
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

        // Appearance
        let editor_group = adw::PreferencesGroup::new();
        editor_group.set_title("Appearance");

        let theme_model = gtk4::StringList::new(&["System", "Light", "Dark"]);
        let theme_row = adw::ComboRow::new();
        theme_row.set_title("Color scheme");
        theme_row.set_model(Some(&theme_model));
        let theme_idx = match current.theme {
            Theme::System => 0u32,
            Theme::Light => 1u32,
            Theme::Dark => 2u32,
        };
        theme_row.set_selected(theme_idx);
        editor_group.add(&theme_row);

        // Editor
        let font_group = adw::PreferencesGroup::new();
        font_group.set_title("Editor");

        let font_desc = gtk4::pango::FontDescription::from_string(
            &format!("{} {}", current.editor_font_family, current.editor_font_size),
        );
        let font_dialog = gtk4::FontDialog::new();
        let font_btn = gtk4::FontDialogButton::new(Some(font_dialog));
        font_btn.set_font_desc(&font_desc);
        font_btn.set_valign(Align::Center);
        let font_row = adw::ActionRow::new();
        font_row.set_title("Editor font");
        font_row.set_subtitle("Family and size");
        font_row.add_suffix(&font_btn);
        font_row.set_activatable_widget(Some(&font_btn));

        let tab_spin = adw::SpinRow::with_range(1.0, 8.0, 1.0);
        tab_spin.set_title("Tab width");
        tab_spin.set_subtitle("Spaces");
        tab_spin.set_value(current.editor_tab_width as f64);

        let wrap_row = adw::SwitchRow::new();
        wrap_row.set_title("Word wrap");
        wrap_row.set_active(current.editor_word_wrap);

        let ws_row = adw::SwitchRow::new();
        ws_row.set_title("Show whitespace");
        ws_row.set_active(current.editor_show_whitespace);

        let spacing_model = gtk4::StringList::new(&["Compact (0 px)", "Normal (2 px)", "Spacious (6 px)"]);
        let spacing_row = adw::ComboRow::new();
        spacing_row.set_title("Line spacing");
        spacing_row.set_subtitle("Extra pixels above and below each line");
        spacing_row.set_model(Some(&spacing_model));
        let spacing_idx = match current.editor_line_spacing {
            0 => 0u32,
            6 => 2u32,
            _ => 1u32,
        };
        spacing_row.set_selected(spacing_idx);

        let typewriter_row = adw::SwitchRow::new();
        typewriter_row.set_title("Typewriter scrolling");
        typewriter_row.set_subtitle("Keep the cursor vertically centred as you type");
        typewriter_row.set_active(current.typewriter_scrolling);

        let high_contrast_row = adw::SwitchRow::new();
        high_contrast_row.set_title("High contrast mode");
        high_contrast_row.set_subtitle("Add extra CSS contrast to the editor and UI");
        high_contrast_row.set_active(current.high_contrast);

        font_group.add(&font_row);
        font_group.add(&tab_spin);
        font_group.add(&wrap_row);
        font_group.add(&ws_row);
        font_group.add(&spacing_row);
        font_group.add(&typewriter_row);
        font_group.add(&high_contrast_row);

        // Bibliography
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
            fd.open(Some(&window_browse), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        row.set_text(path.to_str().unwrap_or(""));
                    }
                }
            });
        });
        bib_row.add_suffix(&browse_btn);
        bib_group.add(&bib_row);

        // Spell check
        let spell_group = adw::PreferencesGroup::new();
        spell_group.set_title("Spell Check");

        let spell_enabled_row = adw::SwitchRow::new();
        spell_enabled_row.set_title("Enable spell check");
        spell_enabled_row.set_active(current.spell_enabled);

        let spell_autocorrect_row = adw::SwitchRow::new();
        spell_autocorrect_row.set_title("Autocorrect");
        spell_autocorrect_row.set_subtitle("Replace on word boundary (edit distance ≤ 1)");
        spell_autocorrect_row.set_active(current.spell_autocorrect);

        let available_langs = crate::spellcheck::SpellChecker::available_languages();
        let lang_strings: Vec<&str> = available_langs.iter().map(|s| s.as_str()).collect();
        let lang_model = gtk4::StringList::new(&lang_strings);
        let lang_row = adw::ComboRow::new();
        lang_row.set_title("Dictionary language");
        lang_row.set_model(Some(&lang_model));
        let current_lang_idx = available_langs
            .iter()
            .position(|l| l == &current.spell_language)
            .unwrap_or(0) as u32;
        lang_row.set_selected(current_lang_idx);

        spell_group.add(&spell_enabled_row);
        spell_group.add(&spell_autocorrect_row);
        spell_group.add(&lang_row);

        // ── Tabs ─────────────────────────────────────────────────────────────

        let notebook = Notebook::new();
        notebook.set_tab_pos(gtk4::PositionType::Top);
        notebook.set_vexpand(true);

        // Developer mode
        let dev_group = adw::PreferencesGroup::new();
        dev_group.set_title("Advanced");
        let dev_mode_row = adw::SwitchRow::new();
        dev_mode_row.set_title("Developer mode");
        dev_mode_row.set_subtitle("Show experimental features (Import…)");
        dev_mode_row.set_active(current.developer_mode);
        dev_group.add(&dev_mode_row);

        let page_general = adw::PreferencesPage::new();
        page_general.add(&folders_group);
        page_general.add(&compile_group);
        page_general.add(&dev_group);
        notebook.append_page(&page_general, Some(&Label::new(Some("General"))));

        let page_editor = adw::PreferencesPage::new();
        page_editor.add(&editor_group);
        page_editor.add(&font_group);
        notebook.append_page(&page_editor, Some(&Label::new(Some("Editor"))));

        let page_extras = adw::PreferencesPage::new();
        page_extras.add(&bib_group);
        page_extras.add(&spell_group);
        notebook.append_page(&page_extras, Some(&Label::new(Some("Extras"))));

        // ── Toolbar view ─────────────────────────────────────────────────────

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&notebook));
        window.set_content(Some(&toolbar_view));

        // ── Wiring ──────────────────────────────────────────────────────────

        let win_cancel = window.clone();
        let on_preview_cancel = on_preview.clone();
        let revert_cfg = current.clone();
        cancel_btn.connect_clicked(move |_| {
            // Revert appearance to original config on Cancel
            if let Some(f) = on_preview_cancel.borrow().as_ref() {
                f(revert_cfg.clone());
            }
            win_cancel.close();
        });

        // Helper that reads the current dialog state into a Config
        let build_config = {
            let work_dir_row = work_dir_row.clone();
            let output_dir_row = output_dir_row.clone();
            let bib_row = bib_row.clone();
            let theme_row = theme_row.clone();
            let font_btn = font_btn.clone();
            let debounce_spin = debounce_spin.clone();
            let auto_row = auto_row.clone();
            let tab_spin = tab_spin.clone();
            let wrap_row = wrap_row.clone();
            let ws_row = ws_row.clone();
            let spacing_row = spacing_row.clone();
            let typewriter_row = typewriter_row.clone();
            let high_contrast_row = high_contrast_row.clone();
            let spell_enabled_row = spell_enabled_row.clone();
            let spell_autocorrect_row = spell_autocorrect_row.clone();
            let lang_row = lang_row.clone();
            let available_langs = available_langs.clone();
            let dev_mode_row = dev_mode_row.clone();
            let recent_files_cur = current.recent_files.clone();
            let recent_projects_cur = current.recent_projects.clone();
            let preview_zoom_cur = current.preview_zoom;
            let sidebar_width_cur = current.sidebar_width;
            let preview_split_cur = current.preview_split;
            move || {
                let work_dir_text = work_dir_row.text().trim().to_string();
                let work_dir = if work_dir_text.is_empty() {
                    crate::config::default_work_dir_pub()
                } else {
                    PathBuf::from(work_dir_text)
                };
                let output_dir_text = output_dir_row.text().trim().to_string();
                let output_dir: Option<PathBuf> = if output_dir_text.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(output_dir_text))
                };
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
                let (editor_font_family, editor_font_size) = font_btn
                    .font_desc()
                    .map(|fd| {
                        let family = fd.family()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "Monospace".to_string());
                        let pts = fd.size() / gtk4::pango::SCALE;
                        let size = if pts <= 0 { 13u32 } else { pts as u32 };
                        (family, size)
                    })
                    .unwrap_or_else(|| ("Monospace".to_string(), 13u32));
                let spell_language = available_langs
                    .get(lang_row.selected() as usize)
                    .cloned()
                    .unwrap_or_else(|| "en_US".to_string());
                let editor_line_spacing = match spacing_row.selected() {
                    0 => 0u32,
                    2 => 6u32,
                    _ => 2u32,
                };
                Config {
                    work_dir,
                    output_dir,
                    recent_files: recent_files_cur.clone(),
                    recent_projects: recent_projects_cur.clone(),
                    bib_path,
                    debounce_ms: debounce_spin.value() as u64,
                    auto_compile: auto_row.is_active(),
                    editor_font_size,
                    theme,
                    editor_font_family,
                    editor_word_wrap: wrap_row.is_active(),
                    editor_show_whitespace: ws_row.is_active(),
                    editor_tab_width: tab_spin.value() as u32,
                    preview_zoom: preview_zoom_cur,
                    spell_enabled: spell_enabled_row.is_active(),
                    spell_autocorrect: spell_autocorrect_row.is_active(),
                    spell_language,
                    editor_line_spacing,
                    typewriter_scrolling: typewriter_row.is_active(),
                    high_contrast: high_contrast_row.is_active(),
                    word_count_goal: 0,
                    sidebar_width: sidebar_width_cur,
                    preview_split: preview_split_cur,
                    developer_mode: dev_mode_row.is_active(),
                    last_export_format: 0,
                }
            }
        };
        let build_config = std::rc::Rc::new(build_config);

        // Live preview: fire on_preview whenever appearance-affecting rows change
        macro_rules! wire_preview {
            ($widget:expr, $signal:ident) => {{
                let bc = build_config.clone();
                let op = on_preview.clone();
                $widget.$signal(move |_| {
                    if let Some(f) = op.borrow().as_ref() { f(bc()); }
                });
            }};
        }
        wire_preview!(theme_row, connect_selected_notify);
        wire_preview!(font_btn, connect_font_desc_notify);
        wire_preview!(tab_spin, connect_value_notify);
        wire_preview!(spacing_row, connect_selected_notify);
        wire_preview!(wrap_row, connect_active_notify);
        wire_preview!(ws_row, connect_active_notify);
        wire_preview!(typewriter_row, connect_active_notify);
        wire_preview!(high_contrast_row, connect_active_notify);

        let on_save_cb = on_save.clone();
        let bc_save = build_config.clone();
        let win_save = window.clone();
        save_btn.connect_clicked(move |_| {
            let new_cfg = bc_save();
            if let Err(e) = new_cfg.save() {
                eprintln!("Failed to save config: {e}");
            }
            if let Some(f) = on_save_cb.borrow().as_ref() {
                f(new_cfg);
            }
            win_save.close();
        });

        Self { window, on_save, on_preview }
    }

    pub fn set_on_save(&self, f: impl Fn(Config) + 'static) {
        *self.on_save.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_preview(&self, f: impl Fn(Config) + 'static) {
        *self.on_preview.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.window.present();
    }
}

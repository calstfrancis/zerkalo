use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, ListBox, ListBoxRow, Notebook, Orientation};
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
        work_dir_btn.set_tooltip_text(Some("Browse for a folder"));
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
        output_dir_btn.set_tooltip_text(Some("Browse for a folder"));
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
        debounce_spin.set_subtitle("Milliseconds between last keystroke and recompile (Auto mode only)");
        debounce_spin.set_value(current.debounce_ms as f64);

        // 3-way pill: Auto | On Save | Manual
        let btn_auto   = gtk4::ToggleButton::with_label("Auto");
        let btn_save   = gtk4::ToggleButton::with_label("On Save");
        let btn_manual = gtk4::ToggleButton::with_label("Manual");
        btn_save.set_group(Some(&btn_auto));
        btn_manual.set_group(Some(&btn_auto));

        if current.manual_compile_only {
            btn_manual.set_active(true);
        } else if current.compile_on_save {
            btn_save.set_active(true);
        } else {
            btn_auto.set_active(true);
        }

        let pill_box = GtkBox::new(Orientation::Horizontal, 0);
        pill_box.add_css_class("linked");
        pill_box.set_valign(Align::Center);
        pill_box.append(&btn_auto);
        pill_box.append(&btn_save);
        pill_box.append(&btn_manual);

        let compile_mode_row = adw::ActionRow::new();
        compile_mode_row.set_title("Compile trigger");
        compile_mode_row.set_subtitle("Auto: after each keystroke · On Save: Ctrl+S only · Manual: Ctrl+Shift+P only");
        compile_mode_row.add_suffix(&pill_box);
        compile_mode_row.set_activatable_widget(Some(&btn_auto));

        compile_group.add(&debounce_spin);
        compile_group.add(&compile_mode_row);

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

        let word_count_goal_spin = adw::SpinRow::with_range(0.0, 1_000_000.0, 100.0);
        word_count_goal_spin.set_title("Word count goal");
        word_count_goal_spin.set_subtitle("Show progress bar in status bar (0 = disabled)");
        word_count_goal_spin.set_value(current.word_count_goal as f64);

        font_group.add(&font_row);
        font_group.add(&tab_spin);
        font_group.add(&wrap_row);
        font_group.add(&ws_row);
        font_group.add(&spacing_row);
        font_group.add(&typewriter_row);
        font_group.add(&high_contrast_row);
        font_group.add(&word_count_goal_spin);

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

        let csl_row = adw::EntryRow::new();
        csl_row.set_title("Custom CSL file");
        if let Some(ref p) = current.custom_csl_path {
            csl_row.set_text(p.to_str().unwrap_or(""));
        }

        let csl_browse_btn = Button::from_icon_name("document-open-symbolic");
        csl_browse_btn.set_valign(Align::Center);
        csl_browse_btn.add_css_class("flat");
        let csl_row_browse = csl_row.clone();
        let window_browse_csl = window.clone();
        csl_browse_btn.connect_clicked(move |_| {
            let row = csl_row_browse.clone();
            let fd = gtk4::FileDialog::new();
            let filter = gtk4::FileFilter::new();
            filter.set_name(Some("CSL files (*.csl)"));
            filter.add_pattern("*.csl");
            let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);
            fd.set_filters(Some(&filters));
            fd.open(Some(&window_browse_csl), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        row.set_text(path.to_str().unwrap_or(""));
                    }
                }
            });
        });
        csl_row.add_suffix(&csl_browse_btn);
        bib_group.add(&csl_row);

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

        // ── Language list ─────────────────────────────────────────────────────
        let selected_langs: Rc<RefCell<Vec<String>>> =
            Rc::new(RefCell::new(current.spell_languages.clone()));

        let lang_list_box = ListBox::new();
        lang_list_box.add_css_class("boxed-list");

        let rebuild_lang_list = {
            let lb = lang_list_box.clone();
            let sl = selected_langs.clone();
            move || {
                while let Some(row) = lb.row_at_index(0) {
                    lb.remove(&row);
                }
                for (i, lang) in sl.borrow().iter().enumerate() {
                    let row = ListBoxRow::new();
                    row.set_activatable(false);
                    let hbox = GtkBox::new(Orientation::Horizontal, 8);
                    hbox.set_margin_start(12);
                    hbox.set_margin_end(8);
                    hbox.set_margin_top(6);
                    hbox.set_margin_bottom(6);
                    let lbl = Label::new(Some(lang));
                    lbl.set_hexpand(true);
                    lbl.set_xalign(0.0);
                    let rm_btn = Button::from_icon_name("list-remove-symbolic");
                    rm_btn.add_css_class("flat");
                    rm_btn.set_tooltip_text(Some("Remove this language"));
                    let sl2 = sl.clone();
                    let lb2 = lb.clone();
                    rm_btn.connect_clicked(move |_| {
                        sl2.borrow_mut().remove(i);
                        // trigger rebuild by emitting a fake signal — we do it by
                        // directly modifying the list since rebuild is a closure we
                        // can't call recursively here
                        while let Some(r) = lb2.row_at_index(0) { lb2.remove(&r); }
                        for (j, l) in sl2.borrow().iter().enumerate() {
                            let r2 = ListBoxRow::new();
                            r2.set_activatable(false);
                            let h2 = GtkBox::new(Orientation::Horizontal, 8);
                            h2.set_margin_start(12);
                            h2.set_margin_end(8);
                            h2.set_margin_top(6);
                            h2.set_margin_bottom(6);
                            let l2 = Label::new(Some(l));
                            l2.set_hexpand(true);
                            l2.set_xalign(0.0);
                            let rb = Button::from_icon_name("list-remove-symbolic");
                            rb.add_css_class("flat");
                            rb.set_tooltip_text(Some("Remove this language"));
                            let sl3 = sl2.clone();
                            let lb3 = lb2.clone();
                            rb.connect_clicked(move |_| {
                                sl3.borrow_mut().remove(j);
                                while let Some(r3) = lb3.row_at_index(0) { lb3.remove(&r3); }
                            });
                            h2.append(&l2);
                            h2.append(&rb);
                            r2.set_child(Some(&h2));
                            lb2.append(&r2);
                        }
                    });
                    hbox.append(&lbl);
                    hbox.append(&rm_btn);
                    row.set_child(Some(&hbox));
                    lb.append(&row);
                }
            }
        };
        rebuild_lang_list();

        // Add-language row: dropdown + button
        let add_lang_strings: Vec<&str> = available_langs.iter().map(|s| s.as_str()).collect();
        let add_lang_model = gtk4::StringList::new(&add_lang_strings);
        let add_combo = adw::ComboRow::new();
        add_combo.set_title("Add language");
        add_combo.set_model(Some(&add_lang_model));

        let add_btn = Button::with_label("Add");
        add_btn.add_css_class("flat");
        add_btn.set_valign(Align::Center);
        {
            let sl = selected_langs.clone();
            let al = available_langs.clone();
            let lb = lang_list_box.clone();
            let combo = add_combo.clone();
            add_btn.connect_clicked(move |_| {
                let idx = combo.selected() as usize;
                if let Some(lang) = al.get(idx) {
                    if !sl.borrow().contains(lang) {
                        sl.borrow_mut().push(lang.clone());
                        // Rebuild rows
                        while let Some(r) = lb.row_at_index(0) { lb.remove(&r); }
                        for lang2 in sl.borrow().iter() {
                            let row = ListBoxRow::new();
                            row.set_activatable(false);
                            let hbox = GtkBox::new(Orientation::Horizontal, 8);
                            hbox.set_margin_start(12);
                            hbox.set_margin_end(8);
                            hbox.set_margin_top(6);
                            hbox.set_margin_bottom(6);
                            let lbl = Label::new(Some(lang2));
                            lbl.set_hexpand(true);
                            lbl.set_xalign(0.0);
                            hbox.append(&lbl);
                            row.set_child(Some(&hbox));
                            lb.append(&row);
                        }
                    }
                }
            });
        }

        spell_group.add(&spell_enabled_row);
        spell_group.add(&spell_autocorrect_row);
        spell_group.add(&lang_list_box);
        spell_group.add(&add_combo);

        // ── Tabs ─────────────────────────────────────────────────────────────

        let notebook = Notebook::new();
        notebook.set_tab_pos(gtk4::PositionType::Top);
        notebook.set_vexpand(true);

        // Developer mode
        let dev_group = adw::PreferencesGroup::new();
        dev_group.set_title("Advanced");
        let dev_mode_row = adw::SwitchRow::new();
        dev_mode_row.set_title("Experimental mode");
        dev_mode_row.set_subtitle("Show experimental features (Import…)");
        dev_mode_row.set_active(current.developer_mode);
        dev_group.add(&dev_mode_row);

        let batch_concurrency_row = adw::SpinRow::with_range(1.0, 5.0, 1.0);
        batch_concurrency_row.set_title("Simultaneous imports");
        batch_concurrency_row.set_subtitle("How many documents Import Folder converts at once");
        batch_concurrency_row.set_value(current.batch_import_concurrency as f64);
        dev_group.add(&batch_concurrency_row);

        let sync_group = adw::PreferencesGroup::new();
        sync_group.set_title("GitHub Sync");
        sync_group.set_description(Some("Personal Access Token for pushing to GitHub. Generate one at github.com → Settings → Developer settings → Personal access tokens → Fine-grained."));
        let token_row = adw::EntryRow::new();
        token_row.set_title("Personal Access Token");
        token_row.set_show_apply_button(false);
        if let Some(tok) = &current.github_token {
            token_row.set_text(tok);
        }
        sync_group.add(&token_row);

        let page_general = adw::PreferencesPage::new();
        page_general.add(&folders_group);
        page_general.add(&compile_group);
        page_general.add(&sync_group);
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
            let tab_spin = tab_spin.clone();
            let wrap_row = wrap_row.clone();
            let ws_row = ws_row.clone();
            let spacing_row = spacing_row.clone();
            let typewriter_row = typewriter_row.clone();
            let high_contrast_row = high_contrast_row.clone();
            let word_count_goal_spin = word_count_goal_spin.clone();
            let spell_enabled_row = spell_enabled_row.clone();
            let spell_autocorrect_row = spell_autocorrect_row.clone();
            let selected_langs = selected_langs.clone();
            let dev_mode_row = dev_mode_row.clone();
            let batch_concurrency_row = batch_concurrency_row.clone();
            let token_row = token_row.clone();
            let recent_files_cur = current.recent_files.clone();
            let recent_projects_cur = current.recent_projects.clone();
            let recent_searches_cur = current.recent_searches.clone();
            let preview_zoom_cur = current.preview_zoom;
            let sidebar_width_cur = current.sidebar_width;
            let preview_split_cur = current.preview_split;
            let _word_count_goal_cur = current.word_count_goal; // replaced by spin row
            let last_export_format_cur = current.last_export_format;
            let auto_save_idle_ms_cur = current.auto_save_idle_ms;
            let active_profile_cur = current.active_profile.clone();
            let locked_author_cur = current.locked_author.clone();
            let locked_affiliation_cur = current.locked_affiliation.clone();
            let simple_mode_cur = current.simple_mode;
            let shown_simple_intro_cur = current.shown_simple_intro;
            let format_bar_visible_cur = current.format_bar_visible;
            let last_used_advanced_cur = current.last_used_advanced;
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
                let custom_csl_path_text = csl_row.text().trim().to_string();
                let custom_csl_path: Option<PathBuf> = if custom_csl_path_text.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(custom_csl_path_text))
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
                let spell_languages = {
                    let langs = selected_langs.borrow().clone();
                    if langs.is_empty() { vec!["en_US".to_string()] } else { langs }
                };
                let editor_line_spacing = match spacing_row.selected() {
                    0 => 0u32,
                    2 => 6u32,
                    _ => 2u32,
                };
                let token_text = token_row.text().trim().to_string();
                let github_token = if token_text.is_empty() { None } else { Some(token_text) };
                Config {
                    work_dir,
                    output_dir,
                    recent_files: recent_files_cur.clone(),
                    recent_projects: recent_projects_cur.clone(),
                    bib_path,
                    custom_csl_path,
                    debounce_ms: debounce_spin.value() as u64,
                    auto_compile: btn_auto.is_active(),
                    compile_on_save: btn_save.is_active(),
                    manual_compile_only: btn_manual.is_active(),
                    editor_font_size,
                    theme,
                    editor_font_family,
                    editor_word_wrap: wrap_row.is_active(),
                    editor_show_whitespace: ws_row.is_active(),
                    editor_tab_width: tab_spin.value() as u32,
                    preview_zoom: preview_zoom_cur,
                    spell_enabled: spell_enabled_row.is_active(),
                    spell_autocorrect: spell_autocorrect_row.is_active(),
                    spell_languages,
                    editor_line_spacing,
                    typewriter_scrolling: typewriter_row.is_active(),
                    high_contrast: high_contrast_row.is_active(),
                    word_count_goal: word_count_goal_spin.value() as u32,
                    sidebar_width: sidebar_width_cur,
                    preview_split: preview_split_cur,
                    developer_mode: dev_mode_row.is_active(),
                    batch_import_concurrency: batch_concurrency_row.value() as u32,
                    last_export_format: last_export_format_cur,
                    recent_searches: recent_searches_cur.clone(),
                    active_profile: active_profile_cur.clone(),
                    auto_save_idle_ms: auto_save_idle_ms_cur,
                    github_token,
                    locked_author: locked_author_cur.clone(),
                    locked_affiliation: locked_affiliation_cur.clone(),
                    simple_mode: simple_mode_cur,
                    shown_simple_intro: shown_simple_intro_cur,
                    format_bar_visible: format_bar_visible_cur,
                    last_used_advanced: last_used_advanced_cur,
                    snippets: Vec::new(),
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
            win_save.close();
            if let Some(f) = on_save_cb.borrow().as_ref() {
                f(new_cfg);
            }
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

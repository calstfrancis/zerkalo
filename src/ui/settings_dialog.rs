use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, ListBox, ListBoxRow, Orientation};
use libadwaita as adw;
use adw::prelude::*;

use crate::config::{Config, Theme};

/// Shows a modal notice on top of the settings window. Everything in this
/// dialog reports through here so the app doesn't mix `gtk4::AlertDialog` and
/// `adw::MessageDialog` for the same kind of message.
fn notice(parent: &adw::Window, heading: &str, body: &str) {
    super::confirm::notice(Some(parent.upcast_ref()), heading, body);
}

/// Clears and rebuilds `lb`'s rows from `selected_langs`, wiring each row's
/// remove button to mutate the shared list and rebuild again — a plain
/// function (rather than a closure) so it can call itself for every row
/// without the self-referential-closure problem that caused rows added after
/// the first removal to end up without working remove buttons.
fn rebuild_lang_rows(lb: &ListBox, selected_langs: &Rc<RefCell<Vec<String>>>) {
    while let Some(row) = lb.row_at_index(0) {
        lb.remove(&row);
    }
    for (i, lang) in selected_langs.borrow().iter().enumerate() {
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
        rm_btn.update_property(&[gtk4::accessible::Property::Label("Remove this language")]);
        let sl = selected_langs.clone();
        let lb2 = lb.clone();
        rm_btn.connect_clicked(move |_| {
            sl.borrow_mut().remove(i);
            rebuild_lang_rows(&lb2, &sl);
        });
        hbox.append(&lbl);
        hbox.append(&rm_btn);
        row.set_child(Some(&hbox));
        lb.append(&row);
    }
}

pub struct SettingsDialog {
    window: adw::Window,
    on_save: Rc<RefCell<Option<Box<dyn Fn(Config)>>>>,
    on_preview: Rc<RefCell<Option<Box<dyn Fn(Config)>>>>,
    on_open_font_manager: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_open_setup_wizard: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_open_backup_locations: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl SettingsDialog {
    pub fn new(parent: &impl IsA<gtk4::Window>, current: &Config) -> Self {
        let window = adw::Window::builder()
            .title("Settings")
            .transient_for(parent)
            .modal(true)
            .default_width(560)
            .default_height(700)
            .width_request(420)
            .height_request(400)
            .resizable(true)
            .build();

        let on_save: Rc<RefCell<Option<Box<dyn Fn(Config)>>>> = Rc::new(RefCell::new(None));
        let on_preview: Rc<RefCell<Option<Box<dyn Fn(Config)>>>> = Rc::new(RefCell::new(None));
        let on_open_font_manager: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_open_setup_wizard: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_open_backup_locations: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        // Set by Save so the close-request revert below leaves the newly saved
        // appearance alone.
        let saved_flag = Rc::new(std::cell::Cell::new(false));

        // ── Header bar ──────────────────────────────────────────────────────

        let header = adw::HeaderBar::new();
        header.add_css_class("fond-chrome");
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
        work_dir_btn.update_property(&[gtk4::accessible::Property::Label("Browse for a work folder")]);
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
        output_dir_btn.update_property(&[gtk4::accessible::Property::Label("Browse for an output folder")]);
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
        debounce_spin.set_title("Compile delay");
        debounce_spin.set_subtitle("How long to wait after you stop typing before updating the preview, in milliseconds (Auto mode only)");
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

        // Document fonts. These used to be a step in setup, which put a font
        // choice between a first-time user and getting started — for a setting
        // whose defaults are already the right answer nearly always.
        let doc_font_group = adw::PreferencesGroup::new();
        doc_font_group.set_title("Document Fonts");
        doc_font_group.set_description(Some(
            "Used by new documents and template previews until a document picks its own.",
        ));

        let doc_fonts = super::font_manager::FontManager::enabled_fonts();
        let doc_font_labels: Vec<&str> = doc_fonts.iter().map(|s| s.as_str()).collect();
        let doc_font_model = gtk4::StringList::new(&doc_font_labels);
        let preview_factory = font_preview_factory();

        let sans_row = adw::ComboRow::new();
        sans_row.set_title("Sans-serif");
        sans_row.set_model(Some(&doc_font_model));
        sans_row.set_factory(Some(&preview_factory));
        sans_row.set_list_factory(Some(&preview_factory));
        sans_row.set_selected(best_font_index(
            &doc_fonts, &current.default_sans_font, SANS_FONT_PRIORITY,
        ));

        let serif_row = adw::ComboRow::new();
        serif_row.set_title("Serif");
        serif_row.set_model(Some(&doc_font_model));
        serif_row.set_factory(Some(&preview_factory));
        serif_row.set_list_factory(Some(&preview_factory));
        serif_row.set_selected(best_font_index(
            &doc_fonts, &current.default_serif_font, SERIF_FONT_PRIORITY,
        ));

        // Which fonts exist to choose from, not just which is the default —
        // this used to be its own hamburger row ("Document Fonts…"), reading
        // as a third font-related surface competing with the two rows above.
        let manage_fonts_row = adw::ActionRow::new();
        manage_fonts_row.set_title("Available fonts");
        manage_fonts_row.set_subtitle("Enable or disable fonts Zerkalo can use");
        let manage_fonts_btn = Button::with_label("Manage…");
        manage_fonts_btn.set_valign(Align::Center);
        manage_fonts_row.add_suffix(&manage_fonts_btn);
        manage_fonts_row.set_activatable_widget(Some(&manage_fonts_btn));
        {
            let cb = on_open_font_manager.clone();
            manage_fonts_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }

        doc_font_group.add(&sans_row);
        doc_font_group.add(&serif_row);
        doc_font_group.add(&manage_fonts_row);

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
        bib_row.set_title("Bib file or Kartoteka vault");
        if let Some(ref p) = current.bib_path {
            bib_row.set_text(p.to_str().unwrap_or(""));
        }

        let browse_btn = Button::from_icon_name("document-open-symbolic");
        browse_btn.set_valign(Align::Center);
        browse_btn.add_css_class("flat");
        browse_btn.set_tooltip_text(Some("Browse for a .bib/.yaml file"));
        browse_btn.update_property(&[gtk4::accessible::Property::Label("Browse for a bibliography file")]);
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

        let vault_browse_btn = Button::from_icon_name("folder-symbolic");
        vault_browse_btn.set_valign(Align::Center);
        vault_browse_btn.add_css_class("flat");
        vault_browse_btn.set_tooltip_text(Some("Browse for a Kartoteka vault folder"));
        vault_browse_btn.update_property(&[gtk4::accessible::Property::Label("Browse for a Kartoteka vault folder")]);
        let bib_row_vault = bib_row.clone();
        let window_vault = window.clone();
        vault_browse_btn.connect_clicked(move |_| {
            let row = bib_row_vault.clone();
            let fd = gtk4::FileDialog::new();
            fd.select_folder(Some(&window_vault), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        row.set_text(path.to_str().unwrap_or(""));
                    }
                }
            });
        });
        bib_row.add_suffix(&vault_browse_btn);
        bib_group.add(&bib_row);
        bib_group.set_description(Some(
            "A .bib/.yaml file — including a library exported from Zotero, Mendeley, or any other reference manager as BibTeX — or a Kartoteka vault folder for live citation autocomplete as you edit the vault.",
        ));

        let csl_row = adw::EntryRow::new();
        csl_row.set_title("Custom CSL file");
        if let Some(ref p) = current.custom_csl_path {
            csl_row.set_text(p.to_str().unwrap_or(""));
        }

        let csl_browse_btn = Button::from_icon_name("document-open-symbolic");
        csl_browse_btn.set_valign(Align::Center);
        csl_browse_btn.add_css_class("flat");
        csl_browse_btn.set_tooltip_text(Some("Browse for a .csl file"));
        csl_browse_btn.update_property(&[gtk4::accessible::Property::Label("Browse for a CSL style file")]);
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

        // CV Elements (Skrizhal)
        let cv_group = adw::PreferencesGroup::new();
        cv_group.set_title("CV Elements");
        cv_group.set_description(Some(
            "Used in CV mode instead of the bibliography above — a Skrizhal YAML file of jobs, degrees, awards, etc.",
        ));

        let cv_row = adw::EntryRow::new();
        cv_row.set_title("Skrizhal file");
        if let Some(ref p) = current.cv_elements_path {
            cv_row.set_text(p.to_str().unwrap_or(""));
        }

        let cv_browse_btn = Button::from_icon_name("document-open-symbolic");
        cv_browse_btn.set_valign(Align::Center);
        cv_browse_btn.add_css_class("flat");
        cv_browse_btn.set_tooltip_text(Some("Browse for a Skrizhal file"));
        cv_browse_btn.update_property(&[gtk4::accessible::Property::Label("Browse for a Skrizhal file")]);
        let cv_row_browse = cv_row.clone();
        let window_browse_cv = window.clone();
        cv_browse_btn.connect_clicked(move |_| {
            let row = cv_row_browse.clone();
            let fd = gtk4::FileDialog::new();
            let filter = gtk4::FileFilter::new();
            filter.set_name(Some("YAML files (*.yaml, *.yml)"));
            filter.add_pattern("*.yaml");
            filter.add_pattern("*.yml");
            let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            filters.append(&filter);
            fd.set_filters(Some(&filters));
            fd.open(Some(&window_browse_cv), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        row.set_text(path.to_str().unwrap_or(""));
                    }
                }
            });
        });
        cv_row.add_suffix(&cv_browse_btn);
        cv_group.add(&cv_row);

        // Spell check
        let spell_group = adw::PreferencesGroup::new();
        spell_group.set_title("Spell Check");

        let spell_enabled_row = adw::SwitchRow::new();
        spell_enabled_row.set_title("Enable spell check");
        spell_enabled_row.set_active(current.spell_enabled);

        let available_langs = crate::spellcheck::SpellChecker::available_languages();

        // ── Language list ─────────────────────────────────────────────────────
        let selected_langs: Rc<RefCell<Vec<String>>> =
            Rc::new(RefCell::new(current.spell_languages.clone()));

        let lang_list_box = ListBox::new();
        lang_list_box.add_css_class("boxed-list");

        rebuild_lang_rows(&lang_list_box, &selected_langs);

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
                        rebuild_lang_rows(&lb, &sl);
                    }
                }
            });
        }

        spell_group.add(&spell_enabled_row);
        spell_group.add(&lang_list_box);
        spell_group.add(&add_combo);

        // ── Pages ────────────────────────────────────────────────────────────
        // An AdwViewStack driven by a header AdwViewSwitcher rather than a raw
        // GtkNotebook: same three pages, but with the platform look and an
        // adaptive switcher. (AdwPreferencesDialog would be the full answer,
        // but it needs libadwaita 1.5 and this build pins v1_4.)

        let view_stack = adw::ViewStack::new();
        view_stack.set_vexpand(true);

        // Advanced
        let dev_group = adw::PreferencesGroup::new();
        dev_group.set_title("Advanced");

        let batch_concurrency_row = adw::SpinRow::with_range(1.0, 5.0, 1.0);
        batch_concurrency_row.set_title("Simultaneous imports");
        batch_concurrency_row.set_subtitle("How many documents Import Folder converts at once");
        batch_concurrency_row.set_value(current.batch_import_concurrency as f64);
        dev_group.add(&batch_concurrency_row);

        // ── Keyboard shortcuts ───────────────────────────────────────────────
        // Bindings live in keybindings.toml with no editor UI; without this row
        // there was nothing in the app saying the file exists.
        let keys_group = adw::PreferencesGroup::new();
        keys_group.set_title("Keyboard Shortcuts");
        let keys_row = adw::ActionRow::new();
        keys_row.set_title("Shortcut bindings");
        keys_row.set_subtitle("Customize any shortcut by editing a text file");
        keys_row.set_tooltip_text(Some(
            &crate::keybindings::keybindings_path().to_string_lossy(),
        ));
        let keys_btn = Button::with_label("Open File");
        keys_btn.set_valign(Align::Center);
        {
            let win_keys = window.clone();
            keys_btn.connect_clicked(move |_| {
                let path = crate::keybindings::keybindings_path();
                crate::keybindings::Keybindings::write_default_if_missing();
                let launched = gtk4::gio::AppInfo::launch_default_for_uri(
                    &format!("file://{}", path.display()),
                    None::<&gtk4::gio::AppLaunchContext>,
                )
                .is_ok();
                if !launched {
                    notice(
                        &win_keys,
                        "Couldn't open the file",
                        &format!("Edit it by hand at:\n{}", path.display()),
                    );
                }
            });
        }
        keys_row.add_suffix(&keys_btn);
        keys_group.add(&keys_row);

        let sync_group = adw::PreferencesGroup::new();
        sync_group.set_title("Backup & Sync");
        sync_group.set_description(Some("Sign in with GitHub to back up your work online when you sync."));

        let account_row = adw::ActionRow::new();
        account_row.set_title("Account");
        let has_token = crate::secret_store::load_github_token().is_some();
        account_row.set_subtitle(if has_token { "Connected" } else { "Not connected" });

        let account_btn_box = GtkBox::new(Orientation::Horizontal, 6);
        account_btn_box.set_valign(Align::Center);

        let signin_btn = Button::with_label(if has_token { "Reconnect" } else { "Sign in with GitHub" });
        signin_btn.add_css_class("suggested-action");
        {
            let parent_win = window.clone();
            let row_c = account_row.clone();
            signin_btn.connect_clicked(move |_| {
                let row_c2 = row_c.clone();
                super::github_signin::present(&parent_win, move |username| {
                    row_c2.set_subtitle(&format!("Connected as {username}"));
                });
            });
        }
        account_btn_box.append(&signin_btn);

        let disconnect_btn = Button::with_label("Disconnect");
        disconnect_btn.add_css_class("destructive-action");
        disconnect_btn.set_visible(has_token);
        {
            let row_c = account_row.clone();
            let signin_c = signin_btn.clone();
            disconnect_btn.connect_clicked(move |btn| {
                crate::secret_store::delete_github_token();
                row_c.set_subtitle("Not connected");
                signin_c.set_label("Sign in with GitHub");
                btn.set_visible(false);
            });
        }
        account_btn_box.append(&disconnect_btn);

        account_row.add_suffix(&account_btn_box);
        sync_group.add(&account_row);

        let backup_locations_row = adw::ActionRow::new();
        backup_locations_row.set_title("Backup locations");
        backup_locations_row.set_subtitle("Where saved versions get sent when you sync");
        let backup_locations_btn = Button::with_label("Manage…");
        backup_locations_btn.set_valign(Align::Center);
        backup_locations_row.add_suffix(&backup_locations_btn);
        backup_locations_row.set_activatable_widget(Some(&backup_locations_btn));
        {
            let cb = on_open_backup_locations.clone();
            backup_locations_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }
        sync_group.add(&backup_locations_row);

        // Tools status (git/tinymist/pandoc bundled, everything else optional)
        // used to be its own hamburger row — a diagnostic you check when
        // something looks wrong belongs beside the rest of setup, not
        // floating as a top-level menu action.
        let (tools_group, _tools_ok, tools_rechecks) = super::tools_window::tools_group();
        // Re-check whenever the window regains focus, matching the
        // standalone Tools window this group used to live in — installing
        // something in a terminal and coming back updates the list without
        // a click.
        window.connect_is_active_notify(move |w| {
            if w.is_active() {
                for f in &tools_rechecks {
                    f();
                }
            }
        });

        // Re-running setup used to have no way back in once the first-run
        // wizard closed.
        let setup_group = adw::PreferencesGroup::new();
        setup_group.set_title("Setup");
        let setup_row = adw::ActionRow::new();
        setup_row.set_title("Setup wizard");
        setup_row.set_subtitle("Re-run the guided first-time setup");
        let setup_btn = Button::with_label("Run…");
        setup_btn.set_valign(Align::Center);
        setup_row.add_suffix(&setup_btn);
        setup_row.set_activatable_widget(Some(&setup_btn));
        {
            let cb = on_open_setup_wizard.clone();
            setup_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }
        setup_group.add(&setup_row);

        let page_general = adw::PreferencesPage::new();
        page_general.add(&folders_group);
        page_general.add(&compile_group);
        page_general.add(&sync_group);
        page_general.add(&setup_group);
        page_general.add(&tools_group);
        page_general.add(&keys_group);
        page_general.add(&dev_group);
        let sp_general = view_stack.add_titled(&page_general, Some("general"), "General");
        sp_general.set_icon_name(Some("preferences-system-symbolic"));

        let page_editor = adw::PreferencesPage::new();
        page_editor.add(&editor_group);
        page_editor.add(&font_group);
        page_editor.add(&doc_font_group);
        let sp_editor = view_stack.add_titled(&page_editor, Some("editor"), "Editor");
        sp_editor.set_icon_name(Some("text-editor-symbolic"));

        let page_extras = adw::PreferencesPage::new();
        page_extras.add(&bib_group);
        page_extras.add(&cv_group);
        page_extras.add(&spell_group);
        let sp_extras = view_stack.add_titled(&page_extras, Some("extras"), "References & Spelling");
        sp_extras.set_icon_name(Some("accessories-dictionary-symbolic"));

        let switcher = adw::ViewSwitcher::new();
        switcher.set_stack(Some(&view_stack));
        switcher.set_policy(adw::ViewSwitcherPolicy::Wide);
        header.set_title_widget(Some(&switcher));

        // ── Toolbar view ─────────────────────────────────────────────────────

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
        toolbar_view.add_top_bar(&header);
        // .fond-ground: the recessed plane the suite's group/card rows sit on
        // — previously only the header carried a Fond surface class, so the
        // page content read as a plain, disconnected white box.
        view_stack.add_css_class("fond-ground");
        toolbar_view.set_content(Some(&view_stack));
        window.set_content(Some(&toolbar_view));

        // ── Wiring ──────────────────────────────────────────────────────────

        // Every abandon route reverts the live appearance preview, not just the
        // Cancel button: Escape, Alt+F4 and a window-manager close all used to
        // leave the previewed theme/font applied over an unchanged config.
        let revert_preview: Rc<dyn Fn()> = {
            let on_preview_revert = on_preview.clone();
            let revert_cfg = current.clone();
            let saved = saved_flag.clone();
            Rc::new(move || {
                if saved.get() {
                    return;
                }
                if let Some(f) = on_preview_revert.borrow().as_ref() {
                    f(revert_cfg.clone());
                }
            })
        };

        {
            let revert = revert_preview.clone();
            window.connect_close_request(move |_| {
                revert();
                glib::Propagation::Proceed
            });
        }

        {
            let win_esc = window.clone();
            let esc = gtk4::EventControllerKey::new();
            esc.connect_key_pressed(move |_, key, _, _| {
                if key == gtk4::gdk::Key::Escape {
                    win_esc.close();
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            window.add_controller(esc);
        }

        let win_cancel = window.clone();
        cancel_btn.connect_clicked(move |_| {
            // close_request does the revert, so Cancel is just a close.
            win_cancel.close();
        });

        // Helper that reads the current dialog state into a Config
        let build_config = {
            let work_dir_row = work_dir_row.clone();
            let output_dir_row = output_dir_row.clone();
            let bib_row = bib_row.clone();
            let cv_row = cv_row.clone();
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
            let selected_langs = selected_langs.clone();
            let batch_concurrency_row = batch_concurrency_row.clone();
            let recent_files_cur = current.recent_files.clone();
            let recent_projects_cur = current.recent_projects.clone();
            let recent_searches_cur = current.recent_searches.clone();
            let preview_zoom_cur = current.preview_zoom;
            let sidebar_width_cur = current.sidebar_width;
            let preview_split_cur = current.preview_split;
            // Owned by the hamburger's own toggle, not this dialog — carried
            // through so saving preferences doesn't switch the UI font back.
            let gost_font_cur = current.gost_font;
            // Same story: the hamburger's Autocorrect toggle is the one
            // control for this setting (matches the GOST-font pattern above)
            // — Settings used to have a second, independently-wired switch
            // for the same config field, which read back correctly but was a
            // duplicate control for one setting.
            let spell_autocorrect_cur = current.spell_autocorrect;
            let last_export_format_cur = current.last_export_format;
            let auto_save_idle_ms_cur = current.auto_save_idle_ms;
            let active_profile_cur = current.active_profile.clone();
            let locked_author_cur = current.locked_author.clone();
            let locked_affiliation_cur = current.locked_affiliation.clone();
            let simple_mode_cur = current.simple_mode;
            let shown_simple_intro_cur = current.shown_simple_intro;
            let shown_editor_orientation_cur = current.shown_editor_orientation;
            let format_bar_visible_cur = current.format_bar_visible;
            let last_used_advanced_cur = current.last_used_advanced;
            let default_sans_font_cur = current.default_sans_font.clone();
            let default_serif_font_cur = current.default_serif_font.clone();
            let doc_fonts_cur = doc_fonts.clone();
            let setup_done_cur = current.setup_done;
            // Owned by the print sheet, not this dialog — carried through so
            // saving preferences doesn't reset the last-used print settings.
            let print_cur = current.print.clone();
            let snippets_cur = current.snippets.clone();
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
                let cv_elements_path_text = cv_row.text().trim().to_string();
                let cv_elements_path: Option<PathBuf> = if cv_elements_path_text.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(cv_elements_path_text))
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
                Config {
                    work_dir,
                    output_dir,
                    recent_files: recent_files_cur.clone(),
                    recent_projects: recent_projects_cur.clone(),
                    bib_path,
                    cv_elements_path,
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
                    spell_autocorrect: spell_autocorrect_cur,
                    spell_languages,
                    editor_line_spacing,
                    typewriter_scrolling: typewriter_row.is_active(),
                    high_contrast: high_contrast_row.is_active(),
                    gost_font: gost_font_cur,
                    word_count_goal: word_count_goal_spin.value() as u32,
                    sidebar_width: sidebar_width_cur,
                    preview_split: preview_split_cur,
                    batch_import_concurrency: batch_concurrency_row.value() as u32,
                    last_export_format: last_export_format_cur,
                    recent_searches: recent_searches_cur.clone(),
                    active_profile: active_profile_cur.clone(),
                    auto_save_idle_ms: auto_save_idle_ms_cur,
                    github_token: None,
                    locked_author: locked_author_cur.clone(),
                    locked_affiliation: locked_affiliation_cur.clone(),
                    simple_mode: simple_mode_cur,
                    shown_simple_intro: shown_simple_intro_cur,
                    shown_editor_orientation: shown_editor_orientation_cur,
                    format_bar_visible: format_bar_visible_cur,
                    last_used_advanced: last_used_advanced_cur,
                    snippets: snippets_cur.clone(),
                    setup_done: setup_done_cur,
                    default_sans_font: doc_fonts_cur
                        .get(sans_row.selected() as usize)
                        .cloned()
                        .unwrap_or_else(|| default_sans_font_cur.clone()),
                    default_serif_font: doc_fonts_cur
                        .get(serif_row.selected() as usize)
                        .cloned()
                        .unwrap_or_else(|| default_serif_font_cur.clone()),
                    print: print_cur.clone(),
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
        wire_preview!(word_count_goal_spin, connect_value_notify);

        let on_save_cb = on_save.clone();
        let bc_save = build_config.clone();
        let win_save = window.clone();
        let saved_on_save = saved_flag.clone();
        save_btn.connect_clicked(move |_| {
            let new_cfg = bc_save();

            // Paths were saved unchecked, so a typo in a folder or file row
            // only showed up later as a compile that quietly went nowhere.
            if !new_cfg.work_dir.is_dir() {
                match std::fs::create_dir_all(&new_cfg.work_dir) {
                    Ok(()) => {}
                    Err(e) => {
                        notice(
                            &win_save,
                            "Work folder isn't usable",
                            &format!("{} could not be created: {e}", new_cfg.work_dir.display()),
                        );
                        return;
                    }
                }
            }
            if let Some(dir) = new_cfg.output_dir.as_ref() {
                if !dir.is_dir() {
                    if let Err(e) = std::fs::create_dir_all(dir) {
                        notice(
                            &win_save,
                            "Output folder isn't usable",
                            &format!("{} could not be created: {e}", dir.display()),
                        );
                        return;
                    }
                }
            }
            for (label, path) in [
                ("Bib file", new_cfg.bib_path.as_ref()),
                ("Custom CSL file", new_cfg.custom_csl_path.as_ref()),
                ("Skrizhal file", new_cfg.cv_elements_path.as_ref()),
            ] {
                if let Some(p) = path {
                    if !p.is_file() {
                        notice(
                            &win_save,
                            &format!("{label} not found"),
                            &format!("{} doesn't exist. Clear the field or pick another file.", p.display()),
                        );
                        return;
                    }
                }
            }

            if let Err(e) = new_cfg.save() {
                notice(&win_save, "Failed to save settings", &format!("{e}"));
                return;
            }
            saved_on_save.set(true);
            win_save.close();
            if let Some(f) = on_save_cb.borrow().as_ref() {
                f(new_cfg);
            }
        });

        Self {
            window,
            on_save,
            on_preview,
            on_open_font_manager,
            on_open_setup_wizard,
            on_open_backup_locations,
        }
    }

    pub fn set_on_save(&self, f: impl Fn(Config) + 'static) {
        *self.on_save.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_preview(&self, f: impl Fn(Config) + 'static) {
        *self.on_preview.borrow_mut() = Some(Box::new(f));
    }

    /// Fired by the Document Fonts group's "Manage available fonts…" button —
    /// the dialog itself doesn't know how to construct a `FontManager` (that
    /// needs the `adw::ApplicationWindow`, which this dialog isn't), so the
    /// caller supplies what "open it" means, same pattern as `on_save`.
    pub fn set_on_open_font_manager(&self, f: impl Fn() + 'static) {
        *self.on_open_font_manager.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_open_setup_wizard(&self, f: impl Fn() + 'static) {
        *self.on_open_setup_wizard.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_open_backup_locations(&self, f: impl Fn() + 'static) {
        *self.on_open_backup_locations.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.window.present();
    }
}

// Preference order for the document-font dropdowns' initial selection when
// nothing has been chosen yet — common, broadly-available names first, so a
// first-time user doesn't land on whatever happens to sort alphabetically
// first in their system font list (often an obscure font).
const SANS_FONT_PRIORITY: &[&str] = &["Noto Sans", "DejaVu Sans", "Cantarell", "Liberation Sans", "Arial", "Inter"];
const SERIF_FONT_PRIORITY: &[&str] = &["Noto Serif", "Liberation Serif", "DejaVu Serif", "Linux Libertine", "Times New Roman", "Georgia"];

/// Picks the best initial ComboRow selection: the user's already-chosen font
/// if it's in the list, else the first name from `priority` that's actually
/// available, else index 0 as a last resort.
fn best_font_index(fonts: &[String], current: &str, priority: &[&str]) -> u32 {
    if let Some(i) = fonts.iter().position(|f| f == current) {
        return i as u32;
    }
    for name in priority {
        if let Some(i) = fonts.iter().position(|f| f == name) {
            return i as u32;
        }
    }
    0
}

/// A list-item factory that renders each font name set in its own font, so
/// the Sans/Serif dropdowns preview the choice instead of listing plain text.
fn font_preview_factory() -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(move |_, obj| {
        let Some(item) = obj.downcast_ref::<gtk4::ListItem>() else { return };
        let label = Label::new(None);
        label.set_xalign(0.0);
        label.set_margin_start(6);
        label.set_margin_end(6);
        label.set_margin_top(4);
        label.set_margin_bottom(4);
        item.set_child(Some(&label));
    });
    factory.connect_bind(move |_, obj| {
        let Some(item) = obj.downcast_ref::<gtk4::ListItem>() else { return };
        let Some(label) = item.child().and_then(|w| w.downcast::<Label>().ok()) else { return };
        let Some(text) = item.item().and_then(|o| o.downcast::<gtk4::StringObject>().ok()) else { return };
        let name = text.string().to_string();
        label.set_text(&name);
        let mut desc = gtk4::pango::FontDescription::new();
        desc.set_family(&name);
        let attrs = gtk4::pango::AttrList::new();
        attrs.insert(gtk4::pango::AttrFontDesc::new(&desc));
        label.set_attributes(Some(&attrs));
    });
    factory
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_already_chosen_font_is_preselected() {
        let fonts = vec!["Cantarell".to_string(), "Noto Sans".to_string()];
        assert_eq!(best_font_index(&fonts, "Noto Sans", SANS_FONT_PRIORITY), 1);
    }

    #[test]
    fn with_nothing_chosen_a_known_good_font_wins_over_alphabetical_order() {
        // "Abyssinica" would otherwise be the default purely by sorting first.
        let fonts = vec!["Abyssinica SIL".to_string(), "Noto Sans".to_string()];
        assert_eq!(best_font_index(&fonts, "", SANS_FONT_PRIORITY), 1);
    }

    #[test]
    fn a_font_list_with_none_of_the_preferred_names_still_selects_something() {
        let fonts = vec!["Abyssinica SIL".to_string()];
        assert_eq!(best_font_index(&fonts, "", SANS_FONT_PRIORITY), 0);
    }
}

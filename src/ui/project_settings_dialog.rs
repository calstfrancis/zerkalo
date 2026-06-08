use std::path::PathBuf;
use std::rc::Rc;
use std::cell::RefCell;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation, StringList};
use libadwaita as adw;
use adw::prelude::*;

use crate::config::ProjectConfig;
use crate::project::collect_typ_files;

pub struct ProjectSettingsDialog {
    window: adw::Window,
}

impl ProjectSettingsDialog {
    /// Opens the per-project settings sheet. `on_saved` is called with the new
    /// config after the user confirms. The caller must apply the new config
    /// (update preview root, bibliography, etc.).
    pub fn new(
        parent: &impl IsA<gtk4::Window>,
        project_root: PathBuf,
        on_saved: impl Fn(ProjectConfig) + 'static,
    ) -> Self {
        let current = ProjectConfig::load(&project_root).unwrap_or_default();

        let window = adw::Window::builder()
            .title("Project Settings")
            .transient_for(parent)
            .modal(true)
            .default_width(460)
            .resizable(false)
            .build();

        let header = adw::HeaderBar::new();

        // ── Root file ──────────────────────────────────────────────────────
        let typ_files = collect_typ_files(&project_root);
        let rel_names: Vec<String> = typ_files.iter()
            .filter_map(|p| p.strip_prefix(&project_root).ok())
            .map(|r| r.to_string_lossy().to_string())
            .collect();

        let root_list = StringList::new(
            &rel_names.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        );
        let root_row = adw::ComboRow::new();
        root_row.set_title("Compilation Root");
        root_row.set_subtitle("The file Typst compiles from");
        root_row.set_model(Some(&root_list));

        // Pre-select current root
        if let Some(ref root) = current.root_file {
            let root_str = root.to_string_lossy();
            if let Some(idx) = rel_names.iter().position(|n| n == root_str.as_ref()) {
                root_row.set_selected(idx as u32);
            }
        }

        // ── Bibliography path ──────────────────────────────────────────────
        let bib_row = adw::EntryRow::new();
        bib_row.set_title("Bibliography file");
        bib_row.set_show_apply_button(true);
        if let Some(ref bp) = current.bib_path {
            bib_row.set_text(&bp.to_string_lossy());
        }

        // ── Group ─────────────────────────────────────────────────────────
        let group = adw::PreferencesGroup::new();
        group.set_title("Compilation");
        group.set_margin_start(16);
        group.set_margin_end(16);
        group.set_margin_top(16);
        group.add(&root_row);
        group.add(&bib_row);

        // ── Config path note ──────────────────────────────────────────────
        let note = gtk4::Label::new(Some(&format!(
            "Saved to {}/.zerkalo/config.toml",
            project_root.display()
        )));
        note.set_wrap(true);
        note.set_xalign(0.0);
        note.add_css_class("dim-label");
        note.add_css_class("caption");
        note.set_margin_start(16);
        note.set_margin_end(16);
        note.set_margin_top(6);
        note.set_margin_bottom(2);

        // ── Buttons ───────────────────────────────────────────────────────
        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.set_hexpand(true);

        let save_btn = Button::with_label("Save");
        save_btn.add_css_class("suggested-action");
        save_btn.set_hexpand(true);

        let btn_row = GtkBox::new(Orientation::Horizontal, 8);
        btn_row.set_margin_start(16);
        btn_row.set_margin_end(16);
        btn_row.set_margin_top(12);
        btn_row.set_margin_bottom(16);
        btn_row.append(&cancel_btn);
        btn_row.append(&save_btn);

        // ── Layout ────────────────────────────────────────────────────────
        let body = GtkBox::new(Orientation::Vertical, 0);
        body.append(&group);
        body.append(&note);
        body.append(&btn_row);

        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&header);
        tv.set_content(Some(&body));
        window.set_content(Some(&tv));

        // ── Signals ───────────────────────────────────────────────────────
        let win_for_cancel = window.clone();
        cancel_btn.connect_clicked(move |_| win_for_cancel.close());

        let on_saved = Rc::new(RefCell::new(Some(on_saved)));
        let win_for_save = window.clone();
        let rel_names = Rc::new(rel_names);
        save_btn.connect_clicked(move |_| {
            let idx = root_row.selected() as usize;
            let root_file = rel_names.get(idx).map(|s| PathBuf::from(s));

            let bib_text = bib_row.text().trim().to_string();
            let bib_path = if bib_text.is_empty() { None } else { Some(PathBuf::from(bib_text)) };

            let new_cfg = ProjectConfig {
                root_file,
                bib_path,
                ..current.clone()
            };

            if let Err(e) = new_cfg.save(&project_root) {
                let dlg = adw::MessageDialog::new(
                    Some(&win_for_save),
                    Some("Could not save project settings"),
                    Some(&e.to_string()),
                );
                dlg.add_response("ok", "OK");
                dlg.present();
                return;
            }

            win_for_save.close();
            if let Some(cb) = on_saved.borrow_mut().take() {
                cb(new_cfg);
            }
        });

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

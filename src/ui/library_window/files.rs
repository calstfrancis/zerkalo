use super::*;

impl LibraryWindow {
    /// Moves a document saved outside the Zerkalo folder (and its comment/
    /// template sidecars, if present) into it, picking a free name with
    /// `name_prompt::suggest_free_name` on a collision. Refuses when the
    /// document is currently open — nothing here retargets an open editor
    /// tab's in-memory path, so moving out from under it would leave that
    /// tab pointing at a file that no longer exists.
    pub(super) fn move_into_work_dir(&self, doc: &crate::library::Document) {
        if self.is_open.borrow().as_ref().is_some_and(|f| f(&doc.path)) {
            let dlg = adw::MessageDialog::new(
                Some(&self.window),
                Some("Close the document first"),
                Some("This document is open in the editor. Close its tab, then move it."),
            );
            dlg.add_response("ok", "OK");
            dlg.present();
            return;
        }

        let new_path = match crate::ui::name_prompt::move_into_dir(
            &doc.path,
            &self.work_dir,
            &[
                crate::comments::sidecar_path as fn(&Path) -> PathBuf,
                crate::ui::template_dialog::sidecar_path as fn(&Path) -> PathBuf,
            ],
        ) {
            Ok(p) => p,
            Err(e) => {
                let dlg = adw::MessageDialog::new(
                    Some(&self.window),
                    Some("Couldn't move the document"),
                    Some(&format!("{e}")),
                );
                dlg.add_response("ok", "OK");
                dlg.present();
                return;
            }
        };

        self.library
            .borrow_mut()
            .update_path(doc.id, &new_path)
            .ok();
        self.refresh();
        let t = adw::Toast::new(&format!(
            "Moved into {}",
            self.work_dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "your Zerkalo folder".to_string())
        ));
        t.set_timeout(4);
        self.toast_overlay.add_toast(t);
    }

    pub(super) fn new_document(&self) {
        let templates_dir = self.work_dir.join("Templates");
        let templates: Vec<std::path::PathBuf> = if templates_dir.is_dir() {
            std::fs::read_dir(&templates_dir)
                .ok()
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| p.extension().map(|e| e == "typ").unwrap_or(false))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            vec![]
        };

        let dlg = adw::MessageDialog::new(Some(&self.window), Some("New Document"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("blank", "Blank Document");
        dlg.set_response_appearance("blank", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("blank"));
        dlg.set_close_response("cancel");

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(100);
        scroll.set_width_request(260);
        let listbox = ListBox::new();
        listbox.set_selection_mode(gtk4::SelectionMode::Single);
        for t in &templates {
            let name = t
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let r = ListBoxRow::new();
            r.set_widget_name(&t.to_string_lossy());
            r.set_child(Some(
                &Label::builder()
                    .label(&name)
                    .halign(Align::Start)
                    .margin_top(6)
                    .margin_bottom(6)
                    .margin_start(8)
                    .build(),
            ));
            listbox.append(&r);
        }
        scroll.set_child(Some(&listbox));
        dlg.set_extra_child(Some(&scroll));

        let this = self.clone();
        let dlg_weak = dlg.downgrade();
        listbox.connect_row_activated(move |_, row| {
            let tpl = std::path::PathBuf::from(row.widget_name().to_string());
            this.create_new_from_template(Some(&tpl));
            if let Some(d) = dlg_weak.upgrade() {
                d.close();
            }
        });

        let this = self.clone();
        let listbox_c = listbox.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "blank" {
                let selected = listbox_c
                    .selected_row()
                    .map(|r| std::path::PathBuf::from(r.widget_name().to_string()));
                this.create_new_from_template(selected.as_deref());
            }
        });
        dlg.present();
    }

    pub(super) fn create_new_from_template(&self, template: Option<&std::path::Path>) {
        let content = template
            .and_then(|t| std::fs::read(t).ok())
            .unwrap_or_default();
        let this = self.clone();
        let suggested = crate::ui::name_prompt::suggest_free_name(&self.work_dir, "Untitled");
        crate::ui::name_prompt::ask_document_name(
            &self.window,
            &self.work_dir,
            "New Document",
            "Create",
            &suggested,
            move |path| {
                if std::fs::write(&path, &content).is_err() {
                    tracing::warn!("Failed to create document at {}", path.display());
                    return;
                }
                this.library.borrow_mut().upsert_document(&path).ok();
                if let Some(cb) = this.on_open.borrow().as_ref() {
                    cb(path);
                }
                this.refresh();
            },
        );
    }

    pub(super) fn import_document(&self) {
        let dialog = gtk4::FileDialog::new();
        dialog.set_title("Import Typst Document");
        let filter = gtk4::FileFilter::new();
        filter.add_pattern("*.typ");
        filter.set_name(Some("Typst files"));
        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&self.work_dir)));

        let this = self.clone();
        dialog.open(
            Some(&self.window),
            gtk4::gio::Cancellable::NONE,
            move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        this.library.borrow_mut().upsert_document(&path).ok();
                        this.library.borrow_mut().touch_opened(&path).ok();
                        if let Some(cb) = this.on_open.borrow().as_ref() {
                            cb(path);
                        }
                        this.refresh();
                    }
                }
            },
        );
    }

    pub(super) fn export_doc_dialog(&self, doc: &crate::library::Document) {
        let dialog = gtk4::FileDialog::new();
        dialog.set_title("Export PDF");
        let stem = doc
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "document".to_string());
        dialog.set_initial_name(Some(&format!("{stem}.pdf")));
        let filter = gtk4::FileFilter::new();
        filter.set_name(Some("PDF files (*.pdf)"));
        filter.add_pattern("*.pdf");
        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));

        let src = doc.path.clone();
        let window = self.window.clone();
        let config = self.config.clone();
        dialog.save(
            Some(&self.window),
            gtk4::gio::Cancellable::NONE,
            move |res| {
                let dest = match res.ok().and_then(|f| f.path()) {
                    Some(p) => p,
                    None => return,
                };
                let dest = if dest.extension().is_none() {
                    dest.with_extension("pdf")
                } else {
                    dest
                };

                // bib_path/cv_elements_path are project-wide config, the same
                // ones PreviewPane::compile_inputs() reads — apply to every
                // document in this project's library, not just the one
                // currently open in the editor, so resolving them from
                // `config` here (rather than an empty HashMap/None, as this
                // used to) fixes CV-mode and out-of-project-bibliography
                // documents exporting blank/uncited PDFs from this dialog.
                let cfg = config.borrow();
                let mut sys_inputs = std::collections::HashMap::new();
                if let Some(cv_path) = cfg.cv_elements_path.clone() {
                    match std::fs::read_to_string(&cv_path) {
                        Ok(yaml) => {
                            sys_inputs.insert("skrizhal-cv-data".to_string(), yaml);
                        }
                        Err(e) => {
                            tracing::warn!("CV mode: couldn't read {}: {e}", cv_path.display());
                        }
                    }
                }
                let bib_path = cfg.bib_path.clone();

                let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Vec<u8>, String>>(1);
                let src_for_thread = src.clone();
                std::thread::spawn(move || {
                    let result = crate::compiler::compile_to_pdf_bytes(
                        &src_for_thread,
                        &std::collections::HashMap::new(),
                        &sys_inputs,
                        bib_path.as_deref(),
                    )
                    .map_err(|e| e.to_string());
                    let _ = tx.send(result);
                });

                let window_err = window.clone();
                crate::ui::async_poll::poll_result(
                    rx,
                    std::time::Duration::from_millis(100),
                    move |bytes| {
                        if let Err(e) = std::fs::write(&dest, &bytes) {
                            show_export_error(&window, &e.to_string());
                        }
                    },
                    move |e| show_export_error(&window_err, &e),
                );
            },
        );
    }
}

pub(super) fn show_export_error(parent: &adw::Window, msg: &str) {
    let dlg = adw::MessageDialog::new(Some(parent), Some("Couldn't make the export"), Some(msg));
    dlg.add_response("ok", "OK");
    dlg.present();
}

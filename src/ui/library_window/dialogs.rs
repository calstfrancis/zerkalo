use super::*;

impl LibraryWindow {
    pub(super) fn edit_notes_dialog(&self, doc: &crate::library::Document) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Notes"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Save");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(120);
        scroll.set_width_request(320);
        let text_view = TextView::new();
        text_view.set_wrap_mode(gtk4::WrapMode::Word);
        text_view.set_top_margin(8);
        text_view.set_bottom_margin(8);
        text_view.set_left_margin(8);
        text_view.set_right_margin(8);
        if let Some(notes) = &doc.notes {
            text_view.buffer().set_text(notes);
        }
        scroll.set_child(Some(&text_view));
        dlg.set_extra_child(Some(&scroll));

        let this = self.clone();
        let id = doc.id;
        let buf = text_view.buffer();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let text = buf
                    .text(&buf.start_iter(), &buf.end_iter(), false)
                    .to_string();
                let notes_owned: Option<String> = if text.trim().is_empty() {
                    None
                } else {
                    Some(text.trim().to_string())
                };
                this.library
                    .borrow_mut()
                    .set_notes(id, notes_owned.as_deref())
                    .ok();
            }
        });
        dlg.present();
    }

    pub(super) fn rename_project_dialog(&self, project_id: i64, current_name: &str) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Rename Project"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Rename");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_text(current_name);
        entry.set_activates_default(true);
        dlg.set_extra_child(Some(&entry));
        let this = self.clone();
        let entry_c = entry.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let name = entry_c.text().to_string();
                if !name.trim().is_empty() {
                    this.library
                        .borrow_mut()
                        .rename_project(project_id, name.trim())
                        .ok();
                    this.refresh();
                }
            }
        });
        dlg.present();
    }

    pub(super) fn rename_doc_dialog(&self, doc: &crate::library::Document) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Rename Document"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Rename");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_text(&doc.title);
        entry.set_activates_default(true);
        dlg.set_extra_child(Some(&entry));
        let this = self.clone();
        let id = doc.id;
        let entry_c = entry.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let title = entry_c.text().to_string();
                if !title.trim().is_empty() {
                    this.library.borrow_mut().set_title(id, title.trim()).ok();
                    this.populate_doc_list();
                }
            }
        });
        dlg.present();
    }

    /// Asks for a name and makes a label of it. No colour is asked for — it
    /// gets one from the palette, and Change Color… is there if it matters.
    pub(super) fn create_label_dialog(&self) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("New Label"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Create");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Label name"));
        entry.set_activates_default(true);
        dlg.set_extra_child(Some(&entry));
        let this = self.clone();
        let entry_c = entry.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let name = entry_c.text().to_string();
                if !name.trim().is_empty() {
                    this.library.borrow_mut().create_label(name.trim()).ok();
                    this.refresh();
                }
            }
        });
        dlg.present();
    }

    pub(super) fn rename_label_dialog(&self, label_id: i64) {
        let current = self.filter_label(&LibraryFilter::Label(label_id));
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Rename Label"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Rename");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_text(&current);
        entry.set_activates_default(true);
        dlg.set_extra_child(Some(&entry));
        let this = self.clone();
        let entry_c = entry.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp != "ok" {
                return;
            }
            let name = entry_c.text().to_string();
            let name = name.trim();
            if name.is_empty() || name == current {
                return;
            }
            let renamed = this.library.borrow_mut().rename_label(label_id, name);
            match renamed {
                Ok(()) => this.refresh(),
                Err(_) => {
                    // Names are unique, ignoring case.
                    let toast = adw::Toast::new("There is already a label with that name.");
                    this.toast_overlay.add_toast(toast);
                }
            }
        });
        dlg.present();
    }

    /// Picks a label's colour from the palette, or hands the choice back to
    /// Zerkalo ("Automatic").
    pub(super) fn label_color_dialog(&self, label_id: i64) {
        let label = self
            .library
            .borrow()
            .all_labels()
            .unwrap_or_default()
            .into_iter()
            .find(|l| l.id == label_id);
        let Some(label) = label else {
            return;
        };
        let dlg = adw::MessageDialog::new(
            Some(&self.window),
            Some(&format!("Color for \u{201c}{}\u{201d}", label.name)),
            None,
        );
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Set");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        // `None` is "Automatic". A row of linked toggles, so the chosen one
        // stays visibly pressed.
        let chosen: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(label.color_hex.clone()));
        let row = GtkBox::new(Orientation::Horizontal, 6);
        row.set_halign(Align::Center);
        let automatic = gtk4::ToggleButton::with_label("Automatic");
        automatic.set_active(label.color_hex.is_none());
        {
            let chosen = chosen.clone();
            automatic.connect_toggled(move |b| {
                if b.is_active() {
                    *chosen.borrow_mut() = None;
                }
            });
        }
        row.append(&automatic);
        for color in TAG_COLORS {
            let btn = gtk4::ToggleButton::new();
            btn.set_group(Some(&automatic));
            btn.set_size_request(24, 24);
            apply_color_css(&btn, color);
            btn.set_active(label.color_hex.as_deref() == Some(*color));
            let chosen = chosen.clone();
            let c = color.to_string();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    *chosen.borrow_mut() = Some(c.clone());
                }
            });
            row.append(&btn);
        }
        dlg.set_extra_child(Some(&row));

        let this = self.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let color = chosen.borrow().clone();
                this.library
                    .borrow_mut()
                    .set_label_color(label_id, color.as_deref())
                    .ok();
                this.refresh();
            }
        });
        dlg.present();
    }

    pub(super) fn create_project_dialog(&self) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("New Project"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Create");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Project name"));
        entry.set_activates_default(true);
        dlg.set_extra_child(Some(&entry));
        let this = self.clone();
        let entry_c = entry.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let name = entry_c.text().to_string();
                if !name.trim().is_empty() {
                    this.library.borrow_mut().create_project(name.trim()).ok();
                    this.refresh();
                }
            }
        });
        dlg.present();
    }

    pub(super) fn permanent_delete_dialog(&self, doc: &crate::library::Document) {
        let dlg = adw::MessageDialog::new(
            Some(&self.window),
            Some("Permanently Delete?"),
            Some(&format!(
                "This permanently deletes “{}” from disk. This cannot be undone.",
                doc.title
            )),
        );
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("delete", "Delete");
        dlg.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dlg.set_default_response(Some("cancel"));
        dlg.set_close_response("cancel");
        let this = self.clone();
        let id = doc.id;
        dlg.connect_response(None, move |_, resp| {
            if resp == "delete" {
                if let Err(e) = this.library.borrow_mut().permanently_delete(id) {
                    tracing::error!("permanently_delete failed: {e}");
                    let toast =
                        adw::Toast::new(&format!("Couldn't delete it — {}.", e.user_message()));
                    this.toast_overlay.add_toast(toast);
                }
                this.refresh();
            }
        });
        dlg.present();
    }
}

// GTK 4.10 deprecated per-widget CSS providers in favour of a display-wide
// provider plus a CSS class. These two set a colour that is only known at
// runtime (a tag's own hex), which the class-based approach cannot express
// without generating a class per colour — left as-is deliberately.
#[allow(deprecated)]
pub(super) fn apply_color_css(widget: &impl IsA<gtk4::Widget>, color: &str) {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(&format!(
        "button {{ background: {color}; border-radius: 4px; min-width: 16px; min-height: 16px; }}"
    ));
    widget
        .as_ref()
        .style_context()
        .add_provider(&provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

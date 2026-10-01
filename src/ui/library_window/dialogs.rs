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

    pub(super) fn rename_category_dialog(&self, current_name: &str) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Rename Category"), None);
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
        let old_name = current_name.to_string();
        let entry_c = entry.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let new_name = entry_c.text().to_string();
                if !new_name.trim().is_empty() && new_name.trim() != old_name {
                    this.library
                        .borrow_mut()
                        .rename_category(&old_name, new_name.trim())
                        .ok();
                    *this.current_filter.borrow_mut() = LibraryFilter::All;
                    this.refresh();
                }
            }
        });
        dlg.present();
    }

    pub(super) fn recolor_category_dialog(&self, name: &str) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Recolor Category"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Set");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let color_row = GtkBox::new(Orientation::Horizontal, 4);
        let initial_color = self
            .library
            .borrow()
            .get_category_color(name)
            .unwrap_or_else(|| stable_palette_color(name).to_string());
        let selected_color: Rc<RefCell<String>> = Rc::new(RefCell::new(initial_color));
        for color in TAG_COLORS {
            let btn = Button::new();
            btn.set_size_request(20, 20);
            apply_color_css(&btn, color);
            let sel = selected_color.clone();
            let c = color.to_string();
            btn.connect_clicked(move |_| *sel.borrow_mut() = c.clone());
            color_row.append(&btn);
        }
        dlg.set_extra_child(Some(&color_row));

        let this = self.clone();
        let name = name.to_string();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let color = selected_color.borrow().clone();
                this.library
                    .borrow_mut()
                    .set_category_color(&name, &color)
                    .ok();
                this.refresh();
            }
        });
        dlg.present();
    }

    pub(super) fn add_subcategory_dialog(&self, parent_name: &str) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Add Subcategory"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Add");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Subcategory name"));
        entry.set_activates_default(true);
        dlg.set_extra_child(Some(&entry));
        let this = self.clone();
        let parent_for_dialog = parent_name.to_string();
        let entry_c = entry.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let name = entry_c.text().to_string();
                let trimmed = name.trim().to_string();
                if !trimmed.is_empty() {
                    this.library
                        .borrow_mut()
                        .create_category(&trimmed, Some(&parent_for_dialog))
                        .ok();
                    this.refresh();
                }
            }
        });
        dlg.present();
    }

    pub(super) fn set_parent_dialog(&self, cat_name: &str) {
        let all_cats = self
            .library
            .borrow()
            .all_categories_structured()
            .unwrap_or_default();
        // Top-level categories with no parent and no children of their own (avoid cycles, keep max 2 levels)
        let parent_names: std::collections::HashSet<String> =
            all_cats.iter().filter_map(|c| c.parent.clone()).collect();
        let candidates: Vec<String> = all_cats
            .iter()
            .filter(|c| c.parent.is_none() && c.name != cat_name && !parent_names.contains(&c.name))
            .map(|c| c.name.clone())
            .collect();

        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Set Parent Category"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Set");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let vbox = GtkBox::new(Orientation::Vertical, 4);
        let listbox = ListBox::new();
        listbox.add_css_class("boxed-list");
        listbox.set_selection_mode(gtk4::SelectionMode::Single);

        let none_row = ListBoxRow::new();
        none_row.set_widget_name("__none__");
        let none_lbl = Label::new(Some("None (top-level)"));
        none_lbl.set_margin_top(8);
        none_lbl.set_margin_bottom(8);
        none_lbl.set_margin_start(8);
        none_row.set_child(Some(&none_lbl));
        listbox.append(&none_row);

        for parent in &candidates {
            let r = ListBoxRow::new();
            r.set_widget_name(parent.as_str());
            let lbl = Label::new(Some(parent.as_str()));
            lbl.set_margin_top(8);
            lbl.set_margin_bottom(8);
            lbl.set_margin_start(8);
            lbl.set_halign(Align::Start);
            r.set_child(Some(&lbl));
            listbox.append(&r);
        }

        vbox.append(&listbox);
        dlg.set_extra_child(Some(&vbox));

        let this = self.clone();
        let cat = cat_name.to_string();
        let listbox_c = listbox.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let parent_value = listbox_c
                    .selected_row()
                    .map(|r| r.widget_name().to_string())
                    .filter(|n| n != "__none__");
                this.library
                    .borrow_mut()
                    .set_category_parent(&cat, parent_value.as_deref())
                    .ok();
                this.refresh();
            }
        });
        dlg.present();
    }

    pub(super) fn bulk_tag_dialog(&self, doc_ids: Vec<i64>) {
        let all_tags = self.library.borrow().all_tags().unwrap_or_default();
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Tag Documents"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Apply");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let container = GtkBox::new(Orientation::Vertical, 6);
        container.set_width_request(300);

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(160);
        let listbox = ListBox::new();
        listbox.add_css_class("boxed-list");
        listbox.set_selection_mode(gtk4::SelectionMode::None);

        let checks: Rc<RefCell<Vec<(i64, CheckButton)>>> = Rc::new(RefCell::new(Vec::new()));
        for tag in &all_tags {
            let check = CheckButton::with_label(&tag.name);
            let r = ListBoxRow::new();
            r.set_selectable(false);
            r.set_child(Some(&check));
            listbox.append(&r);
            checks.borrow_mut().push((tag.id, check));
        }
        scroll.set_child(Some(&listbox));
        container.append(&scroll);

        // Inline new-tag row — mirrors edit_tags_dialog's, so creating a tag
        // that doesn't exist yet doesn't require detouring through Manage Tags.
        let new_tag_box = GtkBox::new(Orientation::Horizontal, 4);
        new_tag_box.set_margin_top(4);
        let new_tag_entry = Entry::new();
        new_tag_entry.set_placeholder_text(Some("New tag…"));
        new_tag_entry.set_hexpand(true);
        new_tag_entry.set_activates_default(false);
        new_tag_box.append(&new_tag_entry);

        let new_color: Rc<RefCell<String>> = Rc::new(RefCell::new(TAG_COLORS[0].to_string()));
        for color in TAG_COLORS {
            let btn = Button::new();
            btn.set_size_request(20, 20);
            apply_color_css(&btn, color);
            let sel = new_color.clone();
            let c = color.to_string();
            btn.connect_clicked(move |_| *sel.borrow_mut() = c.clone());
            new_tag_box.append(&btn);
        }

        let add_tag_btn = Button::with_label("+");
        add_tag_btn.add_css_class("suggested-action");
        new_tag_box.append(&add_tag_btn);
        container.append(&new_tag_box);
        dlg.set_extra_child(Some(&container));

        {
            let this = self.clone();
            let entry = new_tag_entry.clone();
            let color = new_color.clone();
            let listbox_c = listbox.clone();
            let checks_c = checks.clone();
            let add_tag_btn_c = add_tag_btn.clone();
            add_tag_btn.connect_clicked(move |_| {
                let name = entry.text().to_string();
                let name = name.trim().to_string();
                if name.is_empty() {
                    return;
                }
                let color_val = color.borrow().clone();
                let result = this.library.borrow_mut().create_tag(&name, &color_val);
                if let Ok(new_id) = result {
                    let check = CheckButton::with_label(&name);
                    check.set_active(true);
                    let r = ListBoxRow::new();
                    r.set_selectable(false);
                    r.set_child(Some(&check));
                    listbox_c.append(&r);
                    checks_c.borrow_mut().push((new_id, check));
                    entry.set_text("");
                    this.populate_filter_list();
                }
            });
            new_tag_entry.connect_activate(move |_| {
                add_tag_btn_c.emit_clicked();
            });
        }

        let this = self.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let selected: Vec<i64> = checks
                    .borrow()
                    .iter()
                    .filter(|(_, c)| c.is_active())
                    .map(|(id, _)| *id)
                    .collect();
                for doc_id in &doc_ids {
                    this.library
                        .borrow_mut()
                        .add_doc_tags(*doc_id, &selected)
                        .ok();
                }
                this.selection.borrow_mut().clear();
                this.update_action_bar();
                this.refresh();
            }
        });
        dlg.present();
    }

    pub(super) fn bulk_add_to_project_dialog(&self, doc_ids: Vec<i64>) {
        let projects = self.library.borrow().all_projects().unwrap_or_default();
        if projects.is_empty() {
            return;
        }
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Add to Project"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.set_close_response("cancel");

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(150);
        scroll.set_width_request(300);
        let listbox = ListBox::new();
        listbox.set_selection_mode(gtk4::SelectionMode::Single);
        for p in &projects {
            let r = ListBoxRow::new();
            r.set_widget_name(&p.id.to_string());
            r.set_child(Some(
                &Label::builder()
                    .label(&p.name)
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
            if let Ok(pid) = row.widget_name().to_string().parse::<i64>() {
                for doc_id in &doc_ids {
                    this.library
                        .borrow_mut()
                        .add_doc_to_project(pid, *doc_id)
                        .ok();
                }
                this.selection.borrow_mut().clear();
                this.update_action_bar();
                this.refresh();
                if let Some(d) = dlg_weak.upgrade() {
                    d.close();
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

    /// Builds the shared part of the category-checkbox dialogs (single-doc
    /// edit and bulk categorize): a scrollable checklist grouped by parent
    /// (indented children under their parent, standalone categories after),
    /// plus an inline "New category…" row with color swatches and a "+"
    /// button that creates a top-level category and checks it. Mirrors
    /// `edit_tags_dialog`'s inline-create block, adapted for categories'
    /// tree structure and optional (rather than always-set) color.
    pub(super) fn build_category_checklist(
        &self,
        all_cats: &[crate::library::Category],
        checked: &std::collections::HashSet<String>,
    ) -> (GtkBox, Rc<RefCell<Vec<(String, CheckButton)>>>) {
        let container = GtkBox::new(Orientation::Vertical, 6);
        container.set_width_request(300);

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(160);
        let listbox = ListBox::new();
        listbox.add_css_class("boxed-list");
        listbox.set_selection_mode(gtk4::SelectionMode::None);

        let checks: Rc<RefCell<Vec<(String, CheckButton)>>> = Rc::new(RefCell::new(Vec::new()));
        let parent_names: std::collections::HashSet<String> =
            all_cats.iter().filter_map(|c| c.parent.clone()).collect();
        let add_row = |listbox: &ListBox,
                       checks: &Rc<RefCell<Vec<(String, CheckButton)>>>,
                       name: &str,
                       label: &str,
                       active: bool| {
            let check = CheckButton::with_label(label);
            check.set_active(active);
            let r = ListBoxRow::new();
            r.set_selectable(false);
            r.set_child(Some(&check));
            listbox.append(&r);
            checks.borrow_mut().push((name.to_string(), check));
        };
        for cat in all_cats.iter().filter(|c| c.parent.is_none()) {
            add_row(
                &listbox,
                &checks,
                &cat.name,
                &cat.name,
                checked.contains(&cat.name),
            );
            if parent_names.contains(&cat.name) {
                for child in all_cats
                    .iter()
                    .filter(|c| c.parent.as_deref() == Some(cat.name.as_str()))
                {
                    add_row(
                        &listbox,
                        &checks,
                        &child.name,
                        &format!("    {}", child.name),
                        checked.contains(&child.name),
                    );
                }
            }
        }
        scroll.set_child(Some(&listbox));
        container.append(&scroll);

        let new_cat_box = GtkBox::new(Orientation::Horizontal, 4);
        new_cat_box.set_margin_top(4);
        let new_cat_entry = Entry::new();
        new_cat_entry.set_placeholder_text(Some("New category…"));
        new_cat_entry.set_hexpand(true);
        new_cat_box.append(&new_cat_entry);

        let new_color: Rc<RefCell<String>> = Rc::new(RefCell::new(TAG_COLORS[0].to_string()));
        // Only applied if the user actually clicks a swatch — matches the
        // old single-category dialog's care about not force-coloring every
        // new category with whatever the first swatch happens to be.
        let color_picked = Rc::new(std::cell::Cell::new(false));
        for color in TAG_COLORS {
            let btn = Button::new();
            btn.set_size_request(20, 20);
            apply_color_css(&btn, color);
            let sel = new_color.clone();
            let picked = color_picked.clone();
            let c = color.to_string();
            btn.connect_clicked(move |_| {
                *sel.borrow_mut() = c.clone();
                picked.set(true);
            });
            new_cat_box.append(&btn);
        }

        let add_cat_btn = Button::with_label("+");
        add_cat_btn.add_css_class("suggested-action");
        new_cat_box.append(&add_cat_btn);
        container.append(&new_cat_box);

        {
            let this = self.clone();
            let entry = new_cat_entry.clone();
            let color = new_color.clone();
            let listbox_c = listbox.clone();
            let checks_c = checks.clone();
            let add_cat_btn_c = add_cat_btn.clone();
            add_cat_btn.connect_clicked(move |_| {
                let name = entry.text().to_string();
                let name = name.trim().to_string();
                if name.is_empty() {
                    return;
                }
                this.library.borrow_mut().create_category(&name, None).ok();
                if color_picked.get() {
                    let color_val = color.borrow().clone();
                    this.library
                        .borrow_mut()
                        .set_category_color(&name, &color_val)
                        .ok();
                }
                let check = CheckButton::with_label(&name);
                check.set_active(true);
                let r = ListBoxRow::new();
                r.set_selectable(false);
                r.set_child(Some(&check));
                listbox_c.append(&r);
                checks_c.borrow_mut().push((name, check));
                entry.set_text("");
                this.populate_filter_list();
            });
            new_cat_entry.connect_activate(move |_| {
                add_cat_btn_c.emit_clicked();
            });
        }

        (container, checks)
    }

    pub(super) fn edit_categories_dialog(&self, doc_id: i64) {
        let all_cats = self
            .library
            .borrow()
            .all_categories_structured()
            .unwrap_or_default();
        let current: std::collections::HashSet<String> = self
            .library
            .borrow()
            .doc_categories(doc_id)
            .unwrap_or_default()
            .into_iter()
            .map(|c| c.name)
            .collect();

        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Edit Categories"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Save");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let (container, checks) = self.build_category_checklist(&all_cats, &current);
        dlg.set_extra_child(Some(&container));

        let this = self.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let selected: Vec<String> = checks
                    .borrow()
                    .iter()
                    .filter(|(_, c)| c.is_active())
                    .map(|(name, _)| name.clone())
                    .collect();
                this.library
                    .borrow_mut()
                    .set_doc_categories(doc_id, &selected)
                    .ok();
                this.populate_doc_list();
            }
        });
        dlg.present();
    }

    pub(super) fn bulk_category_dialog(&self, doc_ids: Vec<i64>) {
        let all_cats = self
            .library
            .borrow()
            .all_categories_structured()
            .unwrap_or_default();
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Categorize Documents"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Apply");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let (container, checks) =
            self.build_category_checklist(&all_cats, &std::collections::HashSet::new());
        dlg.set_extra_child(Some(&container));

        let this = self.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let selected: Vec<String> = checks
                    .borrow()
                    .iter()
                    .filter(|(_, c)| c.is_active())
                    .map(|(name, _)| name.clone())
                    .collect();
                for doc_id in &doc_ids {
                    this.library
                        .borrow_mut()
                        .add_doc_categories(*doc_id, &selected)
                        .ok();
                }
                this.selection.borrow_mut().clear();
                this.update_action_bar();
                this.refresh();
            }
        });
        dlg.present();
    }

    pub(super) fn edit_tags_dialog(&self, doc_id: i64) {
        let all_tags = self.library.borrow().all_tags().unwrap_or_default();
        let current: Vec<i64> = self
            .library
            .borrow()
            .doc_tags(doc_id)
            .unwrap_or_default()
            .iter()
            .map(|t| t.id)
            .collect();

        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Edit Tags"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Save");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let container = GtkBox::new(Orientation::Vertical, 6);
        container.set_width_request(300);

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(160);
        let listbox = ListBox::new();
        listbox.add_css_class("boxed-list");
        listbox.set_selection_mode(gtk4::SelectionMode::None);

        let checks: Rc<RefCell<Vec<(i64, CheckButton)>>> = Rc::new(RefCell::new(Vec::new()));
        for tag in &all_tags {
            let check = CheckButton::with_label(&tag.name);
            check.set_active(current.contains(&tag.id));
            let r = ListBoxRow::new();
            r.set_selectable(false);
            r.set_child(Some(&check));
            listbox.append(&r);
            checks.borrow_mut().push((tag.id, check));
        }
        scroll.set_child(Some(&listbox));
        container.append(&scroll);

        // Inline new-tag row
        let new_tag_box = GtkBox::new(Orientation::Horizontal, 4);
        new_tag_box.set_margin_top(4);
        let new_tag_entry = Entry::new();
        new_tag_entry.set_placeholder_text(Some("New tag…"));
        new_tag_entry.set_hexpand(true);
        new_tag_box.append(&new_tag_entry);

        let new_color: Rc<RefCell<String>> = Rc::new(RefCell::new(TAG_COLORS[0].to_string()));
        for color in TAG_COLORS {
            let btn = Button::new();
            btn.set_size_request(20, 20);
            apply_color_css(&btn, color);
            let sel = new_color.clone();
            let c = color.to_string();
            btn.connect_clicked(move |_| *sel.borrow_mut() = c.clone());
            new_tag_box.append(&btn);
        }

        let add_tag_btn = Button::with_label("+");
        add_tag_btn.add_css_class("suggested-action");
        new_tag_box.append(&add_tag_btn);
        container.append(&new_tag_box);

        dlg.set_extra_child(Some(&container));

        // Wire inline create
        {
            let this = self.clone();
            let entry = new_tag_entry.clone();
            let color = new_color.clone();
            let listbox_c = listbox.clone();
            let checks_c = checks.clone();
            add_tag_btn.connect_clicked(move |_| {
                let name = entry.text().to_string();
                let name = name.trim().to_string();
                if name.is_empty() {
                    return;
                }
                let color_val = color.borrow().clone();
                let result = this.library.borrow_mut().create_tag(&name, &color_val);
                if let Ok(new_id) = result {
                    let check = CheckButton::with_label(&name);
                    check.set_active(true);
                    let r = ListBoxRow::new();
                    r.set_selectable(false);
                    r.set_child(Some(&check));
                    listbox_c.append(&r);
                    checks_c.borrow_mut().push((new_id, check));
                    entry.set_text("");
                    this.populate_filter_list();
                }
            });
        }

        let this = self.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let selected: Vec<i64> = checks
                    .borrow()
                    .iter()
                    .filter(|(_, c)| c.is_active())
                    .map(|(id, _)| *id)
                    .collect();
                this.library
                    .borrow_mut()
                    .set_doc_tags(doc_id, &selected)
                    .ok();
                this.populate_doc_list();
            }
        });
        dlg.present();
    }

    pub(super) fn add_to_project_dialog(&self, doc_id: i64) {
        let projects = self.library.borrow().all_projects().unwrap_or_default();
        if projects.is_empty() {
            self.create_project_then_add(doc_id);
            return;
        }
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Add to Project"), None);
        dlg.add_response("new", "New Project…");
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Add");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(150);
        scroll.set_width_request(300);
        let listbox = ListBox::new();
        listbox.set_selection_mode(gtk4::SelectionMode::Single);
        for p in &projects {
            let r = ListBoxRow::new();
            r.set_widget_name(&p.id.to_string());
            r.set_child(Some(
                &Label::builder()
                    .label(&p.name)
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
        let listbox_c = listbox.clone();
        dlg.connect_response(None, move |_, resp| match resp {
            "new" => this.create_project_then_add(doc_id),
            "ok" => {
                if let Some(row) = listbox_c.selected_row() {
                    if let Ok(pid) = row.widget_name().to_string().parse::<i64>() {
                        this.library
                            .borrow_mut()
                            .add_doc_to_project(pid, doc_id)
                            .ok();
                        this.refresh();
                    }
                }
            }
            _ => {}
        });
        // Activate on row click
        let this2 = self.clone();
        let dlg_weak = dlg.downgrade();
        listbox.connect_row_activated(move |_, row| {
            if let Ok(pid) = row.widget_name().to_string().parse::<i64>() {
                this2
                    .library
                    .borrow_mut()
                    .add_doc_to_project(pid, doc_id)
                    .ok();
                this2.refresh();
                if let Some(d) = dlg_weak.upgrade() {
                    d.close();
                }
            }
        });
        dlg.present();
    }

    pub(super) fn create_project_then_add(&self, doc_id: i64) {
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
                    if let Ok(pid) = this.library.borrow_mut().create_project(name.trim()) {
                        this.library
                            .borrow_mut()
                            .add_doc_to_project(pid, doc_id)
                            .ok();
                    }
                    this.refresh();
                }
            }
        });
        dlg.present();
    }

    pub(super) fn create_category_dialog(&self) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("New Category"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Create");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let container = GtkBox::new(Orientation::Vertical, 8);
        container.set_width_request(280);
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Category name"));
        entry.set_activates_default(true);
        container.append(&entry);

        let color_row = GtkBox::new(Orientation::Horizontal, 4);
        let selected_color: Rc<RefCell<String>> = Rc::new(RefCell::new(TAG_COLORS[0].to_string()));
        for color in TAG_COLORS {
            let btn = Button::new();
            btn.set_size_request(20, 20);
            apply_color_css(&btn, color);
            let sel = selected_color.clone();
            let c = color.to_string();
            btn.connect_clicked(move |_| *sel.borrow_mut() = c.clone());
            color_row.append(&btn);
        }
        container.append(&color_row);
        dlg.set_extra_child(Some(&container));

        let this = self.clone();
        let entry_c = entry.clone();
        let color_sel = selected_color.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let name = entry_c.text().to_string();
                let name = name.trim().to_string();
                if !name.is_empty() {
                    this.library.borrow_mut().create_category(&name, None).ok();
                    let color = color_sel.borrow().clone();
                    this.library
                        .borrow_mut()
                        .set_category_color(&name, &color)
                        .ok();
                    this.refresh();
                }
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

    pub(super) fn show_manage_tags(&self) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Manage Tags"), None);
        dlg.add_response("close", "Close");
        dlg.set_close_response("close");

        let vbox = GtkBox::new(Orientation::Vertical, 4);
        vbox.set_width_request(320);

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(140);
        let tag_list = ListBox::new();
        tag_list.set_selection_mode(gtk4::SelectionMode::None);
        scroll.set_child(Some(&tag_list));
        vbox.append(&scroll);

        // Compact single row: [name entry] [color swatches] [+ button]
        let new_row = GtkBox::new(Orientation::Horizontal, 4);
        new_row.set_margin_top(2);
        let name_entry = Entry::new();
        name_entry.set_placeholder_text(Some("New tag…"));
        name_entry.set_hexpand(true);
        new_row.append(&name_entry);

        let selected_color: Rc<RefCell<String>> = Rc::new(RefCell::new(TAG_COLORS[0].to_string()));
        for color in TAG_COLORS {
            let btn = Button::new();
            btn.set_size_request(20, 20);
            apply_color_css(&btn, color);
            let sel = selected_color.clone();
            let c = color.to_string();
            btn.connect_clicked(move |_| *sel.borrow_mut() = c.clone());
            new_row.append(&btn);
        }

        let add_btn = Button::with_label("+");
        add_btn.add_css_class("suggested-action");
        new_row.append(&add_btn);
        vbox.append(&new_row);

        let this = self.clone();
        let tag_list_c = tag_list.clone();
        let refresh_slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let rs_outer = refresh_slot.clone();
        let refresh_tags: Rc<dyn Fn()> = Rc::new(move || {
            while let Some(child) = tag_list_c.first_child() {
                tag_list_c.remove(&child);
            }
            let tags = this.library.borrow().all_tags().unwrap_or_default();
            for tag in tags {
                let r = ListBoxRow::new();
                r.set_selectable(false);
                let hbox = GtkBox::new(Orientation::Horizontal, 6);
                hbox.set_margin_top(3);
                hbox.set_margin_bottom(3);
                hbox.set_margin_start(8);
                hbox.set_margin_end(4);
                let dot = Label::new(None);
                dot.set_use_markup(true);
                dot.set_markup(&format!("<span foreground=\"{}\">●</span>", tag.color_hex));
                hbox.append(&dot);
                let name = Label::new(Some(&tag.name));
                name.set_halign(Align::Start);
                name.set_hexpand(true);
                hbox.append(&name);

                let edit = Button::from_icon_name("document-edit-symbolic");
                edit.add_css_class("flat");
                edit.set_tooltip_text(Some("Rename tag"));
                edit.update_property(&[gtk4::accessible::Property::Label(&format!(
                    "Rename tag {}",
                    tag.name
                ))]);
                let this_e = this.clone();
                let tid_e = tag.id;
                let tag_name_e = tag.name.clone();
                let rs_e = rs_outer.clone();
                edit.connect_clicked(move |_| {
                    let entry = Entry::new();
                    entry.set_text(&tag_name_e);
                    entry.set_activates_default(true);
                    let rename_dlg =
                        adw::MessageDialog::new(Some(&this_e.window), Some("Rename Tag"), None);
                    rename_dlg.set_extra_child(Some(&entry));
                    rename_dlg.add_response("cancel", "Cancel");
                    rename_dlg.add_response("ok", "Rename");
                    rename_dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
                    rename_dlg.set_default_response(Some("ok"));
                    let this_r = this_e.clone();
                    let rs_r = rs_e.clone();
                    let entry_c = entry.clone();
                    rename_dlg.connect_response(None, move |dlg, resp| {
                        if resp == "ok" {
                            let new_name = entry_c.text().to_string();
                            if !new_name.trim().is_empty() {
                                this_r
                                    .library
                                    .borrow_mut()
                                    .rename_tag(tid_e, new_name.trim())
                                    .ok();
                                this_r.populate_filter_list();
                                if let Some(f) = rs_r.borrow().as_ref() {
                                    f();
                                }
                            }
                        }
                        dlg.close();
                    });
                    rename_dlg.present();
                });
                hbox.append(&edit);

                let del = Button::from_icon_name("user-trash-symbolic");
                del.add_css_class("flat");
                del.set_tooltip_text(Some("Delete tag"));
                del.update_property(&[gtk4::accessible::Property::Label(&format!(
                    "Delete tag {}",
                    tag.name
                ))]);
                let this2 = this.clone();
                let tid = tag.id;
                let rs_d = rs_outer.clone();
                del.connect_clicked(move |_| {
                    this2.library.borrow_mut().delete_tag(tid).ok();
                    this2.refresh();
                    if let Some(f) = rs_d.borrow().as_ref() {
                        f();
                    }
                });
                hbox.append(&del);
                r.set_child(Some(&hbox));
                tag_list_c.append(&r);
            }
        });
        *refresh_slot.borrow_mut() = Some(refresh_tags.clone());
        refresh_tags();

        {
            let this = self.clone();
            let name_entry = name_entry.clone();
            let sel = selected_color.clone();
            let refresh_tags = refresh_tags.clone();
            add_btn.connect_clicked(move |_| {
                let name = name_entry.text().to_string();
                if !name.trim().is_empty() {
                    let color_val = sel.borrow().clone();
                    this.library
                        .borrow_mut()
                        .create_tag(name.trim(), &color_val)
                        .ok();
                    name_entry.set_text("");
                    refresh_tags();
                    this.populate_filter_list();
                }
            });
        }

        dlg.set_extra_child(Some(&vbox));
        let this = self.clone();
        dlg.connect_response(None, move |_, _| this.refresh());
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

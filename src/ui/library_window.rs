use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, DragSource, DropTarget, Entry, Image, Label,
    ListBox, ListBoxRow, Orientation, Popover, ScrolledWindow, SearchEntry, Separator,
};
use libadwaita as adw;
use adw::prelude::*;

use crate::library::{Library, LibraryFilter};

const TAG_COLORS: &[&str] = &[
    "#3584e4", "#33d17a", "#f6d32d", "#ff7800", "#e01b24", "#9141ac", "#dc8add", "#986a44",
];

#[derive(Clone)]
pub struct LibraryWindow {
    window: adw::ApplicationWindow,
    library: Rc<RefCell<Library>>,
    doc_list: ListBox,
    filter_list: ListBox,
    search_entry: SearchEntry,
    current_filter: Rc<RefCell<LibraryFilter>>,
    #[allow(dead_code)]
    toast_overlay: adw::ToastOverlay,
    on_open: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
    work_dir: PathBuf,
}

impl LibraryWindow {
    pub fn new(app: &adw::Application, library: Rc<RefCell<Library>>, work_dir: PathBuf) -> Self {
        load_library_css();

        let window = adw::ApplicationWindow::new(app);
        window.set_title(Some("Library — Zerkalo"));
        window.set_default_width(900);
        window.set_default_height(650);

        let toast_overlay = adw::ToastOverlay::new();

        let root = GtkBox::new(Orientation::Horizontal, 0);

        // ── Left sidebar ────────────────────────────────────────────────────
        let sidebar = GtkBox::new(Orientation::Vertical, 0);
        sidebar.set_width_request(220);

        let sidebar_header = adw::HeaderBar::new();
        sidebar_header.add_css_class("flat");
        let sidebar_title = adw::WindowTitle::new("Library", "");
        sidebar_header.set_title_widget(Some(&sidebar_title));
        sidebar.append(&sidebar_header);

        let sidebar_scroll = ScrolledWindow::new();
        sidebar_scroll.set_vexpand(true);
        sidebar_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);

        let sidebar_inner = GtkBox::new(Orientation::Vertical, 0);

        let filter_list = ListBox::new();
        filter_list.add_css_class("navigation-sidebar");
        filter_list.set_selection_mode(gtk4::SelectionMode::Browse);
        sidebar_inner.append(&filter_list);

        let manage_box = GtkBox::new(Orientation::Vertical, 0);
        manage_box.set_margin_top(8);
        manage_box.set_margin_bottom(8);
        manage_box.set_margin_start(8);
        manage_box.set_margin_end(8);
        let new_project_btn = Button::with_label("New Project");
        new_project_btn.add_css_class("flat");
        manage_box.append(&new_project_btn);
        let manage_tags_btn = Button::with_label("Manage Tags");
        manage_tags_btn.add_css_class("flat");
        manage_box.append(&manage_tags_btn);
        sidebar_inner.append(&manage_box);

        sidebar_scroll.set_child(Some(&sidebar_inner));
        sidebar.append(&sidebar_scroll);

        root.append(&sidebar);
        root.append(&Separator::new(Orientation::Vertical));

        // ── Right area ──────────────────────────────────────────────────────
        let right = adw::ToolbarView::new();
        right.set_hexpand(true);

        let right_header = adw::HeaderBar::new();
        right_header.set_show_title(false);

        let search_entry = SearchEntry::new();
        search_entry.set_placeholder_text(Some("Search documents…"));
        search_entry.set_width_request(240);
        let start_box = GtkBox::new(Orientation::Horizontal, 6);
        start_box.append(&search_entry);
        right_header.pack_start(&start_box);

        let new_doc_btn = Button::with_label("New Document");
        new_doc_btn.add_css_class("suggested-action");
        let import_btn = Button::with_label("Import…");
        import_btn.add_css_class("flat");
        right_header.pack_end(&import_btn);
        right_header.pack_end(&new_doc_btn);

        right.add_top_bar(&right_header);

        let doc_scroll = ScrolledWindow::new();
        doc_scroll.set_vexpand(true);
        let doc_list = ListBox::new();
        doc_list.set_selection_mode(gtk4::SelectionMode::None);
        doc_scroll.set_child(Some(&doc_list));
        right.set_content(Some(&doc_scroll));

        root.append(&right);

        toast_overlay.set_child(Some(&root));
        window.set_content(Some(&toast_overlay));

        window.connect_close_request(|win| {
            win.set_visible(false);
            glib::Propagation::Stop
        });

        let lw = Self {
            window,
            library,
            doc_list,
            filter_list,
            search_entry,
            current_filter: Rc::new(RefCell::new(LibraryFilter::All)),
            toast_overlay,
            on_open: Rc::new(RefCell::new(None)),
            work_dir,
        };

        lw.populate_filter_list();
        lw.populate_doc_list();
        lw.wire_signals(&new_doc_btn, &import_btn, &manage_tags_btn, &new_project_btn);

        lw
    }

    fn wire_signals(
        &self,
        new_doc_btn: &Button,
        import_btn: &Button,
        manage_tags_btn: &Button,
        new_project_btn: &Button,
    ) {
        {
            let this = self.clone();
            self.filter_list.connect_row_selected(move |_, row| {
                if let Some(row) = row {
                    let name = row.widget_name().to_string();
                    let filter = parse_filter_name(&name);
                    *this.current_filter.borrow_mut() = filter;
                    this.populate_doc_list();
                }
            });
        }
        {
            let this = self.clone();
            self.search_entry.connect_search_changed(move |_| {
                this.populate_doc_list();
            });
        }
        {
            let this = self.clone();
            self.doc_list.connect_row_activated(move |_, row| {
                let doc_id = row.widget_name().to_string().parse::<i64>().ok();
                if let Some(id) = doc_id {
                    this.open_doc_by_id(id);
                }
            });
        }
        {
            let this = self.clone();
            new_doc_btn.connect_clicked(move |_| this.new_document());
        }
        {
            let this = self.clone();
            import_btn.connect_clicked(move |_| this.import_document());
        }
        {
            let this = self.clone();
            manage_tags_btn.connect_clicked(move |_| this.show_manage_tags());
        }
        {
            let this = self.clone();
            new_project_btn.connect_clicked(move |_| this.create_project_dialog());
        }
    }

    fn populate_filter_list(&self) {
        while let Some(child) = self.filter_list.first_child() {
            self.filter_list.remove(&child);
        }

        self.filter_list
            .append(&make_filter_row("all", "document-open-recent-symbolic", "All Documents", None));

        let projects = self
            .library
            .borrow()
            .all_projects()
            .unwrap_or_default();
        if !projects.is_empty() {
            self.filter_list.append(&header_row("PROJECTS"));
            for p in projects {
                self.filter_list.append(&make_filter_row(
                    &format!("project:{}", p.id),
                    "folder-symbolic",
                    &p.name,
                    None,
                ));
            }
        }

        let categories = self
            .library
            .borrow()
            .all_categories()
            .unwrap_or_default();
        if !categories.is_empty() {
            self.filter_list.append(&header_row("CATEGORIES"));
            for c in categories {
                let filter_row = make_filter_row(
                    &format!("category:{}", c),
                    "tag-symbolic",
                    &c,
                    None,
                );
                let drop = DropTarget::new(gtk4::glib::Type::STRING, gtk4::gdk::DragAction::COPY);
                let this = self.clone();
                let cat_name = c.clone();
                drop.connect_drop(move |_, value, _, _| {
                    if let Ok(id_str) = value.get::<String>() {
                        if let Ok(doc_id) = id_str.parse::<i64>() {
                            this.library.borrow_mut().set_category(doc_id, Some(&cat_name)).ok();
                            this.refresh();
                            return true;
                        }
                    }
                    false
                });
                filter_row.add_controller(drop);
                self.filter_list.append(&filter_row);
            }
        }

        let tags = self.library.borrow().all_tags().unwrap_or_default();
        if !tags.is_empty() {
            self.filter_list.append(&header_row("TAGS"));
            for t in tags {
                self.filter_list.append(&make_tag_filter_row(t.id, &t.name, &t.color_hex));
            }
        }

        self.filter_list.append(&header_row(""));
        self.filter_list
            .append(&make_filter_row("archive", "user-trash-symbolic", "Archive", None));

        if let Some(first) = self.filter_list.row_at_index(0) {
            self.filter_list.select_row(Some(&first));
        }
    }

    fn populate_doc_list(&self) {
        while let Some(child) = self.doc_list.first_child() {
            self.doc_list.remove(&child);
        }
        let search = self.search_entry.text().to_string();
        let filter = self.current_filter.borrow().clone();
        let docs = self
            .library
            .borrow()
            .documents(filter, &search)
            .unwrap_or_default();

        if docs.is_empty() {
            let placeholder = Label::new(Some("No documents."));
            placeholder.add_css_class("dim-label");
            placeholder.set_margin_top(40);
            placeholder.set_margin_bottom(40);
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            row.set_child(Some(&placeholder));
            self.doc_list.append(&row);
            return;
        }

        for doc in docs {
            let tags = self.library.borrow().doc_tags(doc.id).unwrap_or_default();
            let row = self.make_doc_row(&doc, &tags);
            self.doc_list.append(&row);
        }
    }

    fn make_doc_row(
        &self,
        doc: &crate::library::Document,
        tags: &[crate::library::Tag],
    ) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.set_widget_name(&doc.id.to_string());

        let hbox = GtkBox::new(Orientation::Horizontal, 12);
        hbox.set_margin_top(10);
        hbox.set_margin_bottom(10);
        hbox.set_margin_start(10);
        hbox.set_margin_end(10);

        let icon = Image::from_icon_name("text-x-generic-symbolic");
        icon.set_pixel_size(32);
        hbox.append(&icon);

        let vbox = GtkBox::new(Orientation::Vertical, 4);
        vbox.set_hexpand(true);

        let title = Label::new(Some(&doc.title));
        title.add_css_class("doc-title");
        title.set_halign(Align::Start);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        vbox.append(&title);

        let chips = GtkBox::new(Orientation::Horizontal, 4);
        if let Some(cat) = &doc.category {
            let chip = Label::new(Some(cat));
            chip.add_css_class("category-chip");
            chip.add_css_class("caption");
            chips.append(&chip);
        }
        for tag in tags.iter().take(4) {
            let chip = Label::new(Some(&tag.name));
            chip.add_css_class("tag-chip");
            chip.add_css_class("caption");
            chips.append(&chip);
        }
        vbox.append(&chips);

        hbox.append(&vbox);

        let meta = GtkBox::new(Orientation::Vertical, 2);
        meta.set_halign(Align::End);
        meta.set_valign(Align::Center);
        let date = Label::new(Some(&format_date(&doc.modified_at)));
        date.add_css_class("dim-label");
        date.add_css_class("caption");
        date.set_halign(Align::End);
        meta.append(&date);
        if doc.archived {
            let badge = Label::new(Some("[archived]"));
            badge.add_css_class("dim-label");
            badge.add_css_class("caption");
            badge.set_halign(Align::End);
            meta.append(&badge);
        }
        hbox.append(&meta);

        row.set_child(Some(&hbox));

        // Drag source — carry doc ID as a string for drop-on-category
        let drag_source = DragSource::new();
        drag_source.set_actions(gtk4::gdk::DragAction::COPY);
        let id_str = doc.id.to_string();
        drag_source.connect_prepare(move |_, _, _| {
            Some(gtk4::gdk::ContentProvider::for_value(&id_str.to_value()))
        });
        row.add_controller(drag_source);

        // Right-click context menu
        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3);
        let this = self.clone();
        let doc_clone = doc.clone();
        let row_weak = row.downgrade();
        gesture.connect_pressed(move |g, _, x, y| {
            g.set_state(gtk4::EventSequenceState::Claimed);
            if let Some(row) = row_weak.upgrade() {
                this.show_doc_menu(&row, &doc_clone, x, y);
            }
        });
        row.add_controller(gesture);

        row
    }

    fn open_doc_by_id(&self, doc_id: i64) {
        let doc = self.library.borrow().doc_by_id(doc_id).ok().flatten();
        if let Some(doc) = doc {
            let path = doc.path.clone();
            self.library.borrow_mut().touch_opened(&path).ok();
            if let Some(cb) = self.on_open.borrow().as_ref() {
                cb(path);
            }
        }
    }

    fn show_doc_menu(
        &self,
        row: &ListBoxRow,
        doc: &crate::library::Document,
        x: f64,
        y: f64,
    ) {
        let popover = Popover::new();
        popover.set_parent(row);
        popover.set_has_arrow(true);
        popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

        let vbox = GtkBox::new(Orientation::Vertical, 2);
        vbox.set_margin_top(4);
        vbox.set_margin_bottom(4);
        vbox.set_margin_start(4);
        vbox.set_margin_end(4);

        let mk = |label: &str| -> Button {
            let b = Button::with_label(label);
            b.add_css_class("flat");
            b.set_halign(Align::Fill);
            if let Some(child) = b.child() {
                child.set_halign(Align::Start);
            }
            b
        };

        let open_b = mk("Open");
        {
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            open_b.connect_clicked(move |_| {
                pop.popdown();
                this.open_doc_by_id(id);
            });
        }
        vbox.append(&open_b);

        let rename_b = mk("Rename…");
        {
            let this = self.clone();
            let doc = doc.clone();
            let pop = popover.clone();
            rename_b.connect_clicked(move |_| {
                pop.popdown();
                this.rename_doc_dialog(&doc);
            });
        }
        vbox.append(&rename_b);

        let cat_b = mk("Set Category…");
        {
            let this = self.clone();
            let doc = doc.clone();
            let pop = popover.clone();
            cat_b.connect_clicked(move |_| {
                pop.popdown();
                this.set_category_dialog(&doc);
            });
        }
        vbox.append(&cat_b);

        let tags_b = mk("Edit Tags…");
        {
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            tags_b.connect_clicked(move |_| {
                pop.popdown();
                this.edit_tags_dialog(id);
            });
        }
        vbox.append(&tags_b);

        let project_b = mk("Add to Project…");
        {
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            project_b.connect_clicked(move |_| {
                pop.popdown();
                this.add_to_project_dialog(id);
            });
        }
        vbox.append(&project_b);

        let arch_label = if doc.archived { "Unarchive" } else { "Archive" };
        let arch_b = mk(arch_label);
        {
            let this = self.clone();
            let id = doc.id;
            let archived = doc.archived;
            let pop = popover.clone();
            arch_b.connect_clicked(move |_| {
                pop.popdown();
                this.library.borrow_mut().set_archived(id, !archived).ok();
                this.refresh();
            });
        }
        vbox.append(&arch_b);

        vbox.append(&Separator::new(Orientation::Horizontal));

        let remove_b = mk("Remove from Library");
        {
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            remove_b.connect_clicked(move |_| {
                pop.popdown();
                this.library.borrow_mut().remove_document(id).ok();
                this.refresh();
            });
        }
        vbox.append(&remove_b);

        let delete_b = mk("Delete File…");
        delete_b.add_css_class("error");
        {
            let this = self.clone();
            let doc = doc.clone();
            let pop = popover.clone();
            delete_b.connect_clicked(move |_| {
                pop.popdown();
                this.delete_file_dialog(&doc);
            });
        }
        vbox.append(&delete_b);

        popover.set_child(Some(&vbox));
        popover.popup();
    }

    fn rename_doc_dialog(&self, doc: &crate::library::Document) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Rename Document"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Rename");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_text(&doc.title);
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

    fn set_category_dialog(&self, doc: &crate::library::Document) {
        let cats = self.library.borrow().all_categories().unwrap_or_default();
        let body = if cats.is_empty() {
            None
        } else {
            Some(format!("Existing: {}", cats.join(", ")))
        };
        let dlg = adw::MessageDialog::new(
            Some(&self.window),
            Some("Set Category"),
            body.as_deref(),
        );
        dlg.add_response("clear", "Clear");
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Set");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Category name"));
        if let Some(cat) = &doc.category {
            entry.set_text(cat);
        }
        dlg.set_extra_child(Some(&entry));
        let this = self.clone();
        let id = doc.id;
        let entry_c = entry.clone();
        dlg.connect_response(None, move |_, resp| {
            match resp {
                "ok" => {
                    let cat = entry_c.text().to_string();
                    let cat = cat.trim();
                    let value = if cat.is_empty() { None } else { Some(cat) };
                    this.library.borrow_mut().set_category(id, value).ok();
                    this.refresh();
                }
                "clear" => {
                    this.library.borrow_mut().set_category(id, None).ok();
                    this.refresh();
                }
                _ => {}
            }
        });
        dlg.present();
    }

    fn edit_tags_dialog(&self, doc_id: i64) {
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
            btn.set_size_request(18, 18);
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
                if name.is_empty() { return; }
                if let Ok(new_id) = this.library.borrow_mut().create_tag(&name, &color.borrow()) {
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
                this.library.borrow_mut().set_doc_tags(doc_id, &selected).ok();
                this.populate_doc_list();
            }
        });
        dlg.present();
    }

    fn add_to_project_dialog(&self, doc_id: i64) {
        let projects = self.library.borrow().all_projects().unwrap_or_default();
        if projects.is_empty() {
            self.create_project_then_add(doc_id);
            return;
        }
        let dlg = adw::MessageDialog::new(
            Some(&self.window),
            Some("Add to Project"),
            None,
        );
        dlg.add_response("new", "New Project…");
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
            r.set_child(Some(&Label::builder().label(&p.name).halign(Align::Start).margin_top(6).margin_bottom(6).margin_start(8).build()));
            listbox.append(&r);
        }
        scroll.set_child(Some(&listbox));
        dlg.set_extra_child(Some(&scroll));

        let this = self.clone();
        let listbox_c = listbox.clone();
        dlg.connect_response(None, move |_, resp| {
            match resp {
                "new" => this.create_project_then_add(doc_id),
                "cancel" => {
                    if let Some(row) = listbox_c.selected_row() {
                        if let Ok(pid) = row.widget_name().to_string().parse::<i64>() {
                            this.library.borrow_mut().add_doc_to_project(pid, doc_id).ok();
                            this.refresh();
                        }
                    }
                }
                _ => {}
            }
        });
        // Activate on row click
        let this2 = self.clone();
        let dlg_weak = dlg.downgrade();
        listbox.connect_row_activated(move |_, row| {
            if let Ok(pid) = row.widget_name().to_string().parse::<i64>() {
                this2.library.borrow_mut().add_doc_to_project(pid, doc_id).ok();
                this2.refresh();
                if let Some(d) = dlg_weak.upgrade() {
                    d.close();
                }
            }
        });
        dlg.present();
    }

    fn create_project_then_add(&self, doc_id: i64) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("New Project"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Create");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Project name"));
        dlg.set_extra_child(Some(&entry));
        let this = self.clone();
        let entry_c = entry.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let name = entry_c.text().to_string();
                if !name.trim().is_empty() {
                    if let Ok(pid) = this.library.borrow_mut().create_project(name.trim()) {
                        this.library.borrow_mut().add_doc_to_project(pid, doc_id).ok();
                    }
                    this.refresh();
                }
            }
        });
        dlg.present();
    }

    fn create_project_dialog(&self) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("New Project"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Create");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Project name"));
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

    fn delete_file_dialog(&self, doc: &crate::library::Document) {
        let dlg = adw::MessageDialog::new(
            Some(&self.window),
            Some("Delete File?"),
            Some(&format!(
                "This permanently deletes {} from disk. This cannot be undone.",
                doc.path.display()
            )),
        );
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("delete", "Delete");
        dlg.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dlg.set_default_response(Some("cancel"));
        dlg.set_close_response("cancel");
        let this = self.clone();
        let doc = doc.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "delete" {
                std::fs::remove_file(&doc.path).ok();
                this.library.borrow_mut().remove_document(doc.id).ok();
                this.refresh();
            }
        });
        dlg.present();
    }

    fn show_manage_tags(&self) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Manage Tags"), None);
        dlg.add_response("close", "Close");
        dlg.set_close_response("close");

        let vbox = GtkBox::new(Orientation::Vertical, 8);
        vbox.set_width_request(320);

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(180);
        let tag_list = ListBox::new();
        tag_list.set_selection_mode(gtk4::SelectionMode::None);
        scroll.set_child(Some(&tag_list));
        vbox.append(&scroll);

        let new_row = GtkBox::new(Orientation::Horizontal, 6);
        let name_entry = Entry::new();
        name_entry.set_placeholder_text(Some("New tag name"));
        name_entry.set_hexpand(true);
        new_row.append(&name_entry);
        vbox.append(&new_row);

        let color_box = GtkBox::new(Orientation::Horizontal, 4);
        let selected_color: Rc<RefCell<String>> = Rc::new(RefCell::new(TAG_COLORS[0].to_string()));
        for color in TAG_COLORS {
            let btn = Button::new();
            btn.set_size_request(24, 24);
            apply_color_css(&btn, color);
            let sel = selected_color.clone();
            let c = color.to_string();
            btn.connect_clicked(move |_| {
                *sel.borrow_mut() = c.clone();
            });
            color_box.append(&btn);
        }
        vbox.append(&color_box);

        let add_btn = Button::with_label("Add Tag");
        add_btn.add_css_class("suggested-action");
        vbox.append(&add_btn);

        let this = self.clone();
        let tag_list_c = tag_list.clone();
        let refresh_tags = Rc::new(move || {
            while let Some(child) = tag_list_c.first_child() {
                tag_list_c.remove(&child);
            }
            let tags = this.library.borrow().all_tags().unwrap_or_default();
            for tag in tags {
                let r = ListBoxRow::new();
                r.set_selectable(false);
                let hbox = GtkBox::new(Orientation::Horizontal, 8);
                hbox.set_margin_top(4);
                hbox.set_margin_bottom(4);
                hbox.set_margin_start(8);
                hbox.set_margin_end(8);
                let dot = Label::new(Some("●"));
                dot.set_use_markup(true);
                dot.set_markup(&format!("<span foreground=\"{}\">●</span>", tag.color_hex));
                hbox.append(&dot);
                let name = Label::new(Some(&tag.name));
                name.set_halign(Align::Start);
                name.set_hexpand(true);
                hbox.append(&name);
                let del = Button::from_icon_name("user-trash-symbolic");
                del.add_css_class("flat");
                let this2 = this.clone();
                let tid = tag.id;
                del.connect_clicked(move |_| {
                    this2.library.borrow_mut().delete_tag(tid).ok();
                    this2.refresh();
                });
                hbox.append(&del);
                r.set_child(Some(&hbox));
                tag_list_c.append(&r);
            }
        });
        refresh_tags();

        {
            let this = self.clone();
            let name_entry = name_entry.clone();
            let sel = selected_color.clone();
            let refresh_tags = refresh_tags.clone();
            add_btn.connect_clicked(move |_| {
                let name = name_entry.text().to_string();
                if !name.trim().is_empty() {
                    this.library
                        .borrow_mut()
                        .create_tag(name.trim(), &sel.borrow())
                        .ok();
                    name_entry.set_text("");
                    refresh_tags();
                    this.populate_filter_list();
                }
            });
        }

        dlg.set_extra_child(Some(&vbox));
        let this = self.clone();
        dlg.connect_response(None, move |_, _| {
            this.refresh();
        });
        dlg.present();
    }

    fn new_document(&self) {
        let mut path = self.work_dir.join("Untitled.typ");
        let mut n = 2;
        while path.exists() {
            path = self.work_dir.join(format!("Untitled {n}.typ"));
            n += 1;
        }
        if std::fs::write(&path, b"").is_err() {
            tracing::warn!("Failed to create new document at {}", path.display());
            return;
        }
        self.library.borrow_mut().upsert_document(&path).ok();
        if let Some(cb) = self.on_open.borrow().as_ref() {
            cb(path);
        }
        self.refresh();
    }

    fn import_document(&self) {
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
        dialog.open(Some(&self.window), gtk4::gio::Cancellable::NONE, move |res| {
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
        });
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn toggle(&self) {
        if self.window.is_visible() {
            self.hide();
        } else {
            self.refresh();
            self.present();
        }
    }

    pub fn set_on_open<F: Fn(PathBuf) + 'static>(&self, f: F) {
        *self.on_open.borrow_mut() = Some(Box::new(f));
    }

    pub fn refresh(&self) {
        self.populate_filter_list();
        self.populate_doc_list();
    }

    #[allow(dead_code)]
    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }
}

fn parse_filter_name(name: &str) -> LibraryFilter {
    if name == "all" {
        LibraryFilter::All
    } else if name == "archive" {
        LibraryFilter::Archive
    } else if let Some(rest) = name.strip_prefix("project:") {
        rest.parse::<i64>()
            .map(LibraryFilter::Project)
            .unwrap_or(LibraryFilter::All)
    } else if let Some(rest) = name.strip_prefix("tag:") {
        rest.parse::<i64>()
            .map(LibraryFilter::Tag)
            .unwrap_or(LibraryFilter::All)
    } else if let Some(rest) = name.strip_prefix("category:") {
        LibraryFilter::Category(rest.to_string())
    } else {
        LibraryFilter::All
    }
}

fn make_filter_row(name: &str, icon: &str, label: &str, count: Option<i64>) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_widget_name(name);
    let hbox = GtkBox::new(Orientation::Horizontal, 8);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);
    hbox.set_margin_start(8);
    hbox.set_margin_end(8);
    let img = Image::from_icon_name(icon);
    img.set_pixel_size(16);
    hbox.append(&img);
    let lbl = Label::new(Some(label));
    lbl.set_hexpand(true);
    lbl.set_halign(Align::Start);
    lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    hbox.append(&lbl);
    if let Some(c) = count {
        let c_lbl = Label::new(Some(&c.to_string()));
        c_lbl.add_css_class("dim-label");
        c_lbl.add_css_class("caption");
        hbox.append(&c_lbl);
    }
    row.set_child(Some(&hbox));
    row
}

fn make_tag_filter_row(tag_id: i64, label: &str, color: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_widget_name(&format!("tag:{tag_id}"));
    let hbox = GtkBox::new(Orientation::Horizontal, 8);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);
    hbox.set_margin_start(8);
    hbox.set_margin_end(8);
    let dot = Label::new(None);
    dot.set_use_markup(true);
    dot.set_markup(&format!("<span foreground=\"{color}\">●</span>"));
    hbox.append(&dot);
    let lbl = Label::new(Some(label));
    lbl.set_hexpand(true);
    lbl.set_halign(Align::Start);
    lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    hbox.append(&lbl);
    row.set_child(Some(&hbox));
    row
}

fn header_row(text: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    if text.is_empty() {
        row.set_child(Some(&Separator::new(Orientation::Horizontal)));
    } else {
        let lbl = Label::new(Some(text));
        lbl.add_css_class("sidebar-header");
        lbl.set_halign(Align::Start);
        row.set_child(Some(&lbl));
    }
    row
}

fn format_date(iso: &str) -> String {
    use chrono::{DateTime, Datelike, Local};
    let dt = DateTime::parse_from_rfc3339(iso)
        .map(|d| d.with_timezone(&Local))
        .unwrap_or_else(|_| Local::now());
    let now = Local::now();
    let days_ago = (now.date_naive() - dt.date_naive()).num_days();
    if days_ago == 0 {
        "Today".to_string()
    } else if days_ago == 1 {
        "Yesterday".to_string()
    } else if days_ago < 7 {
        dt.format("%A").to_string()
    } else if dt.year() == now.year() {
        dt.format("%b %-d").to_string()
    } else {
        dt.format("%b %-d, %Y").to_string()
    }
}

fn apply_color_css(widget: &impl IsA<gtk4::Widget>, color: &str) {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(&format!(
        "button {{ background: {color}; border-radius: 4px; min-width: 16px; min-height: 16px; }}"
    ));
    widget
        .as_ref()
        .style_context()
        .add_provider(&provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

fn load_library_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        ".category-chip {
            background: alpha(@accent_color, 0.15);
            color: @accent_color;
            border-radius: 4px;
            padding: 1px 6px;
            font-size: 0.8em;
        }
        .tag-chip {
            background: alpha(@window_fg_color, 0.12);
            border-radius: 4px;
            padding: 1px 6px;
            font-size: 0.75em;
        }
        .sidebar-header {
            font-size: 0.75em;
            font-weight: bold;
            color: alpha(@window_fg_color, 0.55);
            padding: 8px 12px 2px 12px;
        }
        .doc-title {
            font-weight: 600;
        }",
    );
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

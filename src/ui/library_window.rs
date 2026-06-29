use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, DragSource, DropTarget, Entry, Image, Label,
    ListBox, ListBoxRow, Orientation, Popover, Revealer, ScrolledWindow, SearchEntry, Separator,
    TextView,
};
use libadwaita as adw;
use adw::prelude::*;

use crate::library::{Library, LibraryFilter, SortOrder};

const TAG_COLORS: &[&str] = &[
    "#3584e4", "#33d17a", "#f6d32d", "#ff7800", "#e01b24", "#9141ac", "#dc8add", "#986a44",
];

#[derive(Clone, Debug, PartialEq)]
enum ViewMode {
    List,
    Compact,
}

#[derive(Clone)]
pub struct LibraryWindow {
    window: adw::ApplicationWindow,
    library: Rc<RefCell<Library>>,
    doc_list: ListBox,
    filter_list: ListBox,
    search_entry: SearchEntry,
    current_filter: Rc<RefCell<LibraryFilter>>,
    current_sort: Rc<RefCell<SortOrder>>,
    selection: Rc<RefCell<HashSet<i64>>>,
    action_bar_revealer: Revealer,
    selected_count_label: Label,
    #[allow(dead_code)]
    toast_overlay: adw::ToastOverlay,
    on_open: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
    work_dir: PathBuf,
    view_mode: Rc<RefCell<ViewMode>>,
    stats_label: Label,
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

        let stats_label = Label::new(None);
        stats_label.add_css_class("dim-label");
        stats_label.add_css_class("caption");
        stats_label.set_margin_top(4);
        stats_label.set_margin_bottom(6);
        stats_label.set_margin_start(8);
        stats_label.set_margin_end(8);
        stats_label.set_wrap(true);
        stats_label.set_halign(Align::Start);
        sidebar_inner.append(&stats_label);

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
        let sort_dropdown =
            gtk4::DropDown::from_strings(&["Modified", "Created", "Opened", "A→Z"]);
        sort_dropdown.set_tooltip_text(Some("Sort order"));
        let view_btn = Button::from_icon_name("view-list-compact-symbolic");
        view_btn.set_tooltip_text(Some("Toggle compact view"));
        view_btn.add_css_class("flat");

        right_header.pack_end(&import_btn);
        right_header.pack_end(&new_doc_btn);
        right_header.pack_end(&sort_dropdown);
        right_header.pack_end(&view_btn);

        right.add_top_bar(&right_header);

        let doc_scroll = ScrolledWindow::new();
        doc_scroll.set_vexpand(true);
        let doc_list = ListBox::new();
        doc_list.set_selection_mode(gtk4::SelectionMode::None);
        doc_scroll.set_child(Some(&doc_list));
        right.set_content(Some(&doc_scroll));

        // ── Bulk-action bottom bar ──────────────────────────────────────────
        let action_bar_revealer = Revealer::new();
        action_bar_revealer.set_transition_type(gtk4::RevealerTransitionType::SlideUp);
        action_bar_revealer.set_reveal_child(false);

        let action_bar = GtkBox::new(Orientation::Horizontal, 8);
        action_bar.set_margin_top(8);
        action_bar.set_margin_bottom(8);
        action_bar.set_margin_start(12);
        action_bar.set_margin_end(12);

        let selected_count_label = Label::new(Some("0 selected"));
        selected_count_label.add_css_class("dim-label");
        action_bar.append(&selected_count_label);

        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        action_bar.append(&spacer);

        let bulk_archive_btn = Button::with_label("Archive");
        bulk_archive_btn.add_css_class("flat");
        action_bar.append(&bulk_archive_btn);

        let bulk_tag_btn = Button::with_label("Tag…");
        bulk_tag_btn.add_css_class("flat");
        action_bar.append(&bulk_tag_btn);

        let bulk_project_btn = Button::with_label("Add to Project…");
        bulk_project_btn.add_css_class("flat");
        action_bar.append(&bulk_project_btn);

        let bulk_remove_btn = Button::with_label("Remove");
        bulk_remove_btn.add_css_class("destructive-action");
        action_bar.append(&bulk_remove_btn);

        let clear_btn = Button::from_icon_name("window-close-symbolic");
        clear_btn.add_css_class("flat");
        action_bar.append(&clear_btn);

        action_bar_revealer.set_child(Some(&action_bar));
        right.add_bottom_bar(&action_bar_revealer);

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
            current_sort: Rc::new(RefCell::new(SortOrder::Modified)),
            selection: Rc::new(RefCell::new(HashSet::new())),
            action_bar_revealer,
            selected_count_label,
            toast_overlay,
            on_open: Rc::new(RefCell::new(None)),
            work_dir,
            view_mode: Rc::new(RefCell::new(ViewMode::List)),
            stats_label,
        };

        lw.populate_filter_list();
        lw.populate_doc_list();
        lw.wire_signals(
            &new_doc_btn,
            &import_btn,
            &manage_tags_btn,
            &new_project_btn,
            &sort_dropdown,
            &bulk_archive_btn,
            &bulk_tag_btn,
            &bulk_project_btn,
            &bulk_remove_btn,
            &clear_btn,
            &view_btn,
        );

        lw
    }

    #[allow(clippy::too_many_arguments)]
    fn wire_signals(
        &self,
        new_doc_btn: &Button,
        import_btn: &Button,
        manage_tags_btn: &Button,
        new_project_btn: &Button,
        sort_dropdown: &gtk4::DropDown,
        bulk_archive_btn: &Button,
        bulk_tag_btn: &Button,
        bulk_project_btn: &Button,
        bulk_remove_btn: &Button,
        clear_btn: &Button,
        view_btn: &Button,
    ) {
        {
            let this = self.clone();
            view_btn.connect_clicked(move |_| {
                {
                    let mut mode = this.view_mode.borrow_mut();
                    *mode = if *mode == ViewMode::List {
                        ViewMode::Compact
                    } else {
                        ViewMode::List
                    };
                }
                this.populate_doc_list();
            });
        }
        {
            let this = self.clone();
            self.filter_list.connect_row_selected(move |_, row| {
                if let Some(row) = row {
                    let name = row.widget_name().to_string();
                    let filter = parse_filter_name(&name);
                    *this.current_filter.borrow_mut() = filter;
                    this.selection.borrow_mut().clear();
                    this.update_action_bar();
                    this.populate_doc_list();
                }
            });
        }
        {
            let this = self.clone();
            sort_dropdown.connect_selected_notify(move |dd| {
                let sort = match dd.selected() {
                    1 => SortOrder::Created,
                    2 => SortOrder::Opened,
                    3 => SortOrder::Title,
                    _ => SortOrder::Modified,
                };
                *this.current_sort.borrow_mut() = sort;
                this.populate_doc_list();
            });
        }
        {
            let this = self.clone();
            clear_btn.connect_clicked(move |_| {
                this.selection.borrow_mut().clear();
                this.update_action_bar();
                this.populate_doc_list();
            });
        }
        {
            let this = self.clone();
            bulk_archive_btn.connect_clicked(move |_| {
                let ids: Vec<i64> = this.selection.borrow().iter().cloned().collect();
                for id in ids {
                    this.library.borrow_mut().set_archived(id, true).ok();
                }
                this.selection.borrow_mut().clear();
                this.update_action_bar();
                this.refresh();
            });
        }
        {
            let this = self.clone();
            bulk_remove_btn.connect_clicked(move |_| {
                let ids: Vec<i64> = this.selection.borrow().iter().cloned().collect();
                for id in ids {
                    this.library.borrow_mut().remove_document(id).ok();
                }
                this.selection.borrow_mut().clear();
                this.update_action_bar();
                this.refresh();
            });
        }
        {
            let this = self.clone();
            bulk_tag_btn.connect_clicked(move |_| {
                let ids: Vec<i64> = this.selection.borrow().iter().cloned().collect();
                if !ids.is_empty() {
                    this.bulk_tag_dialog(ids);
                }
            });
        }
        {
            let this = self.clone();
            bulk_project_btn.connect_clicked(move |_| {
                let ids: Vec<i64> = this.selection.borrow().iter().cloned().collect();
                if !ids.is_empty() {
                    this.bulk_add_to_project_dialog(ids);
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

        self.filter_list.append(&make_filter_row(
            "all",
            "document-open-recent-symbolic",
            "All Documents",
            self.library.borrow().doc_count(&LibraryFilter::All).ok(),
        ));
        self.filter_list.append(&make_filter_row(
            "recent",
            "document-open-recent-symbolic",
            "Recently Opened",
            self.library.borrow().doc_count(&LibraryFilter::Recent).ok(),
        ));
        self.filter_list.append(&make_filter_row(
            "untagged",
            "window-close-symbolic",
            "Untagged",
            self.library
                .borrow()
                .doc_count(&LibraryFilter::Untagged)
                .ok(),
        ));

        let projects = self
            .library
            .borrow()
            .all_projects()
            .unwrap_or_default();
        if !projects.is_empty() {
            self.filter_list.append(&header_row("PROJECTS"));
            for p in projects {
                let count = self
                    .library
                    .borrow()
                    .doc_count(&LibraryFilter::Project(p.id))
                    .ok();
                let filter_row = make_filter_row(
                    &format!("project:{}", p.id),
                    "folder-symbolic",
                    &p.name,
                    count,
                );
                let gesture = gtk4::GestureClick::new();
                gesture.set_button(3);
                let this = self.clone();
                let pid = p.id;
                let pname = p.name.clone();
                let row_weak = filter_row.downgrade();
                gesture.connect_pressed(move |g, _, x, y| {
                    g.set_state(gtk4::EventSequenceState::Claimed);
                    if let Some(row) = row_weak.upgrade() {
                        this.show_project_menu(&row, pid, &pname, x, y);
                    }
                });
                filter_row.add_controller(gesture);
                self.filter_list.append(&filter_row);
            }
        }

        let categories = self
            .library
            .borrow()
            .all_categories_with_colors()
            .unwrap_or_default();
        if !categories.is_empty() {
            self.filter_list.append(&header_row("CATEGORIES"));
            for (c, color) in categories {
                let cat_count = self
                    .library
                    .borrow()
                    .doc_count(&LibraryFilter::Category(c.clone()))
                    .ok();
                let filter_row = make_category_filter_row(
                    &format!("category:{}", c),
                    &color,
                    &c,
                    cat_count,
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
                let count = self
                    .library
                    .borrow()
                    .doc_count(&LibraryFilter::Tag(t.id))
                    .ok();
                self.filter_list
                    .append(&make_tag_filter_row(t.id, &t.name, &t.color_hex, count));
            }
        }

        self.filter_list.append(&header_row(""));
        self.filter_list.append(&make_filter_row(
            "trash",
            "user-trash-symbolic",
            "Trash",
            self.library.borrow().doc_count(&LibraryFilter::Trash).ok(),
        ));
        self.filter_list.append(&make_filter_row(
            "archive",
            "view-archive-symbolic",
            "Archive",
            self.library.borrow().doc_count(&LibraryFilter::Archive).ok(),
        ));

        if let Some(first) = self.filter_list.row_at_index(0) {
            self.filter_list.select_row(Some(&first));
        }

        let total = self
            .library
            .borrow()
            .doc_count(&LibraryFilter::All)
            .unwrap_or(0);
        let projects = self.library.borrow().all_projects().unwrap_or_default().len();
        let last = self
            .library
            .borrow()
            .documents(LibraryFilter::Recent, "", SortOrder::Opened)
            .unwrap_or_default()
            .into_iter()
            .next()
            .map(|d| d.title)
            .unwrap_or_else(|| "—".to_string());
        self.stats_label
            .set_text(&format!("{} docs · {} projects\nLast: {}", total, projects, last));
    }

    fn populate_doc_list(&self) {
        while let Some(child) = self.doc_list.first_child() {
            self.doc_list.remove(&child);
        }
        let search = self.search_entry.text().to_string();
        let filter = self.current_filter.borrow().clone();
        let sort = self.current_sort.borrow().clone();
        let project_reorder = match &filter {
            LibraryFilter::Project(pid) => Some(*pid),
            _ => None,
        };
        let docs = self
            .library
            .borrow()
            .documents(filter, &search, sort)
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

        let cat_colors: std::collections::HashMap<String, String> = self
            .library
            .borrow()
            .all_categories_with_colors()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let mode = self.view_mode.borrow().clone();

        for doc in docs {
            let tags = self.library.borrow().doc_tags(doc.id).unwrap_or_default();
            let row = self.make_doc_row(&doc, &tags, project_reorder, mode.clone(), &cat_colors);
            self.doc_list.append(&row);
        }
    }

    fn update_action_bar(&self) {
        let count = self.selection.borrow().len();
        if count == 0 {
            self.action_bar_revealer.set_reveal_child(false);
        } else {
            self.selected_count_label
                .set_text(&format!("{} selected", count));
            self.action_bar_revealer.set_reveal_child(true);
        }
    }

    fn make_doc_row(
        &self,
        doc: &crate::library::Document,
        tags: &[crate::library::Tag],
        project_reorder: Option<i64>,
        mode: ViewMode,
        cat_colors: &std::collections::HashMap<String, String>,
    ) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.set_widget_name(&doc.id.to_string());

        let hbox = if mode == ViewMode::Compact {
            let hbox = GtkBox::new(Orientation::Horizontal, 8);
            hbox.set_margin_top(4);
            hbox.set_margin_bottom(4);
            hbox.set_margin_start(4);
            hbox.set_margin_end(4);

            if doc.pinned {
                let pin = Image::from_icon_name("view-pin-symbolic");
                pin.set_pixel_size(12);
                hbox.append(&pin);
            }

            let title = Label::new(Some(&doc.title));
            title.add_css_class("doc-title");
            title.set_halign(Align::Start);
            title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            hbox.append(&title);

            if let Some(cat) = &doc.category {
                let sep = Label::new(Some("·"));
                sep.add_css_class("dim-label");
                hbox.append(&sep);
                let color = cat_colors.get(cat).map(|s| s.as_str()).unwrap_or("#3584e4");
                let chip = Label::new(Some(cat));
                chip.add_css_class("caption");
                apply_cat_color(&chip, color);
                hbox.append(&chip);
            }
            for tag in tags.iter().take(4) {
                let sep = Label::new(Some("·"));
                sep.add_css_class("dim-label");
                hbox.append(&sep);
                let chip = Label::new(Some(&tag.name));
                chip.add_css_class("tag-chip");
                chip.add_css_class("caption");
                hbox.append(&chip);
            }

            let spacer = GtkBox::new(Orientation::Horizontal, 0);
            spacer.set_hexpand(true);
            hbox.append(&spacer);

            if doc.archived {
                let badge = Label::new(Some("[archived]"));
                badge.add_css_class("dim-label");
                badge.add_css_class("caption");
                hbox.append(&badge);
            }
            let date = Label::new(Some(&format_date(&doc.modified_at)));
            date.add_css_class("dim-label");
            date.add_css_class("caption");
            date.set_halign(Align::End);
            hbox.append(&date);
            hbox
        } else {
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

            let title_box = GtkBox::new(Orientation::Horizontal, 6);
            if doc.pinned {
                let pin = Image::from_icon_name("view-pin-symbolic");
                pin.set_pixel_size(14);
                title_box.append(&pin);
            }
            let title = Label::new(Some(&doc.title));
            title.add_css_class("doc-title");
            title.set_halign(Align::Start);
            title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            title_box.append(&title);
            vbox.append(&title_box);

            let chips = GtkBox::new(Orientation::Horizontal, 4);
            if let Some(cat) = &doc.category {
                let color = cat_colors.get(cat).map(|s| s.as_str()).unwrap_or("#3584e4");
                let chip = Label::new(Some(cat));
                chip.add_css_class("caption");
                apply_cat_color(&chip, color);
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
            let file_size = std::fs::metadata(&doc.path).map(|m| m.len()).unwrap_or(0);
            if file_size > 0 && file_size <= 1_000_000 {
                let line_count = std::fs::read_to_string(&doc.path)
                    .map(|s| s.lines().count())
                    .unwrap_or(0);
                if line_count > 0 {
                    let lines_lbl = Label::new(Some(&format!("{} lines", line_count)));
                    lines_lbl.add_css_class("dim-label");
                    lines_lbl.add_css_class("caption");
                    lines_lbl.set_halign(Align::End);
                    meta.append(&lines_lbl);
                }
            }
            if doc.archived {
                let badge = Label::new(Some("[archived]"));
                badge.add_css_class("dim-label");
                badge.add_css_class("caption");
                badge.set_halign(Align::End);
                meta.append(&badge);
            }
            hbox.append(&meta);
            hbox
        };

        if doc.pinned {
            hbox.add_css_class("pinned-doc");
        }

        if self.selection.borrow().contains(&doc.id) {
            hbox.add_css_class("selected-doc");
        }

        row.set_child(Some(&hbox));

        // Drag source — carry doc ID as a string for drop-on-category
        let drag_source = DragSource::new();
        drag_source.set_actions(gtk4::gdk::DragAction::COPY);
        let id_str = doc.id.to_string();
        drag_source.connect_prepare(move |_, _, _| {
            Some(gtk4::gdk::ContentProvider::for_value(&id_str.to_value()))
        });
        row.add_controller(drag_source);

        if let Some(pid) = project_reorder {
            let drop = DropTarget::new(gtk4::glib::Type::STRING, gtk4::gdk::DragAction::COPY);
            let this = self.clone();
            let target_doc_id = doc.id;
            drop.connect_drop(move |_, value, _, _| {
                if let Ok(id_str) = value.get::<String>() {
                    if let Ok(dragged_id) = id_str.parse::<i64>() {
                        if dragged_id != target_doc_id {
                            if let Ok(Some(target_pos)) =
                                this.library.borrow().position_in_project(pid, target_doc_id)
                            {
                                this.library
                                    .borrow_mut()
                                    .move_doc_in_project(pid, dragged_id, target_pos)
                                    .ok();
                                this.populate_doc_list();
                            }
                        }
                    }
                }
                false
            });
            row.add_controller(drop);
        }

        // Ctrl+click multi-select
        let ctrl_click = gtk4::GestureClick::new();
        ctrl_click.set_button(1);
        let this = self.clone();
        let doc_id = doc.id;
        ctrl_click.connect_pressed(move |g, _, _, _| {
            let mods = g.current_event_state();
            if mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
                g.set_state(gtk4::EventSequenceState::Claimed);
                {
                    let mut sel = this.selection.borrow_mut();
                    if sel.contains(&doc_id) {
                        sel.remove(&doc_id);
                    } else {
                        sel.insert(doc_id);
                    }
                }
                this.update_action_bar();
                this.populate_doc_list();
            }
        });
        row.add_controller(ctrl_click);

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

        let is_trash = *self.current_filter.borrow() == LibraryFilter::Trash;
        if is_trash {
            let restore_b = mk("Restore");
            {
                let this = self.clone();
                let id = doc.id;
                let pop = popover.clone();
                restore_b.connect_clicked(move |_| {
                    pop.popdown();
                    this.library.borrow_mut().restore_from_trash(id).ok();
                    this.refresh();
                });
            }
            vbox.append(&restore_b);

            vbox.append(&Separator::new(Orientation::Horizontal));

            let del_b = mk("Permanently Delete…");
            del_b.add_css_class("error");
            {
                let this = self.clone();
                let doc = doc.clone();
                let pop = popover.clone();
                del_b.connect_clicked(move |_| {
                    pop.popdown();
                    this.permanent_delete_dialog(&doc);
                });
            }
            vbox.append(&del_b);

            popover.set_child(Some(&vbox));
            popover.popup();
            return;
        }

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

        let notes_b = mk("Edit Notes…");
        {
            let this = self.clone();
            let doc = doc.clone();
            let pop = popover.clone();
            notes_b.connect_clicked(move |_| {
                pop.popdown();
                this.edit_notes_dialog(&doc);
            });
        }
        vbox.append(&notes_b);

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

        let maybe_pid = match *self.current_filter.borrow() {
            LibraryFilter::Project(pid) => Some(pid),
            _ => None,
        };
        if let Some(pid) = maybe_pid {
            let root_b = mk("Set as Project Root");
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            root_b.connect_clicked(move |_| {
                pop.popdown();
                this.library.borrow_mut().set_project_root(pid, Some(id)).ok();
            });
            vbox.append(&root_b);
        }

        let pin_label = if doc.pinned { "Unpin" } else { "Pin to Top" };
        let pin_b = mk(pin_label);
        {
            let this = self.clone();
            let id = doc.id;
            let pinned = doc.pinned;
            let pop = popover.clone();
            pin_b.connect_clicked(move |_| {
                pop.popdown();
                this.library.borrow_mut().set_pinned(id, !pinned).ok();
                this.populate_doc_list();
            });
        }
        vbox.append(&pin_b);

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

        let trash_b = mk("Move to Trash");
        trash_b.add_css_class("error");
        {
            let this = self.clone();
            let id = doc.id;
            let pop = popover.clone();
            trash_b.connect_clicked(move |_| {
                pop.popdown();
                this.library.borrow_mut().move_to_trash(id).ok();
                this.refresh();
            });
        }
        vbox.append(&trash_b);

        popover.set_child(Some(&vbox));
        popover.popup();
    }

    fn edit_notes_dialog(&self, doc: &crate::library::Document) {
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

    fn show_project_menu(
        &self,
        row: &ListBoxRow,
        project_id: i64,
        project_name: &str,
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

        if let Ok(Some(root_path)) = self.library.borrow().project_root_path(project_id) {
            let open_root = mk("Open Root File");
            let this = self.clone();
            let pop = popover.clone();
            open_root.connect_clicked(move |_| {
                pop.popdown();
                if let Some(cb) = this.on_open.borrow().as_ref() {
                    this.library.borrow_mut().touch_opened(&root_path).ok();
                    cb(root_path.clone());
                }
            });
            vbox.append(&open_root);
            vbox.append(&Separator::new(Orientation::Horizontal));
        }

        let rename_b = mk("Rename Project…");
        {
            let this = self.clone();
            let pop = popover.clone();
            let pname = project_name.to_string();
            rename_b.connect_clicked(move |_| {
                pop.popdown();
                this.rename_project_dialog(project_id, &pname);
            });
        }
        vbox.append(&rename_b);

        let delete_b = mk("Delete Project");
        delete_b.add_css_class("error");
        {
            let this = self.clone();
            let pop = popover.clone();
            delete_b.connect_clicked(move |_| {
                pop.popdown();
                this.library.borrow_mut().delete_project(project_id).ok();
                *this.current_filter.borrow_mut() = LibraryFilter::All;
                this.refresh();
            });
        }
        vbox.append(&delete_b);

        popover.set_child(Some(&vbox));
        popover.popup();
    }

    fn rename_project_dialog(&self, project_id: i64, current_name: &str) {
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Rename Project"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Rename");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");
        let entry = Entry::new();
        entry.set_text(current_name);
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

    fn bulk_tag_dialog(&self, doc_ids: Vec<i64>) {
        let all_tags = self.library.borrow().all_tags().unwrap_or_default();
        let dlg = adw::MessageDialog::new(Some(&self.window), Some("Tag Documents"), None);
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Apply");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(160);
        scroll.set_width_request(300);
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
        dlg.set_extra_child(Some(&scroll));

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
                    this.library.borrow_mut().set_doc_tags(*doc_id, &selected).ok();
                }
                this.selection.borrow_mut().clear();
                this.update_action_bar();
                this.refresh();
            }
        });
        dlg.present();
    }

    fn bulk_add_to_project_dialog(&self, doc_ids: Vec<i64>) {
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
                    this.library.borrow_mut().add_doc_to_project(pid, *doc_id).ok();
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
        let container = GtkBox::new(Orientation::Vertical, 8);
        container.set_width_request(280);
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Category name"));
        if let Some(cat) = &doc.category {
            entry.set_text(cat);
        }
        container.append(&entry);

        let color_row = GtkBox::new(Orientation::Horizontal, 4);
        let initial_color = doc
            .category
            .as_ref()
            .map(|c| self.library.borrow().get_category_color(c))
            .unwrap_or_else(|| TAG_COLORS[0].to_string());
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
        container.append(&color_row);
        dlg.set_extra_child(Some(&container));

        let this = self.clone();
        let id = doc.id;
        let entry_c = entry.clone();
        let color_sel = selected_color.clone();
        dlg.connect_response(None, move |_, resp| {
            match resp {
                "ok" => {
                    let cat = entry_c.text().to_string();
                    let cat = cat.trim();
                    let value = if cat.is_empty() { None } else { Some(cat) };
                    this.library.borrow_mut().set_category(id, value).ok();
                    if let Some(name) = value {
                        let color = color_sel.borrow().clone();
                        this.library.borrow_mut().set_category_color(name, &color).ok();
                    }
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

        {
            let this = self.clone();
            let checks_c = checks.clone();
            let listbox_c = listbox.clone();
            let bib_doc_btn = Button::with_label("Import cited authors from BibTeX…");
            bib_doc_btn.add_css_class("flat");
            container.append(&bib_doc_btn);
            let rt_for_bib: Rc<dyn Fn()> = Rc::new(move || {
                let all_tags = this.library.borrow().all_tags().unwrap_or_default();
                let current_checks: Vec<i64> =
                    checks_c.borrow().iter().map(|(id, _)| *id).collect();
                for tag in &all_tags {
                    if !current_checks.contains(&tag.id) {
                        let check = CheckButton::with_label(&tag.name);
                        check.set_active(false);
                        let r = ListBoxRow::new();
                        r.set_selectable(false);
                        r.set_child(Some(&check));
                        listbox_c.append(&r);
                        checks_c.borrow_mut().push((tag.id, check));
                    }
                }
                this.populate_filter_list();
            });
            let this2 = self.clone();
            bib_doc_btn.connect_clicked(move |_| {
                let path = this2
                    .library
                    .borrow()
                    .doc_by_id(doc_id)
                    .ok()
                    .flatten()
                    .map(|d| d.path);
                let paths = path.into_iter().collect();
                this2.import_authors_from_bibtex(paths, rt_for_bib.clone());
            });
        }

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

    fn permanent_delete_dialog(&self, doc: &crate::library::Document) {
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
                this.library.borrow_mut().permanently_delete(id).ok();
                this.refresh();
            }
        });
        dlg.present();
    }

    fn show_manage_tags(&self) {
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

        let bib_btn = Button::with_label("Import authors from BibTeX…");
        bib_btn.add_css_class("flat");
        bib_btn.set_margin_top(0);
        vbox.append(&bib_btn);

        let this = self.clone();
        let tag_list_c = tag_list.clone();
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
                    let color_val = sel.borrow().clone();
                    this.library.borrow_mut().create_tag(name.trim(), &color_val).ok();
                    name_entry.set_text("");
                    refresh_tags();
                    this.populate_filter_list();
                }
            });
        }

        let doc_paths: Vec<PathBuf> = match *self.current_filter.borrow() {
            LibraryFilter::Project(pid) => self
                .library
                .borrow()
                .documents(LibraryFilter::Project(pid), "", SortOrder::Title)
                .unwrap_or_default()
                .into_iter()
                .map(|d| d.path)
                .collect(),
            _ => vec![],
        };

        {
            let this = self.clone();
            let refresh_tags = refresh_tags.clone();
            let doc_paths = doc_paths.clone();
            bib_btn.connect_clicked(move |_| {
                this.import_authors_from_bibtex(doc_paths.clone(), refresh_tags.clone());
            });
        }

        dlg.set_extra_child(Some(&vbox));
        let this = self.clone();
        dlg.connect_response(None, move |_, _| this.refresh());
        dlg.present();
    }

    fn import_authors_from_bibtex(&self, doc_paths: Vec<PathBuf>, refresh_tags: Rc<dyn Fn()>) {
        let dialog = gtk4::FileDialog::new();
        dialog.set_title("Select BibTeX File");
        let filter = gtk4::FileFilter::new();
        filter.add_pattern("*.bib");
        filter.set_name(Some("BibTeX files"));
        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&self.work_dir)));
        let this = self.clone();
        dialog.open(Some(&self.window), gtk4::gio::Cancellable::NONE, move |res| {
            if let Ok(file) = res {
                if let Some(path) = file.path() {
                    let mut keys = std::collections::HashSet::new();
                    for dp in &doc_paths {
                        keys.extend(extract_cite_keys(dp));
                    }
                    let authors = if keys.is_empty() {
                        parse_bibtex_authors_for_keys(&path, None)
                    } else {
                        parse_bibtex_authors_for_keys(&path, Some(&keys))
                    };
                    if !authors.is_empty() {
                        this.show_author_selection_dialog(authors, refresh_tags.clone());
                    }
                }
            }
        });
    }

    fn show_author_selection_dialog(&self, authors: Vec<String>, refresh_tags: Rc<dyn Fn()>) {
        let dlg = adw::MessageDialog::new(
            Some(&self.window),
            Some("Import Author Tags"),
            Some("Select authors to create as tags:"),
        );
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("ok", "Create Tags");
        dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dlg.set_default_response(Some("ok"));
        dlg.set_close_response("cancel");

        let scroll = ScrolledWindow::new();
        scroll.set_min_content_height(180);
        scroll.set_width_request(280);
        let listbox = ListBox::new();
        listbox.set_selection_mode(gtk4::SelectionMode::None);

        let checks: Vec<(String, CheckButton)> = authors
            .into_iter()
            .map(|name| {
                let check = CheckButton::with_label(&name);
                check.set_active(true);
                let r = ListBoxRow::new();
                r.set_selectable(false);
                r.set_child(Some(&check));
                listbox.append(&r);
                (name, check)
            })
            .collect();

        scroll.set_child(Some(&listbox));
        dlg.set_extra_child(Some(&scroll));

        let this = self.clone();
        dlg.connect_response(None, move |_, resp| {
            if resp == "ok" {
                for (name, check) in &checks {
                    if check.is_active() {
                        let color = TAG_COLORS[1].to_string(); // green for authors
                        this.library.borrow_mut().create_tag(name, &color).ok();
                    }
                }
                refresh_tags();
                this.populate_filter_list();
            }
        });
        dlg.present();
    }

    fn new_document(&self) {
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

        if templates.is_empty() {
            self.create_new_from_template(None);
            return;
        }

        let dlg = adw::MessageDialog::new(Some(&self.window), Some("New Document"), None);
        dlg.add_response("blank", "Blank Document");
        dlg.set_default_response(Some("blank"));
        dlg.set_close_response("blank");

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

    fn create_new_from_template(&self, template: Option<&std::path::Path>) {
        let mut path = self.work_dir.join("Untitled.typ");
        let mut n = 2;
        while path.exists() {
            path = self.work_dir.join(format!("Untitled {n}.typ"));
            n += 1;
        }
        let content = template
            .and_then(|t| std::fs::read(t).ok())
            .unwrap_or_default();
        if std::fs::write(&path, &content).is_err() {
            tracing::warn!("Failed to create document at {}", path.display());
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
    } else if name == "recent" {
        LibraryFilter::Recent
    } else if name == "archive" {
        LibraryFilter::Archive
    } else if name == "untagged" {
        LibraryFilter::Untagged
    } else if name == "trash" {
        LibraryFilter::Trash
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

fn make_category_filter_row(name: &str, color: &str, label: &str, count: Option<i64>) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_widget_name(name);
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
    if let Some(c) = count {
        let c_lbl = Label::new(Some(&c.to_string()));
        c_lbl.add_css_class("dim-label");
        c_lbl.add_css_class("caption");
        hbox.append(&c_lbl);
    }
    row.set_child(Some(&hbox));
    row
}

fn make_tag_filter_row(tag_id: i64, label: &str, color: &str, count: Option<i64>) -> ListBoxRow {
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
    if let Some(c) = count {
        let c_lbl = Label::new(Some(&c.to_string()));
        c_lbl.add_css_class("dim-label");
        c_lbl.add_css_class("caption");
        hbox.append(&c_lbl);
    }
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

fn apply_cat_color(widget: &impl IsA<gtk4::Widget>, bg_hex: &str) {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(&format!(
        "* {{ background: {bg_hex}22; color: {bg_hex}; border-radius: 4px; padding: 1px 6px; }}"
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
        }
        .selected-doc {
            background: alpha(@accent_color, 0.1);
            border-radius: 4px;
        }
        .pinned-doc {
            border-left: 2px solid @accent_color;
            padding-left: 6px;
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

fn extract_cite_keys(typ_path: &std::path::Path) -> std::collections::HashSet<String> {
    let content = match std::fs::read_to_string(typ_path) {
        Ok(c) => c,
        Err(_) => return std::collections::HashSet::new(),
    };
    let mut keys = std::collections::HashSet::new();
    for cap in regex::Regex::new(r"@([a-zA-Z][a-zA-Z0-9_:.-]*)")
        .unwrap()
        .captures_iter(&content)
    {
        keys.insert(cap[1].to_string());
    }
    for cap in regex::Regex::new(r"#cite\(<([^>]+)>\)")
        .unwrap()
        .captures_iter(&content)
    {
        keys.insert(cap[1].to_string());
    }
    for cap in regex::Regex::new(r#"#cite\("([^"]+)"\)"#)
        .unwrap()
        .captures_iter(&content)
    {
        keys.insert(cap[1].to_string());
    }
    keys
}

fn parse_bibtex_authors_for_keys(
    bib_path: &std::path::Path,
    filter_keys: Option<&std::collections::HashSet<String>>,
) -> Vec<String> {
    let content = match std::fs::read_to_string(bib_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut seen = std::collections::HashSet::new();
    let mut in_matching_entry = filter_keys.is_none();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('@')
            && !trimmed.to_lowercase().starts_with("@string")
            && trimmed.contains('{')
        {
            let after = &trimmed[trimmed.find('{').unwrap() + 1..];
            let key = after.split(',').next().unwrap_or("").trim().to_string();
            in_matching_entry = filter_keys.map_or(true, |keys| keys.contains(&key));
        }
        if !in_matching_entry {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if lower.starts_with("author") {
            if let Some(eq) = trimmed.find('=') {
                let value = trimmed[eq + 1..]
                    .trim()
                    .trim_start_matches('{')
                    .trim_end_matches('}')
                    .trim_start_matches('"')
                    .trim_end_matches('"')
                    .trim_end_matches(',')
                    .trim();
                for part in value.split_terminator(" and ") {
                    let name = part.trim();
                    if name.is_empty() {
                        continue;
                    }
                    let tag_name = if let Some(c) = name.find(',') {
                        name[..c].trim().to_string()
                    } else {
                        name.split_whitespace().last().unwrap_or(name).to_string()
                    };
                    if !tag_name.is_empty() {
                        seen.insert(tag_name);
                    }
                }
            }
        }
    }
    let mut result: Vec<String> = seen.into_iter().collect();
    result.sort();
    result
}

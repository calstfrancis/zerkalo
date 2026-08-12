use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, DragSource, DropTarget, Entry, Image, Label,
    ListBox, ListBoxRow, Orientation, Popover, Revealer, ScrolledWindow, SearchEntry, Separator,
    Stack, TextView,
};
use libadwaita as adw;
use adw::prelude::*;

use crate::library::{Library, LibraryFilter, SortOrder};

const TAG_COLORS: &[&str] = &[
    "#3584e4", "#33d17a", "#f6d32d", "#ff7800", "#e01b24", "#9141ac", "#dc8add", "#986a44",
];

/// Deterministic palette color for a category/tag name that has never had one
/// explicitly assigned, so distinct uncolored categories still look distinct
/// instead of all silently defaulting to the same blue.
fn stable_palette_color(name: &str) -> &'static str {
    let hash = name.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    TAG_COLORS[hash as usize % TAG_COLORS.len()]
}

#[derive(Clone, Debug, PartialEq)]
enum ViewMode {
    List,
    Compact,
}

#[derive(Clone)]
pub struct LibraryWindow {
    window: adw::Window,
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
    bottom_filter_list: ListBox,
    doc_list_stack: Stack,
    empty_page: adw::StatusPage,
}

impl LibraryWindow {
    pub fn new(_app: &adw::Application, library: Rc<RefCell<Library>>, work_dir: PathBuf) -> Self {
        let window = adw::Window::new();
        window.set_title(Some("Library — Zerkalo"));
        window.set_default_width(900);
        window.set_default_height(650);

        let toast_overlay = adw::ToastOverlay::new();

        let root = GtkBox::new(Orientation::Horizontal, 0);

        // ── Left sidebar ────────────────────────────────────────────────────
        let sidebar = GtkBox::new(Orientation::Vertical, 0);
        sidebar.set_width_request(220);
        sidebar.add_css_class("fond-sidebar");

        let sidebar_header = adw::HeaderBar::new();
        sidebar_header.add_css_class("fond-chrome");
        sidebar_header.add_css_class("flat");
        sidebar_header.set_show_start_title_buttons(false);
        sidebar_header.set_show_end_title_buttons(false);
        let sidebar_title = adw::WindowTitle::new("Library", "");
        sidebar_header.set_title_widget(Some(&sidebar_title));
        sidebar.append(&sidebar_header);

        let sidebar_scroll = ScrolledWindow::new();
        sidebar_scroll.set_vexpand(true);
        sidebar_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);

        let sidebar_inner = GtkBox::new(Orientation::Vertical, 0);

        let filter_list = ListBox::new();
        // The suite's list rather than navigation-sidebar: selection is a wash
        // across the row, not an accent fill, so the sidebar stays quiet while
        // still saying which filter is current.
        filter_list.add_css_class("fond-list");
        filter_list.set_selection_mode(gtk4::SelectionMode::Single);
        sidebar_inner.append(&filter_list);

        let stats_label = Label::new(None);
        stats_label.add_css_class("fond-row-meta");

        sidebar_scroll.set_child(Some(&sidebar_inner));
        sidebar.append(&sidebar_scroll);

        // ── Fixed bottom section (always visible, outside scroll) ────────────
        sidebar.append(&Separator::new(Orientation::Horizontal));

        let bottom_filter_list = ListBox::new();
        bottom_filter_list.add_css_class("fond-list");
        bottom_filter_list.set_selection_mode(gtk4::SelectionMode::Single);
        sidebar.append(&bottom_filter_list);

        sidebar.append(&Separator::new(Orientation::Horizontal));

        let manage_box = GtkBox::new(Orientation::Vertical, 0);
        manage_box.set_margin_top(4);
        manage_box.set_margin_bottom(8);
        manage_box.set_margin_start(8);
        manage_box.set_margin_end(8);
        let new_project_btn = Button::with_label("New Project");
        new_project_btn.add_css_class("flat");
        new_project_btn.add_css_class("fond-quiet");
        manage_box.append(&new_project_btn);
        let new_cat_btn = Button::with_label("New Category");
        new_cat_btn.add_css_class("flat");
        new_cat_btn.add_css_class("fond-quiet");
        manage_box.append(&new_cat_btn);
        let manage_tags_btn = Button::with_label("Manage Tags");
        manage_tags_btn.add_css_class("flat");
        manage_tags_btn.add_css_class("fond-quiet");
        manage_box.append(&manage_tags_btn);
        sidebar.append(&manage_box);

        root.append(&sidebar);
        root.append(&Separator::new(Orientation::Vertical));

        // ── Right area ──────────────────────────────────────────────────────
        let right = adw::ToolbarView::new();
        right.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
        right.set_hexpand(true);

        let right_header = adw::HeaderBar::new();
        right_header.add_css_class("fond-chrome");
        right_header.set_show_title(false);

        let search_entry = SearchEntry::new();
        search_entry.set_placeholder_text(Some("Search documents…"));
        search_entry.set_width_request(240);
        let start_box = GtkBox::new(Orientation::Horizontal, 6);
        start_box.append(&search_entry);
        right_header.pack_start(&start_box);

        // One bordered control in the header, the way the main window has one.
        // A filled suggested-action button next to a filled sort dropdown made
        // the top of the window the loudest thing in it.
        let new_doc_btn = Button::with_label("New Document");
        new_doc_btn.add_css_class("fond-pill");
        new_doc_btn.set_valign(Align::Center);
        let import_btn = Button::with_label("Import…");
        import_btn.add_css_class("flat");
        import_btn.add_css_class("fond-quiet");
        let sort_dropdown =
            gtk4::DropDown::from_strings(&["Modified", "Created", "Opened", "A→Z"]);
        sort_dropdown.set_tooltip_text(Some("Sort order"));
        right_header.pack_end(&import_btn);
        right_header.pack_end(&new_doc_btn);
        right_header.pack_end(&sort_dropdown);

        right.add_top_bar(&right_header);

        let doc_scroll = ScrolledWindow::new();
        doc_scroll.set_vexpand(true);
        doc_scroll.add_css_class("fond-ground");
        let doc_list = ListBox::new();
        doc_list.set_selection_mode(gtk4::SelectionMode::None);
        doc_list.add_css_class("fond-list");
        doc_list.set_margin_start(12);
        doc_list.set_margin_end(12);
        doc_list.set_margin_bottom(8);
        doc_scroll.set_child(Some(&doc_list));

        let empty_page = adw::StatusPage::new();
        empty_page.set_icon_name(Some("folder-open-symbolic"));
        empty_page.set_title("No documents");
        empty_page.set_description(Some("Nothing here yet"));
        empty_page.set_vexpand(true);

        let doc_list_stack = Stack::new();
        doc_list_stack.set_vexpand(true);
        doc_list_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        doc_list_stack.add_named(&doc_scroll, Some("docs"));
        doc_list_stack.add_named(&empty_page, Some("empty"));
        right.set_content(Some(&doc_list_stack));

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
        clear_btn.set_tooltip_text(Some("Clear selection"));
        clear_btn.update_property(&[gtk4::accessible::Property::Label("Clear selection")]);
        action_bar.append(&clear_btn);

        action_bar_revealer.set_child(Some(&action_bar));
        right.add_bottom_bar(&action_bar_revealer);

        // ── Library status bar ─────────────────────────────────────────────
        let lib_status_bar = GtkBox::new(Orientation::Horizontal, 8);
        lib_status_bar.add_css_class("fond-chrome");
        lib_status_bar.add_css_class("fond-statusbar");
        lib_status_bar.set_margin_start(12);
        lib_status_bar.set_margin_end(8);
        lib_status_bar.append(&stats_label);
        stats_label.set_hexpand(true);
        stats_label.set_halign(Align::Start);
        // A status-bar toggle whose label is its own name, bold when on —
        // the same control the editor's status bar uses.
        let compact_btn = Button::with_label("compact");
        compact_btn.add_css_class("flat");
        lib_status_bar.append(&compact_btn);
        right.add_bottom_bar(&lib_status_bar);

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
            bottom_filter_list,
            doc_list_stack,
            empty_page,
        };

        lw.populate_filter_list();
        lw.populate_doc_list();
        lw.wire_signals(
            &new_doc_btn,
            &import_btn,
            &manage_tags_btn,
            &new_project_btn,
            &new_cat_btn,
            &sort_dropdown,
            &bulk_archive_btn,
            &bulk_tag_btn,
            &bulk_project_btn,
            &bulk_remove_btn,
            &clear_btn,
            &compact_btn,
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
        new_cat_btn: &Button,
        sort_dropdown: &gtk4::DropDown,
        bulk_archive_btn: &Button,
        bulk_tag_btn: &Button,
        bulk_project_btn: &Button,
        bulk_remove_btn: &Button,
        clear_btn: &Button,
        compact_btn: &Button,
    ) {
        {
            let this = self.clone();
            let compact_btn_c = compact_btn.clone();
            compact_btn.connect_clicked(move |_| {
                {
                    let mut mode = this.view_mode.borrow_mut();
                    *mode = if *mode == ViewMode::List {
                        ViewMode::Compact
                    } else {
                        ViewMode::List
                    };
                }
                if *this.view_mode.borrow() == ViewMode::Compact {
                    compact_btn_c.add_css_class("fond-toggle-active");
                } else {
                    compact_btn_c.remove_css_class("fond-toggle-active");
                }
                this.populate_doc_list();
            });
        }
        let inhibit: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let inhibit_b = inhibit.clone();
        {
            let this = self.clone();
            let inhibit = inhibit.clone();
            self.filter_list.connect_row_selected(move |_, row| {
                if *inhibit.borrow() { return; }
                if let Some(row) = row {
                    *inhibit.borrow_mut() = true;
                    this.bottom_filter_list.unselect_all();
                    *inhibit.borrow_mut() = false;
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
            self.bottom_filter_list.connect_row_selected(move |_, row| {
                if *inhibit_b.borrow() { return; }
                if let Some(row) = row {
                    *inhibit_b.borrow_mut() = true;
                    this.filter_list.unselect_all();
                    *inhibit_b.borrow_mut() = false;
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
        {
            let this = self.clone();
            new_cat_btn.connect_clicked(move |_| this.create_category_dialog());
        }
    }

    fn populate_filter_list(&self) {
        while let Some(child) = self.filter_list.first_child() {
            self.filter_list.remove(&child);
        }

        self.filter_list.append(&make_filter_row(
            "all",
            "view-list-symbolic",
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
            "edit-clear-symbolic",
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

        let all_cats = self
            .library
            .borrow()
            .all_categories_structured()
            .unwrap_or_default();
        if !all_cats.is_empty() {
            // Partition into parents (have children), children (have parent), standalone
            let parent_names: std::collections::HashSet<String> = all_cats
                .iter()
                .filter_map(|c| c.parent.clone())
                .collect();
            let cats_with_children: std::collections::HashSet<String> = all_cats
                .iter()
                .filter(|c| parent_names.contains(&c.name))
                .map(|c| c.name.clone())
                .collect();

            self.filter_list.append(&header_row("CATEGORIES"));

            // Emit parent rows first, then their children, then standalones
            let mut emitted: std::collections::HashSet<String> = std::collections::HashSet::new();
            for cat in &all_cats {
                if cat.parent.is_none() && cats_with_children.contains(&cat.name) {
                    // Parent category row
                    let has_children = true;
                    let cat_count = self
                        .library
                        .borrow()
                        .doc_count(&LibraryFilter::CategoryGroup(cat.name.clone()))
                        .ok();
                    let filter_row = make_category_filter_row(
                        &format!("category-group:{}", cat.name),
                        &cat.color_hex.clone().unwrap_or_else(|| stable_palette_color(&cat.name).to_string()),
                        &cat.name,
                        cat_count,
                    );
                    // Parent rows: drop rejected with toast
                    let drop = DropTarget::new(gtk4::glib::Type::STRING, gtk4::gdk::DragAction::COPY);
                    let toast_overlay = self.toast_overlay.clone();
                    drop.connect_drop(move |_, _, _, _| {
                        let toast = adw::Toast::new("Drop onto a specific subcategory");
                        toast_overlay.add_toast(toast);
                        false
                    });
                    filter_row.add_controller(drop);
                    let gesture = gtk4::GestureClick::new();
                    gesture.set_button(3);
                    let this = self.clone();
                    let cat_name = cat.name.clone();
                    let row_weak = filter_row.downgrade();
                    gesture.connect_pressed(move |g, _, x, y| {
                        g.set_state(gtk4::EventSequenceState::Claimed);
                        if let Some(row) = row_weak.upgrade() {
                            this.show_category_menu(&row, &cat_name, has_children, x, y);
                        }
                    });
                    filter_row.add_controller(gesture);
                    self.filter_list.append(&filter_row);
                    emitted.insert(cat.name.clone());

                    // Children of this parent
                    for child in &all_cats {
                        if child.parent.as_deref() == Some(&cat.name) {
                            let child_count = self
                                .library
                                .borrow()
                                .doc_count(&LibraryFilter::Category(child.name.clone()))
                                .ok();
                            let child_row = make_category_filter_row_indented(
                                &format!("category:{}", child.name),
                                &child.color_hex.clone().unwrap_or_else(|| stable_palette_color(&child.name).to_string()),
                                &child.name,
                                child_count,
                                16,
                            );
                            let drop2 = DropTarget::new(gtk4::glib::Type::STRING, gtk4::gdk::DragAction::COPY);
                            let this2 = self.clone();
                            let cname2 = child.name.clone();
                            drop2.connect_drop(move |_, value, _, _| {
                                if let Ok(id_str) = value.get::<String>() {
                                    if let Ok(doc_id) = id_str.parse::<i64>() {
                                        this2.library.borrow_mut().set_category(doc_id, Some(&cname2)).ok();
                                        this2.refresh();
                                        return true;
                                    }
                                }
                                false
                            });
                            child_row.add_controller(drop2);
                            let gesture2 = gtk4::GestureClick::new();
                            gesture2.set_button(3);
                            let this2 = self.clone();
                            let cname2 = child.name.clone();
                            let row_weak2 = child_row.downgrade();
                            gesture2.connect_pressed(move |g, _, x, y| {
                                g.set_state(gtk4::EventSequenceState::Claimed);
                                if let Some(row) = row_weak2.upgrade() {
                                    this2.show_category_menu(&row, &cname2, false, x, y);
                                }
                            });
                            child_row.add_controller(gesture2);
                            self.filter_list.append(&child_row);
                            emitted.insert(child.name.clone());
                        }
                    }
                }
            }
            // Standalone categories (no parent, no children)
            for cat in &all_cats {
                if emitted.contains(&cat.name) { continue; }
                let cat_count = self
                    .library
                    .borrow()
                    .doc_count(&LibraryFilter::Category(cat.name.clone()))
                    .ok();
                let filter_row = make_category_filter_row(
                    &format!("category:{}", cat.name),
                    &cat.color_hex.clone().unwrap_or_else(|| stable_palette_color(&cat.name).to_string()),
                    &cat.name,
                    cat_count,
                );
                let drop = DropTarget::new(gtk4::glib::Type::STRING, gtk4::gdk::DragAction::COPY);
                let this = self.clone();
                let cat_name = cat.name.clone();
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
                let gesture = gtk4::GestureClick::new();
                gesture.set_button(3);
                let this = self.clone();
                let cat_name = cat.name.clone();
                let row_weak = filter_row.downgrade();
                gesture.connect_pressed(move |g, _, x, y| {
                    g.set_state(gtk4::EventSequenceState::Claimed);
                    if let Some(row) = row_weak.upgrade() {
                        this.show_category_menu(&row, &cat_name, false, x, y);
                    }
                });
                filter_row.add_controller(gesture);
                self.filter_list.append(&filter_row);
            }
        }

        let tags_with_counts = self.library.borrow().all_tags_with_counts().unwrap_or_default();
        if !tags_with_counts.is_empty() {
            self.filter_list.append(&header_row("TAGS"));
            for (t, _) in tags_with_counts.iter() {
                let count = self.library.borrow().doc_count(&LibraryFilter::Tag(t.id)).ok();
                self.filter_list.append(&make_tag_filter_row(t.id, &t.name, &t.color_hex, count));
            }
        }

        // Repopulate the fixed bottom list (Trash / Archive)
        while let Some(child) = self.bottom_filter_list.first_child() {
            self.bottom_filter_list.remove(&child);
        }
        self.bottom_filter_list.append(&make_filter_row(
            "trash",
            "user-trash-symbolic",
            "Trash",
            self.library.borrow().doc_count(&LibraryFilter::Trash).ok(),
        ));
        self.bottom_filter_list.append(&make_filter_row(
            "archive",
            "view-archive-symbolic",
            "Archive",
            self.library.borrow().doc_count(&LibraryFilter::Archive).ok(),
        ));

        // Restore selection to match current_filter
        let current = self.current_filter.borrow().clone();
        let is_bottom = matches!(current, LibraryFilter::Trash | LibraryFilter::Archive);
        if is_bottom {
            // Select the matching bottom row
            let idx = if matches!(current, LibraryFilter::Trash) { 0 } else { 1 };
            if let Some(row) = self.bottom_filter_list.row_at_index(idx) {
                self.bottom_filter_list.select_row(Some(&row));
            }
        } else if let Some(first) = self.filter_list.row_at_index(0) {
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
            .set_text(&format!("{} docs · {} projects · Last: {}", total, projects, last));
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
            self.empty_page.set_description(Some(if !search.is_empty() {
                "Try a different search"
            } else {
                "Nothing here yet"
            }));
            self.doc_list_stack.set_visible_child_name("empty");
            return;
        }
        self.doc_list_stack.set_visible_child_name("docs");

        let cat_colors: HashMap<String, String> = self
            .library
            .borrow()
            .all_categories_with_colors()
            .unwrap_or_default()
            .into_iter()
            .map(|(name, color)| {
                let color = color.unwrap_or_else(|| stable_palette_color(&name).to_string());
                (name, color)
            })
            .collect();
        let mode = self.view_mode.borrow().clone();

        if mode == ViewMode::Compact {
            self.doc_list.add_css_class("compact-mode");
        } else {
            self.doc_list.remove_css_class("compact-mode");
        }

        // Pinned documents are a section of their own, announced the way every
        // other section in the suite is — a dot, a small-caps title and a count.
        // A bare separator between the two groups said less and looked like a
        // gap rather than a heading.
        let (pinned, rest): (Vec<_>, Vec<_>) = docs.into_iter().partition(|d| d.pinned);
        let groups: [(&str, &str, Vec<crate::library::Document>); 2] = [
            ("Pinned", "fond-accent-pinned", pinned),
            ("Documents", "fond-accent-library", rest),
        ];

        for (title, accent, group) in groups {
            if group.is_empty() {
                continue;
            }
            self.doc_list.append(&section_row(title, accent, group.len()));
            let last_idx = group.len() - 1;
            for (i, doc) in group.into_iter().enumerate() {
                let tags = self.library.borrow().doc_tags(doc.id).unwrap_or_default();
                let row = self.make_doc_row(&doc, &tags, project_reorder, mode.clone(), &cat_colors);
                if i == 0 {
                    row.add_css_class("fond-card-first");
                }
                if i == last_idx {
                    row.add_css_class("fond-card-last");
                }
                self.doc_list.append(&row);
            }
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
        cat_colors: &HashMap<String, String>,
    ) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.set_widget_name(&doc.id.to_string());
        row.add_css_class("fond-card");
        row.add_css_class("fond-row");

        // One line per document: a cue in the category's colour, the title, the
        // category and tags as dim reference text, and the date and length at
        // the right edge. What was here before was a three-line card with a
        // 32px file icon, four coloured chips and a line of notes — the titles
        // are what a library is scanned for, and they were the smallest thing
        // on the row. Notes are the tooltip now; the tags stay clickable.
        //
        // Compact mode is the same row under a `.compact-mode` list (fond.css
        // tightens the metrics), not a second row built by hand.
        let hbox = GtkBox::new(Orientation::Horizontal, 8);
        hbox.set_margin_start(10);
        hbox.set_margin_end(10);

        if doc.pinned {
            let pin = Image::from_icon_name("view-pin-symbolic");
            pin.set_pixel_size(12);
            pin.add_css_class("fond-row-meta");
            hbox.append(&pin);
        }

        let cue_color = doc.category.as_ref().map(|cat| {
            cat_colors
                .get(cat)
                .map(|s| s.to_string())
                .unwrap_or_else(|| stable_palette_color(cat).to_string())
        });
        hbox.append(&crate::ui::styles::fond_cue(cue_color.as_deref()));

        let title = Label::new(Some(&doc.title));
        title.add_css_class("fond-row-title");
        title.set_halign(Align::Start);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        hbox.append(&title);

        if let Some(cat) = &doc.category {
            let cat_lbl = Label::new(Some(cat));
            cat_lbl.add_css_class("fond-row-detail");
            hbox.append(&cat_lbl);
        }
        for tag in tags.iter().take(4) {
            let chip = Label::new(Some(&tag.name));
            chip.add_css_class("fond-row-detail");
            chip.set_tooltip_text(Some(&format!("Show only {}", tag.name)));
            hbox.append(&chip);
            let chip_click = gtk4::GestureClick::new();
            chip_click.set_button(1);
            let this_chip = self.clone();
            let tag_id = tag.id;
            let chip_ref = chip.clone();
            chip_click.connect_pressed(move |g, _, _, _| {
                g.set_state(gtk4::EventSequenceState::Claimed);
                chip_ref.add_css_class("chip-active");
                let chip_weak = chip_ref.downgrade();
                glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                    if let Some(c) = chip_weak.upgrade() { c.remove_css_class("chip-active"); }
                });
                *this_chip.current_filter.borrow_mut() = LibraryFilter::Tag(tag_id);
                this_chip.populate_doc_list();
                let tag_name = format!("tag:{}", tag_id);
                let mut i = 0;
                while let Some(row) = this_chip.filter_list.row_at_index(i) {
                    if row.widget_name().as_str() == tag_name {
                        this_chip.filter_list.select_row(Some(&row));
                        return;
                    }
                    i += 1;
                }
                let mut j = 0;
                while let Some(row) = this_chip.bottom_filter_list.row_at_index(j) {
                    if row.widget_name().as_str() == tag_name {
                        this_chip.bottom_filter_list.select_row(Some(&row));
                        return;
                    }
                    j += 1;
                }
            });
            chip.add_controller(chip_click);
        }

        if let Some(notes) = &doc.notes {
            if !notes.trim().is_empty() {
                hbox.set_tooltip_text(Some(notes.trim()));
            }
        }

        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        hbox.append(&spacer);

        if doc.archived {
            let badge = Label::new(Some("archived"));
            badge.add_css_class("fond-row-meta");
            hbox.append(&badge);
        }

        // The word count reads every file in the list, so it is worth having
        // only where there is room to read it.
        let mut meta_text = format_date(&doc.modified_at);
        if mode != ViewMode::Compact {
            let word_count = count_prose_words(std::path::Path::new(&doc.path));
            if word_count > 0 {
                meta_text = format!("{} \u{b7} {} words", meta_text, word_count);
            }
        }
        let meta = Label::new(Some(&meta_text));
        meta.add_css_class("fond-row-meta");
        meta.set_halign(Align::End);
        hbox.append(&meta);

        if self.selection.borrow().contains(&doc.id) {
            row.add_css_class("doc-selected");
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

        let export_b = mk("Export…");
        {
            let this = self.clone();
            let doc = doc.clone();
            let pop = popover.clone();
            export_b.connect_clicked(move |_| {
                pop.popdown();
                this.export_doc_dialog(&doc);
            });
        }
        vbox.append(&export_b);

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

    fn show_category_menu(&self, row: &ListBoxRow, cat_name: &str, has_children: bool, x: f64, y: f64) {
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
        let add_sub_b = mk("Add Subcategory…");
        {
            let this = self.clone();
            let pop = popover.clone();
            let cname = cat_name.to_string();
            add_sub_b.connect_clicked(move |_| {
                pop.popdown();
                this.add_subcategory_dialog(&cname);
            });
        }
        vbox.append(&add_sub_b);
        if !has_children {
            let set_parent_b = mk("Set Parent…");
            let this = self.clone();
            let pop = popover.clone();
            let cname = cat_name.to_string();
            set_parent_b.connect_clicked(move |_| {
                pop.popdown();
                this.set_parent_dialog(&cname);
            });
            vbox.append(&set_parent_b);
        }
        let rename_b = mk("Rename Category…");
        {
            let this = self.clone();
            let pop = popover.clone();
            let cname = cat_name.to_string();
            rename_b.connect_clicked(move |_| {
                pop.popdown();
                this.rename_category_dialog(&cname);
            });
        }
        vbox.append(&rename_b);
        let delete_b = mk("Delete Category");
        delete_b.add_css_class("error");
        if has_children {
            delete_b.set_sensitive(false);
            delete_b.add_css_class("dim-label");
            delete_b.set_tooltip_text(Some("Remove subcategories first"));
        } else {
            let this = self.clone();
            let pop = popover.clone();
            let cname = cat_name.to_string();
            delete_b.connect_clicked(move |_| {
                pop.popdown();
                let deleted = this.library.borrow_mut().force_delete_category_if_no_children(&cname).unwrap_or(false);
                if !deleted {
                    let toast = adw::Toast::new("Cannot delete: subcategories exist");
                    this.toast_overlay.add_toast(toast);
                }
                *this.current_filter.borrow_mut() = LibraryFilter::All;
                this.refresh();
            });
        }
        vbox.append(&delete_b);
        popover.set_child(Some(&vbox));
        popover.popup();
    }

    fn rename_category_dialog(&self, current_name: &str) {
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
                    this.library.borrow_mut().rename_category(&old_name, new_name.trim()).ok();
                    *this.current_filter.borrow_mut() = LibraryFilter::All;
                    this.refresh();
                }
            }
        });
        dlg.present();
    }

    fn add_subcategory_dialog(&self, parent_name: &str) {
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
                    this.library.borrow_mut().create_category(&trimmed, Some(&parent_for_dialog)).ok();
                    this.refresh();
                }
            }
        });
        dlg.present();
    }

    fn set_parent_dialog(&self, cat_name: &str) {
        let all_cats = self.library.borrow().all_categories_structured().unwrap_or_default();
        // Top-level categories with no parent and no children of their own (avoid cycles, keep max 2 levels)
        let parent_names: std::collections::HashSet<String> = all_cats
            .iter()
            .filter_map(|c| c.parent.clone())
            .collect();
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
                this.library.borrow_mut().set_category_parent(&cat, parent_value.as_deref()).ok();
                this.refresh();
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
                    this.library.borrow_mut().add_doc_tags(*doc_id, &selected).ok();
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
            .and_then(|c| {
                self.library
                    .borrow()
                    .get_category_color(c)
                    .or_else(|| Some(stable_palette_color(c).to_string()))
            })
            .unwrap_or_else(|| TAG_COLORS[0].to_string());
        let selected_color: Rc<RefCell<String>> = Rc::new(RefCell::new(initial_color));
        // Only persisted if the user actually clicks a swatch. Saving the
        // pre-filled colour regardless would pin every category to whatever it
        // happened to be showing, which is what made the palette fallback
        // pointless in the first place.
        let color_picked = Rc::new(std::cell::Cell::new(false));
        for color in TAG_COLORS {
            let btn = Button::new();
            btn.set_size_request(20, 20);
            apply_color_css(&btn, color);
            let sel = selected_color.clone();
            let picked = color_picked.clone();
            let c = color.to_string();
            btn.connect_clicked(move |_| {
                *sel.borrow_mut() = c.clone();
                picked.set(true);
            });
            color_row.append(&btn);
        }
        container.append(&color_row);
        dlg.set_extra_child(Some(&container));

        let this = self.clone();
        let id = doc.id;
        let entry_c = entry.clone();
        let color_sel = selected_color.clone();
        let color_picked_c = color_picked.clone();
        dlg.connect_response(None, move |_, resp| {
            match resp {
                "ok" => {
                    let cat = entry_c.text().to_string();
                    let cat = cat.trim();
                    let value = if cat.is_empty() { None } else { Some(cat) };
                    this.library.borrow_mut().set_category(id, value).ok();
                    if let (Some(name), true) = (value, color_picked_c.get()) {
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
                        // any tag missing from current_checks was just created by the
                        // BibTeX author import above, so it applies to this document
                        let check = CheckButton::with_label(&tag.name);
                        check.set_active(true);
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
                "ok" => {
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

    fn create_category_dialog(&self) {
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
                    this.library.borrow_mut().set_category_color(&name, &color).ok();
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
                edit.update_property(&[gtk4::accessible::Property::Label(&format!("Rename tag {}", tag.name))]);
                let this_e = this.clone();
                let tid_e = tag.id;
                let tag_name_e = tag.name.clone();
                let rs_e = rs_outer.clone();
                edit.connect_clicked(move |_| {
                    let entry = Entry::new();
                    entry.set_text(&tag_name_e);
                    entry.set_activates_default(true);
                    let rename_dlg = adw::MessageDialog::new(
                        Some(&this_e.window),
                        Some("Rename Tag"),
                        None,
                    );
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
                                this_r.library.borrow_mut().rename_tag(tid_e, new_name.trim()).ok();
                                this_r.populate_filter_list();
                                if let Some(f) = rs_r.borrow().as_ref() { f(); }
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
                del.update_property(&[gtk4::accessible::Property::Label(&format!("Delete tag {}", tag.name))]);
                let this2 = this.clone();
                let tid = tag.id;
                let rs_d = rs_outer.clone();
                del.connect_clicked(move |_| {
                    this2.library.borrow_mut().delete_tag(tid).ok();
                    this2.refresh();
                    if let Some(f) = rs_d.borrow().as_ref() { f(); }
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

    fn export_doc_dialog(&self, doc: &crate::library::Document) {
        let dialog = gtk4::FileDialog::new();
        dialog.set_title("Export PDF");
        let stem = doc.path
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
        dialog.save(Some(&self.window), gtk4::gio::Cancellable::NONE, move |res| {
            let dest = match res.ok().and_then(|f| f.path()) { Some(p) => p, None => return };
            let dest = if dest.extension().is_none() { dest.with_extension("pdf") } else { dest };

            // CV mode gap (see skrizhal/plan.md Phase 3a): LibraryWindow has no
            // Config reference and this can export any document in the
            // library, not just the active one, so there's no
            // effective_cv_elements to resolve here yet — a CV-mode
            // document exported this way (rather than via the main Export
            // dialog, which is covered) won't resolve #cv-entry/#cv-section.
            let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Vec<u8>, String>>(1);
            let src_for_thread = src.clone();
            std::thread::spawn(move || {
                let result = crate::compiler::compile_to_pdf_bytes(
                    &src_for_thread,
                    &std::collections::HashMap::new(),
                    &std::collections::HashMap::new(),
                ).map_err(|e| e.to_string());
                let _ = tx.send(result);
            });

            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                use std::sync::mpsc::TryRecvError;
                match rx.try_recv() {
                    Ok(Ok(bytes)) => {
                        if let Err(e) = std::fs::write(&dest, &bytes) {
                            show_export_error(&window, &e.to_string());
                        }
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        show_export_error(&window, &e);
                        glib::ControlFlow::Break
                    }
                    Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                }
            });
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
    pub fn window(&self) -> &adw::Window {
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
    } else if let Some(rest) = name.strip_prefix("category-group:") {
        LibraryFilter::CategoryGroup(rest.to_string())
    } else if let Some(rest) = name.strip_prefix("category:") {
        LibraryFilter::Category(rest.to_string())
    } else {
        LibraryFilter::All
    }
}

/// The shell every sidebar filter row shares: the suite's single-line row, a
/// cue or an icon at the left, the name, and a plain count at the right. The
/// count used to be a filled pill, which made a sidebar of quiet names read as
/// a column of badges.
fn filter_row_shell(name: &str, label: &str, count: Option<i64>) -> (ListBoxRow, GtkBox) {
    let row = ListBoxRow::new();
    row.set_widget_name(name);
    row.add_css_class("fond-row");
    let hbox = GtkBox::new(Orientation::Horizontal, 8);
    hbox.set_margin_start(10);
    hbox.set_margin_end(10);
    let lbl = Label::new(Some(label));
    lbl.set_hexpand(true);
    lbl.set_halign(Align::Start);
    lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    lbl.add_css_class("fond-row-title");
    hbox.append(&lbl);
    if let Some(c) = count {
        let c_lbl = Label::new(Some(&c.to_string()));
        c_lbl.add_css_class("fond-row-meta");
        c_lbl.set_halign(Align::End);
        c_lbl.set_visible(c > 0);
        hbox.append(&c_lbl);
    }
    row.set_child(Some(&hbox));
    (row, hbox)
}

fn make_filter_row(name: &str, icon: &str, label: &str, count: Option<i64>) -> ListBoxRow {
    let (row, hbox) = filter_row_shell(name, label, count);
    let img = Image::from_icon_name(icon);
    img.set_pixel_size(14);
    img.add_css_class("fond-quiet");
    hbox.prepend(&img);
    row
}

fn make_category_filter_row(name: &str, color: &str, label: &str, count: Option<i64>) -> ListBoxRow {
    let (row, hbox) = filter_row_shell(name, label, count);
    hbox.prepend(&crate::ui::styles::fond_cue(Some(color)));
    row
}

fn make_category_filter_row_indented(name: &str, color: &str, label: &str, count: Option<i64>, indent: i32) -> ListBoxRow {
    let row = make_category_filter_row(name, color, label, count);
    if let Some(child) = row.child() {
        child.set_margin_start(indent);
    }
    row
}

fn make_tag_filter_row(tag_id: i64, label: &str, color: &str, count: Option<i64>) -> ListBoxRow {
    let (row, hbox) = filter_row_shell(&format!("tag:{tag_id}"), label, count);
    hbox.prepend(&crate::ui::styles::fond_cue(Some(color)));
    row
}

fn header_row(text: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    if text.is_empty() {
        row.set_child(Some(&Separator::new(Orientation::Horizontal)));
    } else {
        // Title case, because the shared section style letterspaces and
        // uppercases it in CSS — passing SHOUTING text through would come out
        // spaced twice and read as a different typeface from every other
        // section in the suite.
        let title = title_case(text);
        let accent = if title == "Tags" { "fond-accent-pinned" } else { "fond-accent-library" };
        row.set_child(Some(&crate::ui::styles::fond_section_header(&title, accent)));
    }
    row
}

fn title_case(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => lower,
    }
}

/// A section header inside the document list: the suite's dot-and-small-caps
/// header, plus the number of documents under it.
fn section_row(title: &str, accent: &str, count: usize) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    let bx = crate::ui::styles::fond_section_header(title, accent);
    let meta = crate::ui::styles::fond_section_meta();
    meta.set_text(&format!("\u{b7} {count}"));
    bx.append(&meta);
    row.set_child(Some(&bx));
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

// GTK 4.10 deprecated per-widget CSS providers in favour of a display-wide
// provider plus a CSS class. These two set a colour that is only known at
// runtime (a tag's own hex), which the class-based approach cannot express
// without generating a class per colour — left as-is deliberately.
#[allow(deprecated)]
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

fn extract_cite_keys(typ_path: &std::path::Path) -> std::collections::HashSet<String> {
    // Compiled once, not three times per file — this runs for every document in
    // the library on a scan.
    static SHORTHAND: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static CITE_LABEL: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static CITE_STRING: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let content = match std::fs::read_to_string(typ_path) {
        Ok(c) => c,
        Err(_) => return std::collections::HashSet::new(),
    };
    let mut keys = std::collections::HashSet::new();
    let patterns = [
        SHORTHAND.get_or_init(|| regex::Regex::new(r"@([a-zA-Z][a-zA-Z0-9_:.-]*)").unwrap()),
        CITE_LABEL.get_or_init(|| regex::Regex::new(r"#cite\(<([^>]+)>\)").unwrap()),
        CITE_STRING.get_or_init(|| regex::Regex::new(r#"#cite\("([^"]+)"\)"#).unwrap()),
    ];
    for re in patterns {
        for cap in re.captures_iter(&content) {
            keys.insert(cap[1].to_string());
        }
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
            in_matching_entry = filter_keys.is_none_or(|keys| keys.contains(&key));
        }
        if !in_matching_entry {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if lower.starts_with("author") && lower[6..].trim_start().starts_with('=') {
            if let Some(eq) = trimmed.find('=') {
                let raw = trimmed[eq + 1..].trim();
                let value = raw
                    .trim_start_matches(['{', '"'])
                    .trim_end_matches([',', '}', '"'])
                    .trim();
                for part in value.split(" and ") {
                    if let Some(tag) = extract_author_tag(part) {
                        seen.insert(tag);
                    }
                }
            }
        }
    }
    let mut result: Vec<String> = seen.into_iter().collect();
    result.sort();
    result
}

fn extract_author_tag(raw: &str) -> Option<String> {
    let name = raw
        .trim()
        .trim_start_matches('{')
        .trim_end_matches(['}', ','])
        .trim();
    if name.is_empty() {
        return None;
    }
    let lower = name.to_lowercase();
    // BibLaTeX extended format: "family=Doe, given=John, ..."
    if lower.contains("family=") {
        let family = name.split(',')
            .find(|p| p.trim().to_lowercase().starts_with("family="))
            .map(|p| p.trim()[7..].trim().trim_matches(|c: char| c == '{' || c == '}').trim())
            .unwrap_or("");
        let given = name.split(',')
            .find(|p| p.trim().to_lowercase().starts_with("given="))
            .map(|p| p.trim()[6..].trim().trim_matches(|c: char| c == '{' || c == '}').trim())
            .unwrap_or("");
        if !family.is_empty() {
            return Some(if given.is_empty() {
                family.to_string()
            } else {
                format!("{family}, {given}")
            });
        }
    }
    // BibTeX comma format: "Last, First [Middle]"
    if let Some(comma) = name.find(',') {
        let last = name[..comma]
            .trim()
            .trim_matches(|c: char| c == '{' || c == '}')
            .trim();
        let first = name[comma + 1..]
            .trim()
            .trim_matches(|c: char| c == '{' || c == '}')
            .trim();
        if !last.is_empty() {
            return Some(if first.is_empty() {
                last.to_string()
            } else {
                format!("{last}, {first}")
            });
        }
    }
    // "First [Middle] Last" — last word is surname
    let parts: Vec<&str> = name.split_whitespace().collect();
    if parts.len() >= 2 {
        let last = *parts.last().unwrap();
        let first = parts[..parts.len() - 1].join(" ");
        return Some(format!("{last}, {first}"));
    }
    Some(name.to_string())
}

fn count_prose_words(path: &std::path::Path) -> usize {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let mut in_code_block = false;
    let mut total = 0usize;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block || t.starts_with("//") || t.starts_with('#') {
            continue;
        }
        for word in t.split_whitespace() {
            if !word.starts_with('@') && !word.starts_with('<') && !word.starts_with('`') {
                total += 1;
            }
        }
    }
    total
}

fn show_export_error(parent: &adw::Window, msg: &str) {
    let dlg = adw::MessageDialog::new(Some(parent), Some("Export Failed"), Some(msg));
    dlg.add_response("ok", "OK");
    dlg.present();
}

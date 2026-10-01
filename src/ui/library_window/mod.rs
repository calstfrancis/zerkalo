use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, DragSource, DropTarget, Entry, Image, Label,
    ListBox, ListBoxRow, Orientation, Popover, Revealer, ScrolledWindow, SearchEntry, Separator,
    Stack, TextView,
};
use libadwaita as adw;

use crate::config::Config;
use crate::library::{Library, LibraryFilter, SortOrder};

mod dialogs;
mod files;
mod menus;
mod rows;
mod sidebar;

use sidebar::*;

const TAG_COLORS: &[&str] = &[
    "#3584e4", "#33d17a", "#f6d32d", "#ff7800", "#e01b24", "#9141ac", "#dc8add", "#986a44",
];

/// Deterministic palette color for a category/tag name that has never had one
/// explicitly assigned, so distinct uncolored categories still look distinct
/// instead of all silently defaulting to the same blue.
fn stable_palette_color(name: &str) -> &'static str {
    let hash = name
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
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
    toast_overlay: adw::ToastOverlay,
    on_open: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
    /// Whether a path is currently open in the editor — used to refuse
    /// "Move into Zerkalo Folder…" on a document with an open tab, since
    /// nothing here retargets that tab's in-memory path.
    is_open: Rc<RefCell<Option<Box<dyn Fn(&Path) -> bool>>>>,
    work_dir: PathBuf,
    config: Rc<RefCell<Config>>,
    view_mode: Rc<RefCell<ViewMode>>,
    stats_label: Label,
    bottom_filter_list: ListBox,
    doc_list_stack: Stack,
    empty_page: adw::StatusPage,
    empty_new_doc_btn: Button,
    empty_clear_search_btn: Button,
    /// Held while the sidebar's own selection is being restored, so putting the
    /// highlight back on the current view doesn't read as the user choosing it.
    inhibit_select: Rc<RefCell<bool>>,
    authors_expanded: Rc<RefCell<bool>>,
}

impl LibraryWindow {
    pub fn new(
        _app: &adw::Application,
        library: Rc<RefCell<Library>>,
        work_dir: PathBuf,
        config: Rc<RefCell<Config>>,
    ) -> Self {
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
        // One search per pause in typing, not one per keystroke — each search
        // rebuilds the whole list.
        search_entry.set_search_delay(150);
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
            gtk4::DropDown::from_strings(&["Last edited", "Date created", "Last opened", "Name"]);
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

        // One of these two is shown at a time, matching whichever empty-state
        // message populate_doc_list picked — a bare "Nothing here yet"/"Try a
        // different search" with no button was a dead end, since "New
        // Document" already sits right above in the header.
        let empty_new_doc_btn = Button::with_label("New Document");
        empty_new_doc_btn.add_css_class("fond-pill");
        empty_new_doc_btn.set_halign(Align::Center);
        let empty_clear_search_btn = Button::with_label("Clear Search");
        empty_clear_search_btn.add_css_class("flat");
        empty_clear_search_btn.set_halign(Align::Center);
        empty_clear_search_btn.set_visible(false);
        let empty_actions = GtkBox::new(Orientation::Vertical, 0);
        empty_actions.append(&empty_new_doc_btn);
        empty_actions.append(&empty_clear_search_btn);
        empty_page.set_child(Some(&empty_actions));

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

        let bulk_category_btn = Button::with_label("Categorize…");
        bulk_category_btn.add_css_class("flat");
        action_bar.append(&bulk_category_btn);

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

        // F1 labels everything on screen, same as the main editor window —
        // Library's Project/Category/Tag/Archive/Trash sidebar has no other
        // in-app explanation anywhere.
        let help_overlay = super::help_overlay::HelpOverlay::new(&toast_overlay);
        help_overlay.annotate(
            &filter_list,
            "Filters",
            "All Documents, plus any Projects, Categories, and Tags you've made, and the Authors your documents cite. Click one to show only those documents.",
        );
        help_overlay.annotate(
            &bottom_filter_list,
            "Trash & Archive",
            "Trash holds deleted documents until you empty it. Archive holds documents you're done with but want to keep.",
        );
        help_overlay.annotate(
            &manage_box,
            "Organize",
            "Make a new Project or Category, or rename/recolor your Tags.",
        );
        help_overlay.annotate(
            &search_entry,
            "Search",
            "Filters the document list below as you type.",
        );
        help_overlay.annotate(
            &new_doc_btn,
            "New Document",
            "Starts a new document from a template.",
        );
        window.set_content(Some(help_overlay.widget()));

        {
            let overlay_for_key = help_overlay.clone();
            let controller = gtk4::EventControllerKey::new();
            controller.connect_key_pressed(move |_, key, _, _| {
                if key == gtk4::gdk::Key::F1 {
                    overlay_for_key.toggle();
                    return glib::Propagation::Stop;
                }
                if key == gtk4::gdk::Key::Escape && overlay_for_key.is_shown() {
                    overlay_for_key.hide();
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            window.add_controller(controller);
        }

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
            is_open: Rc::new(RefCell::new(None)),
            work_dir,
            config,
            view_mode: Rc::new(RefCell::new(ViewMode::List)),
            stats_label,
            bottom_filter_list,
            doc_list_stack,
            empty_page,
            empty_new_doc_btn,
            empty_clear_search_btn,
            inhibit_select: Rc::new(RefCell::new(false)),
            authors_expanded: Rc::new(RefCell::new(false)),
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
            &bulk_category_btn,
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
        bulk_category_btn: &Button,
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
        let inhibit = self.inhibit_select.clone();
        let inhibit_b = inhibit.clone();
        {
            let this = self.clone();
            let inhibit = inhibit.clone();
            self.filter_list.connect_row_selected(move |_, row| {
                if *inhibit.borrow() {
                    return;
                }
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
                if *inhibit_b.borrow() {
                    return;
                }
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
                for id in &ids {
                    this.library.borrow_mut().set_archived(*id, true).ok();
                }
                this.selection.borrow_mut().clear();
                this.update_action_bar();
                this.refresh();
                let undo = this.clone();
                let message = if ids.len() == 1 {
                    "Archived 1 document".to_string()
                } else {
                    format!("Archived {} documents", ids.len())
                };
                this.toast_with_undo(&message, move || {
                    for id in &ids {
                        undo.library.borrow_mut().set_archived(*id, false).ok();
                    }
                    undo.refresh();
                });
            });
        }
        {
            let this = self.clone();
            let win = self.window.clone();
            bulk_remove_btn.connect_clicked(move |_| {
                let ids: Vec<i64> = this.selection.borrow().iter().cloned().collect();
                if ids.is_empty() {
                    return;
                }
                let this2 = this.clone();
                let count = ids.len();
                let body = if count == 1 {
                    "The document's file on disk isn't touched — this only removes it from \
                     this list."
                        .to_string()
                } else {
                    format!(
                        "The {count} documents' files on disk aren't touched — this only \
                         removes them from this list."
                    )
                };
                super::confirm::confirm_destructive(
                    Some(win.upcast_ref()),
                    "Remove from list?",
                    &body,
                    "Remove",
                    move || {
                        for id in &ids {
                            this2.library.borrow_mut().remove_document(*id).ok();
                        }
                        this2.selection.borrow_mut().clear();
                        this2.update_action_bar();
                        this2.refresh();
                    },
                );
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
            bulk_category_btn.connect_clicked(move |_| {
                let ids: Vec<i64> = this.selection.borrow().iter().cloned().collect();
                if !ids.is_empty() {
                    this.bulk_category_dialog(ids);
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
            self.empty_new_doc_btn
                .connect_clicked(move |_| this.new_document());
        }
        {
            let this = self.clone();
            self.empty_clear_search_btn.connect_clicked(move |_| {
                this.search_entry.set_text("");
            });
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

    /// A toast with an Undo button — for actions that are easy to take back, in
    /// place of a confirmation dialog asking first.
    fn toast_with_undo(&self, message: &str, undo: impl Fn() + 'static) {
        let toast = adw::Toast::new(message);
        toast.set_use_markup(false);
        toast.set_button_label(Some("Undo"));
        toast.set_timeout(6);
        toast.connect_button_clicked(move |_| undo());
        self.toast_overlay.add_toast(toast);
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

    pub fn set_is_open<F: Fn(&Path) -> bool + 'static>(&self, f: F) {
        *self.is_open.borrow_mut() = Some(Box::new(f));
    }

    pub fn refresh(&self) {
        // The bibliography Settings points at decides which documents cite
        // whom; if it has changed since the library last looked, re-derive.
        let bib = crate::authors::configured_bibliography(
            &self.work_dir,
            self.config.borrow().bib_path.as_deref(),
        );
        let changed = self.library.borrow_mut().set_bibliography(bib);
        if changed {
            self.library.borrow_mut().resync_authors();
        }
        self.populate_filter_list();
        self.populate_doc_list();
    }

    pub fn window(&self) -> &adw::Window {
        &self.window
    }
}

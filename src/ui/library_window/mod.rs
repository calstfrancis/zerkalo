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
mod organize;
mod rows;
mod sidebar;

use menus::popup_menu_at;
use rows::RowParts;
use sidebar::*;

const TAG_COLORS: &[&str] = &[
    "#3584e4", "#33d17a", "#f6d32d", "#ff7800", "#e01b24", "#9141ac", "#dc8add", "#986a44",
];

/// A label's colour: the one chosen for it, or a palette colour taken from its
/// name so labels nobody has coloured still look distinct.
fn label_color(label: &crate::library::Label) -> String {
    label
        .color_hex
        .clone()
        .unwrap_or_else(|| stable_palette_color(&label.name).to_string())
}

/// Deterministic palette color for a label name that has never had one
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
    bottom_filter_list: ListBox,
    doc_list_stack: Stack,
    empty_page: adw::StatusPage,
    empty_new_doc_btn: Button,
    empty_import_btn: Button,
    empty_clear_search_btn: Button,
    /// Whether a search is held to the view it was typed in, rather than looking
    /// through every document. Reset whenever the search box is emptied.
    search_scoped: Rc<RefCell<bool>>,
    /// The strip above the list saying what a search is looking through, shown
    /// only while it isn't simply everything.
    scope_bar: GtkBox,
    scope_label: Label,
    scope_btn: Button,
    /// Held while the sidebar's own selection is being restored, so putting the
    /// highlight back on the current view doesn't read as the user choosing it.
    inhibit_select: Rc<RefCell<bool>>,
    authors_expanded: Rc<RefCell<bool>>,
    /// The controls of every row now listed, so selecting can update them in
    /// place instead of rebuilding the list.
    row_widgets: Rc<RefCell<HashMap<i64, RowParts>>>,
    /// Document ids in the order the list shows them, for Shift+click ranges.
    list_order: Rc<RefCell<Vec<i64>>>,
    /// The document last clicked, where a Shift+click range starts.
    anchor: Rc<RefCell<Option<i64>>>,
    /// Held while a checkbox is being set from the selection, so that doesn't
    /// read as the user ticking it.
    syncing_checks: Rc<std::cell::Cell<bool>>,
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
        let prefs = config.borrow().library.clone();
        window.set_default_width(prefs.width.max(640));
        window.set_default_height(prefs.height.max(420));

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

        sidebar_scroll.set_child(Some(&sidebar_inner));
        sidebar.append(&sidebar_scroll);

        // ── Fixed bottom section (always visible, outside scroll) ────────────
        sidebar.append(&Separator::new(Orientation::Horizontal));

        let bottom_filter_list = ListBox::new();
        bottom_filter_list.add_css_class("fond-list");
        bottom_filter_list.set_selection_mode(gtk4::SelectionMode::Single);
        sidebar.append(&bottom_filter_list);

        sidebar.append(&Separator::new(Orientation::Horizontal));

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
        search_entry.set_placeholder_text(Some("Search titles, text and authors…"));
        search_entry.set_width_request(290);
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
        sort_dropdown.set_selected(sort_to_index(&sort_from_pref(&prefs.sort)));
        let view_menu = gtk4::gio::Menu::new();
        view_menu.append(Some("Compact list"), Some("view.compact"));
        view_menu.append(
            Some("Save labels and notes in the folder"),
            Some("view.export"),
        );
        let view_btn = gtk4::MenuButton::new();
        view_btn.set_icon_name("open-menu-symbolic");
        view_btn.set_menu_model(Some(&view_menu));
        view_btn.set_tooltip_text(Some("View"));
        view_btn.add_css_class("flat");
        view_btn.update_property(&[gtk4::accessible::Property::Label("View options")]);
        right_header.pack_end(&view_btn);
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
        empty_page.add_css_class("compact");
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
        let empty_import_btn = Button::with_label("Import…");
        empty_import_btn.add_css_class("flat");
        empty_import_btn.set_halign(Align::Center);
        empty_actions.append(&empty_new_doc_btn);
        empty_actions.append(&empty_import_btn);
        empty_actions.append(&empty_clear_search_btn);
        empty_page.set_child(Some(&empty_actions));

        let doc_list_stack = Stack::new();
        doc_list_stack.set_vexpand(true);
        doc_list_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        doc_list_stack.add_named(&doc_scroll, Some("docs"));
        doc_list_stack.add_named(&empty_page, Some("empty"));
        let scope_bar = GtkBox::new(Orientation::Horizontal, 8);
        scope_bar.set_margin_start(12);
        scope_bar.set_margin_end(12);
        scope_bar.set_margin_top(6);
        scope_bar.set_visible(false);
        let scope_label = Label::new(None);
        scope_label.add_css_class("fond-row-meta");
        scope_label.set_halign(Align::Start);
        let scope_btn = Button::new();
        scope_btn.add_css_class("flat");
        scope_btn.add_css_class("fond-quiet");
        scope_bar.append(&scope_label);
        scope_bar.append(&scope_btn);
        let content = GtkBox::new(Orientation::Vertical, 0);
        content.append(&scope_bar);
        content.append(&doc_list_stack);
        right.set_content(Some(&content));

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

        let bulk_organize_btn = Button::with_label("Organize…");
        bulk_organize_btn.add_css_class("flat");
        action_bar.append(&bulk_organize_btn);

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

        root.append(&right);

        toast_overlay.set_child(Some(&root));

        // F1 labels everything on screen, same as the main editor window —
        // Library's Project/Label/Archive/Trash sidebar has no other
        // in-app explanation anywhere.
        let help_overlay = super::help_overlay::HelpOverlay::new(&toast_overlay);
        help_overlay.annotate(
            &filter_list,
            "Filters",
            "All Documents, plus any Projects and Labels you've made, and the Authors your documents cite. Click one to show only those documents. Drag a document onto a project or label to file it; hover a row for its ⋯ menu.",
        );
        help_overlay.annotate(
            &bottom_filter_list,
            "Trash & Archive",
            "Trash holds deleted documents until you empty it. Archive holds documents you're done with but want to keep.",
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
            let (w, h) = (win.width(), win.height());
            if w > 0 && h > 0 {
                crate::config::update(|c| {
                    c.library.width = w;
                    c.library.height = h;
                })
                .ok();
            }
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
            current_sort: Rc::new(RefCell::new(sort_from_pref(&prefs.sort))),
            selection: Rc::new(RefCell::new(HashSet::new())),
            action_bar_revealer,
            selected_count_label,
            toast_overlay,
            on_open: Rc::new(RefCell::new(None)),
            is_open: Rc::new(RefCell::new(None)),
            work_dir,
            config,
            view_mode: Rc::new(RefCell::new(if prefs.compact {
                ViewMode::Compact
            } else {
                ViewMode::List
            })),
            bottom_filter_list,
            doc_list_stack,
            empty_page,
            empty_new_doc_btn,
            empty_import_btn,
            empty_clear_search_btn,
            search_scoped: Rc::new(RefCell::new(false)),
            scope_bar,
            scope_label,
            scope_btn,
            inhibit_select: Rc::new(RefCell::new(false)),
            authors_expanded: Rc::new(RefCell::new(prefs.authors_open)),
            row_widgets: Rc::new(RefCell::new(HashMap::new())),
            list_order: Rc::new(RefCell::new(Vec::new())),
            anchor: Rc::new(RefCell::new(None)),
            syncing_checks: Rc::new(std::cell::Cell::new(false)),
        };

        lw.populate_filter_list();
        lw.populate_doc_list();
        lw.wire_signals(
            &new_doc_btn,
            &import_btn,
            &sort_dropdown,
            &bulk_archive_btn,
            &bulk_organize_btn,
            &bulk_remove_btn,
            &clear_btn,
        );

        lw
    }

    #[allow(clippy::too_many_arguments)]
    fn wire_signals(
        &self,
        new_doc_btn: &Button,
        import_btn: &Button,
        sort_dropdown: &gtk4::DropDown,
        bulk_archive_btn: &Button,
        bulk_organize_btn: &Button,
        bulk_remove_btn: &Button,
        clear_btn: &Button,
    ) {
        {
            let group = gtk4::gio::SimpleActionGroup::new();
            let compact = gtk4::gio::SimpleAction::new_stateful(
                "compact",
                None,
                &(*self.view_mode.borrow() == ViewMode::Compact).to_variant(),
            );
            let this = self.clone();
            compact.connect_activate(move |action, _| {
                let on = !action
                    .state()
                    .and_then(|s| s.get::<bool>())
                    .unwrap_or(false);
                action.set_state(&on.to_variant());
                *this.view_mode.borrow_mut() = if on {
                    ViewMode::Compact
                } else {
                    ViewMode::List
                };
                crate::config::update(|c| c.library.compact = on).ok();
                this.populate_doc_list();
            });
            group.add_action(&compact);

            let export = gtk4::gio::SimpleAction::new_stateful(
                "export",
                None,
                &self.config.borrow().library.export.to_variant(),
            );
            export.connect_activate(move |action, _| {
                let on = !action.state().and_then(|s| s.get::<bool>()).unwrap_or(true);
                action.set_state(&on.to_variant());
                crate::config::update(|c| c.library.export = on).ok();
            });
            group.add_action(&export);
            self.window.insert_action_group("view", Some(&group));
        }
        self.install_doc_actions();
        self.install_project_actions();
        self.install_label_actions();
        self.install_selection_keys();
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
                crate::config::update(|c| c.library.sort = sort_to_pref(&sort).to_string()).ok();
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
            bulk_organize_btn.connect_clicked(move |btn| {
                let ids: Vec<i64> = this.selection.borrow().iter().cloned().collect();
                this.show_organize(ids, btn.upcast_ref());
            });
        }
        {
            let this = self.clone();
            self.search_entry.connect_search_changed(move |entry| {
                if entry.text().trim().is_empty() {
                    *this.search_scoped.borrow_mut() = false;
                }
                this.populate_doc_list();
            });
        }
        {
            // Escape in the box empties it, which also lifts any narrowing.
            self.search_entry
                .connect_stop_search(move |entry| entry.set_text(""));
        }
        {
            let this = self.clone();
            self.scope_btn.connect_clicked(move |_| {
                let narrowed = *this.search_scoped.borrow();
                *this.search_scoped.borrow_mut() = !narrowed;
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
            self.empty_import_btn
                .connect_clicked(move |_| this.import_document());
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
        self.announce_folder_export();
        let changed = self.library.borrow_mut().set_bibliography(bib);
        if changed {
            self.library.borrow_mut().resync_authors();
        }
        self.populate_filter_list();
        self.populate_doc_list();
    }

    /// Says, once, that labels, projects and notes are now also kept in the
    /// folder — a hidden folder appearing in someone's documents, which then
    /// goes wherever the folder goes, deserves a word.
    fn announce_folder_export(&self) {
        let prefs = self.config.borrow().library.clone();
        if !prefs.export || prefs.export_announced {
            return;
        }
        let exported = self
            .library
            .borrow()
            .export_records()
            .map(|r| !r.is_empty())
            .unwrap_or(false);
        if !exported {
            return;
        }
        let toast = adw::Toast::new(
            "Your labels, projects and notes are now also saved in your Zerkalo folder \
             (.zerkalo/library), so they back up with it. You can turn this off in the View menu.",
        );
        toast.set_use_markup(false);
        toast.set_timeout(12);
        self.toast_overlay.add_toast(toast);
        crate::config::update(|c| c.library.export_announced = true).ok();
    }

    pub fn window(&self) -> &adw::Window {
        &self.window
    }
}

fn sort_from_pref(pref: &str) -> SortOrder {
    match pref {
        "created" => SortOrder::Created,
        "opened" => SortOrder::Opened,
        "name" => SortOrder::Title,
        _ => SortOrder::Modified,
    }
}

fn sort_to_pref(sort: &SortOrder) -> &'static str {
    match sort {
        SortOrder::Modified => "modified",
        SortOrder::Created => "created",
        SortOrder::Opened => "opened",
        SortOrder::Title => "name",
    }
}

/// The sort dropdown's position for `sort` — the order of its entries.
fn sort_to_index(sort: &SortOrder) -> u32 {
    match sort {
        SortOrder::Modified => 0,
        SortOrder::Created => 1,
        SortOrder::Opened => 2,
        SortOrder::Title => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_sort_comes_back_as_the_same_sort() {
        for sort in [
            SortOrder::Modified,
            SortOrder::Created,
            SortOrder::Opened,
            SortOrder::Title,
        ] {
            assert_eq!(sort_from_pref(sort_to_pref(&sort)), sort);
        }
    }

    #[test]
    fn an_unknown_or_missing_saved_sort_means_last_edited() {
        assert_eq!(sort_from_pref(""), SortOrder::Modified);
        assert_eq!(sort_from_pref("sideways"), SortOrder::Modified);
    }

    #[test]
    fn the_dropdown_position_matches_the_handler_order() {
        // `connect_selected_notify` maps 1 → Created, 2 → Opened, 3 → Title.
        assert_eq!(sort_to_index(&SortOrder::Created), 1);
        assert_eq!(sort_to_index(&SortOrder::Opened), 2);
        assert_eq!(sort_to_index(&SortOrder::Title), 3);
    }
}

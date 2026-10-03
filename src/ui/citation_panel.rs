use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, Label, ListBox, ListBoxRow, MenuButton, Orientation,
    Popover, Revealer, RevealerTransitionType, ScrolledWindow, SearchEntry, SelectionMode,
    Separator,
};

use crate::bibliography::BibEntry;

type InsertCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;
type ChooseCb = Rc<RefCell<Option<Box<dyn Fn()>>>>;
type SourcesCb = Rc<RefCell<Option<Box<dyn Fn(SourcesAction)>>>>;

/// What the Sources menu asks for beyond choosing a file, vault or new file
/// (those have their own callbacks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourcesAction {
    ConnectZotero,
    PickFromZotero,
    KeepCopy(bool),
    TellKartoteka(bool),
    Freeze,
}

/// How the Sources menu's switches and optional items should look.
#[derive(Clone, Copy, Debug, Default)]
pub struct SourcesState {
    pub keep_copy: bool,
    pub tell_kartoteka: bool,
    pub vault_in_use: bool,
    pub zotero_ready: bool,
}

/// The panel connects to either a bibliography or a Skrizhal CV-element
/// database, never both at once — `cv_mode` mirrors the active document's
/// CV mode (`EditorPane::set_cv_mode`) and swaps which list is shown.
#[derive(Clone)]
pub struct CitationPanel {
    widget: GtkBox,
    list: ListBox,
    search: SearchEntry,
    /// An optional page (or range, or "ch. 3") for the *next* citation inserted: `12` gives
    /// `@key[p. 12]`. Cleared once used, so it never leaks into a later citation.
    page: gtk4::Entry,
    title_label: Label,
    bib_entries: Rc<RefCell<Vec<BibEntry>>>,
    cv_entries: Rc<RefCell<Vec<skrizhal_core::CvEntry>>>,
    cv_mode: Rc<Cell<bool>>,
    on_insert: InsertCb,
    on_choose_bib: ChooseCb,
    on_new_bib: ChooseCb,
    on_choose_cv: ChooseCb,
    on_choose_vault: ChooseCb,
    on_open_skrizhal: ChooseCb,
    on_open_kartoteka: ChooseCb,
    choose_btn: Button,
    sources_btn: MenuButton,
    on_sources: SourcesCb,
    keep_copy_check: CheckButton,
    tell_kartoteka_check: CheckButton,
    zotero_pick_btn: Button,
    /// Set while the menu's switches are being set from outside, so that
    /// doing so doesn't look like the person clicking them.
    syncing_switches: Rc<Cell<bool>>,
    new_bib_btn: Button,
    vault_btn: Button,
    kartoteka_btn: Button,
    bib_name_label: Label,
    skrizhal_btn: Button,
    bib_filename: Rc<RefCell<Option<String>>>,
    /// Why the current source couldn't be read, shown instead of the
    /// "No bibliography yet" invitation — a broken file is not an empty one.
    bib_problem: Rc<RefCell<Option<String>>>,
    /// Keys the open document already cites; they are listed first.
    cited: Rc<RefCell<std::collections::HashSet<String>>>,
    cv_filename: Rc<RefCell<Option<String>>>,
    collapse_btn: Button,
    revealer: Revealer,
    on_collapse_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
}

impl CitationPanel {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);
        widget.add_css_class("fond-sidebar");

        let header_box = GtkBox::new(Orientation::Horizontal, 6);
        header_box.set_margin_start(10);
        header_box.set_margin_end(6);
        header_box.set_margin_top(6);
        header_box.set_margin_bottom(6);

        let dot = Label::new(Some("\u{25cf}"));
        dot.add_css_class("fond-section-dot");
        dot.add_css_class("fond-accent-citations");
        dot.set_valign(Align::Center);
        header_box.append(&dot);

        let title_label = Label::new(Some("Citations"));
        title_label.set_xalign(0.0);
        title_label.add_css_class("fond-section-title");
        header_box.append(&title_label);

        let bib_name_label = Label::new(None);
        bib_name_label.add_css_class("dim-label");
        bib_name_label.add_css_class("caption");
        bib_name_label.set_hexpand(true);
        bib_name_label.set_halign(Align::Start);
        bib_name_label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        bib_name_label.set_visible(false);
        header_box.append(&bib_name_label);

        // In CV mode, replaces bib_name_label — opens the actual Skrizhal
        // app to edit the YAML database, rather than just naming the file.
        let skrizhal_btn = Button::with_label("Skrizhal");
        skrizhal_btn.add_css_class("flat");
        skrizhal_btn.set_hexpand(true);
        skrizhal_btn.set_halign(Align::Start);
        skrizhal_btn.set_tooltip_text(Some("Open Skrizhal to edit CV elements"));
        skrizhal_btn.set_visible(false);
        header_box.append(&skrizhal_btn);

        // Bib mode only (hidden in CV mode, like bib_name_label) — Kartoteka's library is a
        // vault folder, not a single file, so choose_btn's file-only dialog can't point at
        // one; this opens a folder picker instead.
        let vault_btn = Button::from_icon_name("folder-symbolic");
        vault_btn.add_css_class("flat");
        vault_btn.add_css_class("circular");
        vault_btn.set_tooltip_text(Some("Choose a Kartoteka vault folder"));
        vault_btn.update_property(&[gtk4::accessible::Property::Label(
            "Choose a Kartoteka vault folder",
        )]);
        vault_btn.set_visible(false);
        header_box.append(&vault_btn);

        // Bib mode only — launches (or focuses, if already running) the actual Kartoteka
        // app, same idea as skrizhal_btn does for CV elements. A bare "K" used to be the
        // whole label, with no accessible name either — this reads the same way its
        // circular siblings (vault_btn, choose_btn, new_bib_btn) do.
        let kartoteka_btn = Button::from_icon_name("send-to-symbolic");
        kartoteka_btn.add_css_class("flat");
        kartoteka_btn.add_css_class("circular");
        kartoteka_btn.set_tooltip_text(Some("Open Kartoteka"));
        kartoteka_btn.update_property(&[gtk4::accessible::Property::Label("Open Kartoteka")]);
        kartoteka_btn.set_visible(false);
        header_box.append(&kartoteka_btn);

        let choose_btn = Button::from_icon_name("document-open-symbolic");
        choose_btn.add_css_class("flat");
        choose_btn.add_css_class("circular");
        choose_btn.set_tooltip_text(Some(
            "Choose bibliography file (.bib, .yaml) — including a library exported from Zotero, Mendeley, or any other reference manager as BibTeX",
        ));
        choose_btn.update_property(&[gtk4::accessible::Property::Label(
            "Choose bibliography file",
        )]);
        choose_btn.set_visible(false);
        header_box.append(&choose_btn);

        // Bib mode only — creates a new, empty .bib file so a first-time user
        // isn't stuck needing one to already exist before they can add a source.
        let new_bib_btn = Button::from_icon_name("list-add-symbolic");
        new_bib_btn.add_css_class("flat");
        new_bib_btn.add_css_class("circular");
        new_bib_btn.set_tooltip_text(Some("Start a new bibliography"));
        new_bib_btn.update_property(&[gtk4::accessible::Property::Label(
            "Start a new bibliography",
        )]);
        new_bib_btn.set_visible(false);
        header_box.append(&new_bib_btn);

        // The one place to say where citations come from. It replaces the row of
        // small icons that used to do this (they are still created for CV mode's
        // sake, or hidden here).
        let on_sources: SourcesCb = Rc::new(RefCell::new(None));
        let syncing_switches = Rc::new(Cell::new(false));
        let sources_btn = MenuButton::new();
        sources_btn.set_label("Sources");
        sources_btn.add_css_class("flat");
        sources_btn.set_tooltip_text(Some("Where your citations come from"));
        let popover = Popover::new();
        let menu_box = GtkBox::new(Orientation::Vertical, 2);
        menu_box.set_margin_top(6);
        menu_box.set_margin_bottom(6);
        menu_box.set_margin_start(6);
        menu_box.set_margin_end(6);
        let item = |label: &str, tip: &str| {
            let b = Button::with_label(label);
            b.add_css_class("flat");
            b.set_halign(Align::Fill);
            if let Some(child) = b.child().and_then(|c| c.downcast::<Label>().ok()) {
                child.set_xalign(0.0);
            }
            b.set_tooltip_text(Some(tip));
            b
        };
        let pop_for = |b: &Button, f: Box<dyn Fn()>| {
            let pop = popover.clone();
            b.connect_clicked(move |_| {
                pop.popdown();
                f();
            });
        };
        let choose_item = item(
            "Choose a file…",
            "A .bib or .yaml file — such as the one Zotero keeps updated",
        );
        let vault_item = item(
            "Choose a Kartoteka vault…",
            "Cite from your Kartoteka library, live",
        );
        let new_item = item(
            "Start a new bibliography…",
            "Make an empty .bib file to add sources to",
        );
        let zotero_item = item(
            "Connect Zotero…",
            "How to keep Zotero's export up to date here",
        );
        let zotero_pick = item(
            "Pick from Zotero…",
            "Open Zotero's own picker and cite what you choose",
        );
        zotero_pick.set_visible(false);
        let freeze_item = item(
            "Freeze for submission…",
            "Save a small file holding only the sources this document cites, and point the document at it",
        );
        let kartoteka_item = item("Open Kartoteka", "Open the Kartoteka app");
        let keep_copy_check = CheckButton::with_label("Keep a copy in the project");
        keep_copy_check.set_tooltip_text(Some(
            "When you choose a library stored elsewhere, keep an up-to-date copy in this folder so your document compiles on any computer. Copies already made keep updating.",
        ));
        keep_copy_check.set_margin_start(8);
        let tell_kartoteka_check =
            CheckButton::with_label("Tell Kartoteka which documents cite its sources");
        tell_kartoteka_check.set_tooltip_text(Some(
            "Adds one small file to the vault's projects folder so Kartoteka can show where each source is used. Off unless you turn it on.",
        ));
        tell_kartoteka_check.set_margin_start(8);
        tell_kartoteka_check.set_visible(false);

        menu_box.append(&choose_item);
        menu_box.append(&vault_item);
        menu_box.append(&new_item);
        menu_box.append(&Separator::new(Orientation::Horizontal));
        menu_box.append(&zotero_item);
        menu_box.append(&zotero_pick);
        menu_box.append(&Separator::new(Orientation::Horizontal));
        menu_box.append(&keep_copy_check);
        menu_box.append(&tell_kartoteka_check);
        menu_box.append(&freeze_item);
        menu_box.append(&kartoteka_item);
        popover.set_child(Some(&menu_box));
        sources_btn.set_popover(Some(&popover));
        header_box.append(&sources_btn);

        // Furthest right on the bar, matching Comments' and Packages'
        // collapse toggle — collapsing hides everything below the header via
        // the Revealer wrapping `body` below, and `AppWindow`'s sidebar
        // wiring (editor_extras.rs) reclaims the freed space in the shared
        // Paned rather than leaving a blank gap.
        let collapse_btn = Button::from_icon_name("pan-down-symbolic");
        collapse_btn.add_css_class("flat");
        collapse_btn.set_tooltip_text(Some("Hide Citations"));
        collapse_btn.update_property(&[gtk4::accessible::Property::Label("Hide Citations")]);
        header_box.append(&collapse_btn);

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&header_box);
        widget.append(&Separator::new(Orientation::Horizontal));

        let body = GtkBox::new(Orientation::Vertical, 0);
        body.set_vexpand(true);

        let search = SearchEntry::new();
        search.set_placeholder_text(Some("Search by key, author, title…"));
        search.set_margin_start(8);
        search.set_margin_end(8);
        search.set_margin_top(6);
        search.set_margin_bottom(6);
        search.set_size_request(0, -1);
        body.append(&search);
        // Fill this in first, then pick the source (a single click inserts it).
        let page = gtk4::Entry::builder()
            .placeholder_text("Page for the next citation (optional)")
            .tooltip_text(
                "Type a page, a range such as 12-14, or something like ch. 3, then click the \
                 source: it is inserted as @key[p. 12]. Leave it empty for a plain @key.",
            )
            .margin_start(8)
            .margin_end(8)
            .margin_bottom(6)
            .build();
        page.set_size_request(0, -1);
        body.append(&page);
        body.append(&Separator::new(Orientation::Horizontal));

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        let list = ListBox::new();
        list.set_selection_mode(SelectionMode::Single);
        list.set_activate_on_single_click(true);
        list.add_css_class("fond-list");
        list.set_margin_start(12);
        list.set_margin_end(12);
        list.set_margin_bottom(8);
        scroll.set_child(Some(&list));
        body.append(&scroll);

        let revealer = Revealer::new();
        revealer.set_transition_type(RevealerTransitionType::SlideDown);
        revealer.set_reveal_child(true);
        revealer.set_vexpand(true);
        revealer.set_child(Some(&body));
        widget.append(&revealer);

        let on_collapse_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> =
            Rc::new(RefCell::new(None));
        {
            let revealer = revealer.clone();
            let collapse_btn_c = collapse_btn.clone();
            let on_collapse_toggle_c = on_collapse_toggle.clone();
            collapse_btn.connect_clicked(move |_| {
                let now_collapsed = revealer.reveals_child();
                revealer.set_reveal_child(!now_collapsed);
                collapse_btn_c.set_icon_name(if now_collapsed {
                    "pan-end-symbolic"
                } else {
                    "pan-down-symbolic"
                });
                collapse_btn_c.set_tooltip_text(Some(if now_collapsed {
                    "Show Citations"
                } else {
                    "Hide Citations"
                }));
                if let Some(f) = on_collapse_toggle_c.borrow().as_ref() {
                    f(now_collapsed);
                }
            });
        }

        let on_insert: InsertCb = Rc::new(RefCell::new(None));
        let on_choose_bib: ChooseCb = Rc::new(RefCell::new(None));
        let on_new_bib: ChooseCb = Rc::new(RefCell::new(None));
        let on_choose_cv: ChooseCb = Rc::new(RefCell::new(None));
        let on_choose_vault: ChooseCb = Rc::new(RefCell::new(None));
        let on_open_skrizhal: ChooseCb = Rc::new(RefCell::new(None));
        let on_open_kartoteka: ChooseCb = Rc::new(RefCell::new(None));
        let bib_entries: Rc<RefCell<Vec<BibEntry>>> = Rc::new(RefCell::new(Vec::new()));
        let cv_entries: Rc<RefCell<Vec<skrizhal_core::CvEntry>>> =
            Rc::new(RefCell::new(Vec::new()));
        let cv_mode: Rc<Cell<bool>> = Rc::new(Cell::new(false));

        // Wire activation once on the list — fires on double-click and Enter.
        // Row's widget_name holds the citation/CV key set during rebuild_list;
        // the insert text format depends on which mode was active at build time.
        {
            let cb = on_insert.clone();
            let cv_mode_ra = cv_mode.clone();
            let page_ra = page.clone();
            list.connect_row_activated(move |_, row| {
                let key = row.widget_name().to_string();
                if !key.is_empty() {
                    let text = insert_text_for(&key, cv_mode_ra.get(), page_ra.text().as_str());
                    page_ra.set_text("");
                    if let Some(f) = cb.borrow().as_ref() {
                        f(text);
                    }
                }
            });
        }

        {
            let cb_bib = on_choose_bib.clone();
            let cb_cv = on_choose_cv.clone();
            let cv_mode_cb = cv_mode.clone();
            choose_btn.connect_clicked(move |_| {
                if cv_mode_cb.get() {
                    if let Some(f) = cb_cv.borrow().as_ref() {
                        f();
                    }
                } else if let Some(f) = cb_bib.borrow().as_ref() {
                    f();
                }
            });
        }

        {
            let cb = on_new_bib.clone();
            new_bib_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }

        {
            let cb = on_open_skrizhal.clone();
            skrizhal_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }

        {
            let cb = on_choose_vault.clone();
            vault_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }

        {
            let cb = on_open_kartoteka.clone();
            kartoteka_btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f();
                }
            });
        }

        // Sources menu items.
        {
            let cb = on_choose_bib.clone();
            pop_for(
                &choose_item,
                Box::new(move || {
                    if let Some(f) = cb.borrow().as_ref() {
                        f();
                    }
                }),
            );
            let cb = on_choose_vault.clone();
            pop_for(
                &vault_item,
                Box::new(move || {
                    if let Some(f) = cb.borrow().as_ref() {
                        f();
                    }
                }),
            );
            let cb = on_new_bib.clone();
            pop_for(
                &new_item,
                Box::new(move || {
                    if let Some(f) = cb.borrow().as_ref() {
                        f();
                    }
                }),
            );
            let cb = on_open_kartoteka.clone();
            pop_for(
                &kartoteka_item,
                Box::new(move || {
                    if let Some(f) = cb.borrow().as_ref() {
                        f();
                    }
                }),
            );
            for (btn, action) in [
                (&zotero_item, SourcesAction::ConnectZotero),
                (&zotero_pick, SourcesAction::PickFromZotero),
                (&freeze_item, SourcesAction::Freeze),
            ] {
                let cb = on_sources.clone();
                pop_for(
                    btn,
                    Box::new(move || {
                        if let Some(f) = cb.borrow().as_ref() {
                            f(action);
                        }
                    }),
                );
            }
            let (cb, guard) = (on_sources.clone(), syncing_switches.clone());
            keep_copy_check.connect_toggled(move |c| {
                if !guard.get() {
                    if let Some(f) = cb.borrow().as_ref() {
                        f(SourcesAction::KeepCopy(c.is_active()));
                    }
                }
            });
            let (cb, guard) = (on_sources.clone(), syncing_switches.clone());
            tell_kartoteka_check.connect_toggled(move |c| {
                if !guard.get() {
                    if let Some(f) = cb.borrow().as_ref() {
                        f(SourcesAction::TellKartoteka(c.is_active()));
                    }
                }
            });
        }

        let panel = Self {
            widget,
            list,
            search,
            page,
            title_label,
            bib_entries,
            cv_entries,
            cv_mode,
            on_insert,
            on_choose_bib,
            on_new_bib,
            on_choose_cv,
            on_choose_vault,
            on_open_skrizhal,
            on_open_kartoteka,
            choose_btn,
            sources_btn,
            on_sources,
            keep_copy_check,
            tell_kartoteka_check,
            zotero_pick_btn: zotero_pick,
            syncing_switches,
            new_bib_btn,
            vault_btn,
            kartoteka_btn,
            bib_name_label,
            skrizhal_btn,
            bib_filename: Rc::new(RefCell::new(None)),
            bib_problem: Rc::new(RefCell::new(None)),
            cited: Rc::new(RefCell::new(std::collections::HashSet::new())),
            cv_filename: Rc::new(RefCell::new(None)),
            collapse_btn,
            revealer,
            on_collapse_toggle,
        };

        {
            let p = panel.clone();
            panel.search.connect_search_changed(move |e| {
                p.rebuild_list(e.text().as_str());
            });
        }

        panel
    }

    /// First and last row carry the card's rounded corners; with none, there is
    /// no card to round.
    fn round_card_ends(&self, shown: usize) {
        if shown == 0 {
            return;
        }
        if let Some(first) = self.list.row_at_index(0) {
            first.add_css_class("fond-card-first");
        }
        if let Some(last) = self.list.row_at_index(shown as i32 - 1) {
            last.add_css_class("fond-card-last");
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    /// Restores a persisted collapsed/expanded state — called once at
    /// startup, same idiom as `PackageBrowser`/`CommentsPanel`.
    pub fn set_collapsed(&self, collapsed: bool) {
        self.revealer.set_reveal_child(!collapsed);
        self.collapse_btn.set_icon_name(if collapsed {
            "pan-end-symbolic"
        } else {
            "pan-down-symbolic"
        });
        self.collapse_btn.set_tooltip_text(Some(if collapsed {
            "Show Citations"
        } else {
            "Hide Citations"
        }));
    }

    /// Fires once the collapse/expand slide animation has finished — the section's
    /// final size isn't measurable until then.
    pub fn connect_collapse_settled(&self, f: impl Fn() + 'static) {
        self.revealer.connect_child_revealed_notify(move |_| f());
    }

    pub fn is_collapsed(&self) -> bool {
        !self.revealer.reveals_child()
    }

    /// Fires with the new collapsed state whenever the user clicks the
    /// header's collapse toggle, so the caller can persist it.
    pub fn set_on_collapse_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_collapse_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn load_bib(&self, entries: Vec<BibEntry>) {
        *self.bib_entries.borrow_mut() = entries;
        if !self.cv_mode.get() {
            let query = self.search.text();
            self.rebuild_list(query.as_str());
        }
    }

    pub fn load_cv_entries(&self, entries: Vec<skrizhal_core::CvEntry>) {
        *self.cv_entries.borrow_mut() = entries;
        if self.cv_mode.get() {
            let query = self.search.text();
            self.rebuild_list(query.as_str());
        }
    }

    /// Swaps the panel between citation mode and CV-element mode — the
    /// active document's `#doc-kind: cv` front matter drives this, not a
    /// user toggle (see `EditorPane::set_cv_mode`).
    pub fn set_cv_mode(&self, active: bool) {
        if self.cv_mode.get() == active {
            return;
        }
        self.cv_mode.set(active);
        self.page.set_visible(!active);
        if active {
            self.title_label.set_text("CV Elements");
            self.search
                .set_placeholder_text(Some("Search by key, title, tag…"));
            self.choose_btn
                .set_tooltip_text(Some("Choose CV element file (.yaml)"));
            self.choose_btn
                .update_property(&[gtk4::accessible::Property::Label("Choose CV element file")]);
            self.bib_name_label.set_visible(false);
            self.skrizhal_btn.set_visible(true);
            self.choose_btn.set_visible(true);
            self.sources_btn.set_visible(false);
            self.vault_btn.set_visible(false);
            self.kartoteka_btn.set_visible(false);
            self.new_bib_btn.set_visible(false);
        } else {
            self.title_label.set_text("Citations");
            self.search
                .set_placeholder_text(Some("Search by key, author, title…"));
            self.choose_btn.set_tooltip_text(Some(
                "Choose bibliography file (.bib, .yaml) — including a library exported from Zotero, Mendeley, or any other reference manager as BibTeX",
            ));
            self.choose_btn
                .update_property(&[gtk4::accessible::Property::Label(
                    "Choose bibliography file",
                )]);
            self.skrizhal_btn.set_visible(false);
            // The Sources menu replaces the separate file / vault / new / Kartoteka
            // icons.
            self.choose_btn.set_visible(false);
            self.sources_btn.set_visible(true);
            self.vault_btn.set_visible(false);
            self.kartoteka_btn.set_visible(false);
            self.new_bib_btn.set_visible(false);
            self.refresh_filename_label(self.bib_filename.borrow().as_deref());
        }
        let query = self.search.text();
        self.rebuild_list(query.as_str());
    }

    pub fn set_on_insert(&self, f: impl Fn(String) + 'static) {
        *self.on_insert.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_choose_bib(&self, f: impl Fn() + 'static) {
        *self.on_choose_bib.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_new_bib(&self, f: impl Fn() + 'static) {
        *self.on_new_bib.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_choose_cv(&self, f: impl Fn() + 'static) {
        *self.on_choose_cv.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_open_skrizhal(&self, f: impl Fn() + 'static) {
        *self.on_open_skrizhal.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_sources(&self, f: impl Fn(SourcesAction) + 'static) {
        *self.on_sources.borrow_mut() = Some(Box::new(f));
    }

    /// Brings the Sources menu's switches and optional items in line with the
    /// current settings, without it counting as a click.
    pub fn set_sources_state(&self, state: SourcesState) {
        self.syncing_switches.set(true);
        self.keep_copy_check.set_active(state.keep_copy);
        self.tell_kartoteka_check.set_active(state.tell_kartoteka);
        self.syncing_switches.set(false);
        self.tell_kartoteka_check.set_visible(state.vault_in_use);
        self.zotero_pick_btn.set_visible(state.zotero_ready);
    }

    pub fn set_on_choose_vault(&self, f: impl Fn() + 'static) {
        *self.on_choose_vault.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_open_kartoteka(&self, f: impl Fn() + 'static) {
        *self.on_open_kartoteka.borrow_mut() = Some(Box::new(f));
    }

    /// Records (or clears) why the current source couldn't be read. Call
    /// before `load_bib`, which redraws the list.
    pub fn set_bib_problem(&self, problem: Option<String>) {
        *self.bib_problem.borrow_mut() = problem;
    }

    /// The keys the open document cites, so they sort to the top.
    pub fn set_cited_keys(&self, keys: std::collections::HashSet<String>) {
        if *self.cited.borrow() == keys {
            return;
        }
        *self.cited.borrow_mut() = keys;
        if !self.cv_mode.get() {
            let query = self.search.text();
            self.rebuild_list(query.as_str());
        }
    }

    pub fn set_bib_filename(&self, name: Option<&str>) {
        *self.bib_filename.borrow_mut() = name.map(str::to_string);
        if !self.cv_mode.get() {
            self.refresh_filename_label(name);
        }
    }

    pub fn set_cv_filename(&self, name: Option<&str>) {
        *self.cv_filename.borrow_mut() = name.map(str::to_string);
        self.skrizhal_btn.set_tooltip_text(Some(&match name {
            Some(n) => format!("Open Skrizhal to edit CV elements ({n})"),
            None => "Open Skrizhal to edit CV elements".to_string(),
        }));
    }

    fn refresh_filename_label(&self, name: Option<&str>) {
        match name {
            Some(n) => {
                self.bib_name_label.set_text(n);
                self.bib_name_label.set_visible(true);
            }
            None => {
                self.bib_name_label.set_visible(false);
            }
        }
    }

    fn rebuild_list(&self, filter: &str) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        if self.cv_mode.get() {
            self.rebuild_cv_list(filter);
        } else {
            self.rebuild_bib_list(filter);
        }
    }

    fn rebuild_bib_list(&self, filter: &str) {
        let entries = self.bib_entries.borrow();

        if entries.is_empty() {
            if let Some(problem) = self.bib_problem.borrow().as_deref() {
                self.append_placeholder(&format!("{problem}\nYour file hasn't been changed."));
            } else if self.bib_filename.borrow().is_some() {
                self.append_placeholder("This bibliography has no entries yet.");
            } else {
                self.append_placeholder("No bibliography loaded yet.\nUse + above to start one, or the folder icon to pick an existing file.");
            }
            return;
        }

        let mut shown = 0usize;
        let order = crate::cite_search::search(&entries, filter, &self.cited.borrow());
        for entry in order.iter().map(|&i| &entries[i]) {
            let row = ListBoxRow::new();
            row.set_activatable(true);
            row.add_css_class("fond-card");
            row.add_css_class("fond-row");
            row.set_widget_name(&entry.key);
            row.set_tooltip_text(Some(&format!("Click to insert @{}", entry.key)));

            let box_ = GtkBox::new(Orientation::Vertical, 2);
            box_.set_margin_start(8);
            box_.set_margin_end(8);

            let top = GtkBox::new(Orientation::Horizontal, 4);
            let key_lbl = Label::new(None);
            key_lbl.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&entry.key)));
            key_lbl.set_xalign(0.0);
            key_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

            let meta_str = match (entry.author.is_empty(), entry.year.is_empty()) {
                (false, false) => format!(" · {} ({})", entry.author, entry.year),
                (false, true) => format!(" · {}", entry.author),
                (true, false) => format!(" · ({})", entry.year),
                (true, true) => String::new(),
            };
            let meta_lbl = Label::new(Some(&meta_str));
            meta_lbl.add_css_class("dim-label");
            meta_lbl.add_css_class("caption");
            meta_lbl.set_xalign(0.0);
            meta_lbl.set_hexpand(true);
            meta_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            top.append(&key_lbl);
            top.append(&meta_lbl);
            box_.append(&top);

            if !entry.title.is_empty() {
                let title_lbl = Label::new(None);
                title_lbl.set_markup(&format!(
                    "<i>{}</i>",
                    glib::markup_escape_text(&entry.title)
                ));
                title_lbl.set_xalign(0.0);
                title_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                title_lbl.add_css_class("caption");
                box_.append(&title_lbl);
            }

            row.set_child(Some(&box_));
            self.list.append(&row);
            shown += 1;
        }

        self.round_card_ends(shown);

        if shown == 0 {
            self.append_placeholder("No matching entries");
        }
    }

    fn rebuild_cv_list(&self, filter: &str) {
        let filter_lower = filter.to_lowercase();
        let entries = self.cv_entries.borrow();

        if entries.is_empty() {
            self.append_placeholder("No CV elements loaded.\nSet a Skrizhal file in Settings.");
            return;
        }

        let mut shown = 0usize;
        for entry in entries.iter() {
            if !filter_lower.is_empty() {
                let haystack = format!(
                    "{} {} {} {}",
                    entry.key,
                    entry.title,
                    entry.organization.as_deref().unwrap_or(""),
                    entry.tags.join(" ")
                )
                .to_lowercase();
                if !haystack.contains(&filter_lower) {
                    continue;
                }
            }

            let row = ListBoxRow::new();
            row.set_activatable(true);
            row.set_widget_name(&entry.key);
            row.set_tooltip_text(Some(&format!(
                "Double-click or Enter to insert #cv-entry(\"{}\")",
                entry.key
            )));

            let box_ = GtkBox::new(Orientation::Vertical, 2);
            box_.set_margin_start(8);
            box_.set_margin_end(8);

            let top = GtkBox::new(Orientation::Horizontal, 4);
            let title_lbl = Label::new(None);
            let title_text = if entry.title.is_empty() {
                entry.key.as_str()
            } else {
                &entry.title
            };
            title_lbl.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(title_text)));
            title_lbl.set_xalign(0.0);
            title_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

            let meta_str = match (&entry.organization, &entry.date) {
                (Some(org), Some(date)) => format!(" · {org} ({date})"),
                (Some(org), None) => format!(" · {org}"),
                (None, Some(date)) => format!(" · ({date})"),
                (None, None) => String::new(),
            };
            let meta_lbl = Label::new(Some(&meta_str));
            meta_lbl.add_css_class("dim-label");
            meta_lbl.add_css_class("caption");
            meta_lbl.set_xalign(0.0);
            meta_lbl.set_hexpand(true);
            meta_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            top.append(&title_lbl);
            top.append(&meta_lbl);
            box_.append(&top);

            let sub_str = format!("{} · {}", entry.category, entry.key);
            let sub_lbl = Label::new(Some(&sub_str));
            sub_lbl.set_xalign(0.0);
            sub_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            sub_lbl.add_css_class("caption");
            sub_lbl.add_css_class("dim-label");
            box_.append(&sub_lbl);

            row.set_child(Some(&box_));
            self.list.append(&row);
            shown += 1;
        }

        if shown == 0 {
            self.append_placeholder("No matching entries");
        }
    }

    fn append_placeholder(&self, text: &str) {
        let row = ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        let lbl = Label::new(Some(text));
        lbl.add_css_class("dim-label");
        lbl.set_justify(gtk4::Justification::Center);
        lbl.set_margin_top(16);
        lbl.set_margin_bottom(16);
        row.set_child(Some(&lbl));
        self.list.append(&row);
    }
}

/// The text a click on `key` inserts: `#cv-entry("key")` for a CV element, otherwise a
/// citation — `@key`, or `@key[p. 12]` when `page` is filled in (the same text Kartoteka's Cite
/// box produces, from `fond_bib::cite`).
fn insert_text_for(key: &str, cv: bool, page: &str) -> String {
    if cv {
        format!("#cv-entry(\"{key}\")")
    } else {
        fond_bib::cite::typst_citation(key, Some(page))
    }
}

#[cfg(test)]
mod insert_text_tests {
    use super::insert_text_for;

    #[test]
    fn a_citation_carries_the_page_when_there_is_one() {
        assert_eq!(
            insert_text_for("cone1970black", false, ""),
            "@cone1970black"
        );
        assert_eq!(
            insert_text_for("cone1970black", false, "  "),
            "@cone1970black"
        );
        assert_eq!(
            insert_text_for("cone1970black", false, "12"),
            "@cone1970black[p. 12]"
        );
        assert_eq!(
            insert_text_for("cone1970black", false, "12-14"),
            "@cone1970black[pp. 12–14]"
        );
        assert_eq!(
            insert_text_for("cone1970black", false, "ch. 3"),
            "@cone1970black[ch. 3]"
        );
    }

    #[test]
    fn a_cv_element_ignores_the_page() {
        assert_eq!(insert_text_for("job-1", true, "12"), "#cv-entry(\"job-1\")");
    }
}

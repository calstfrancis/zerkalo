use std::cell::RefCell;
use std::rc::Rc;

use gtk4::gdk::Rectangle;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, EventControllerKey, Label, ListBox, ListBoxRow, Orientation, Popover,
    PositionType, ScrolledWindow, SelectionMode,
};

use crate::bibliography::{format_author_year, BibEntry};

/// Which source `show_filtered` should search — selected by which trigger
/// character opened the popup (`@` vs `!`, see editor_pane.rs).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PopupSource {
    Bib,
    Cv,
}

/// One matched item, from either source. Keeping this as an enum (rather
/// than, say, having `BibPopup` juggle two separate widget-building code
/// paths) means `show_filtered`/`append_row` only need to handle one shape
/// of "thing with a key, a search haystack, and a couple of display lines"
/// regardless of which source it came from.
#[derive(Clone)]
pub enum PopupEntry {
    Bib(BibEntry),
    Cv(skrizhal_core::CvEntry),
}

impl PopupEntry {
    fn key(&self) -> &str {
        match self {
            PopupEntry::Bib(e) => &e.key,
            PopupEntry::Cv(e) => &e.key,
        }
    }

    /// The text inserted into the document on selection — `@key` for a
    /// citation, `#cv-entry("key")` for a CV entry (a string argument, not
    /// a label: see skrizhal/plan.md's Phase 3a correction).
    pub fn insert_text(&self) -> String {
        match self {
            PopupEntry::Bib(e) => format!("@{}", e.key),
            PopupEntry::Cv(e) => format!("#cv-entry(\"{}\")", e.key),
        }
    }

    fn search_haystack(&self) -> String {
        match self {
            PopupEntry::Bib(e) => format!("{} {} {}", e.key, e.author, e.title),
            PopupEntry::Cv(e) => format!(
                "{} {} {} {}",
                e.key,
                e.title,
                e.organization.as_deref().unwrap_or(""),
                e.tags.join(" ")
            ),
        }
    }

    /// The entry's key, for callers outside this module (the inline ghost
    /// completes it, the status line names it).
    pub fn key_text(&self) -> String {
        self.key().to_string()
    }

    /// One line saying what this entry is, for the status hint.
    pub fn describe(&self) -> String {
        match self {
            PopupEntry::Bib(e) => {
                let who = format_author_year(e);
                if e.title.is_empty() {
                    who
                } else {
                    format!("{who} — {}", truncate(&e.title, 60))
                }
            }
            PopupEntry::Cv(e) => {
                let what = if e.title.is_empty() {
                    e.key.clone()
                } else {
                    e.title.clone()
                };
                match e.organization.as_deref().filter(|o| !o.is_empty()) {
                    Some(org) => format!("{what} — {org}"),
                    None => what,
                }
            }
        }
    }

    /// Whether the query is a prefix of whatever field users are most
    /// likely to actually search by — used to float better matches to the
    /// top rather than just relying on match order.
    fn is_prefix_match(&self, q: &str) -> bool {
        match self {
            PopupEntry::Bib(e) => {
                e.key.to_lowercase().starts_with(q) || e.author.to_lowercase().starts_with(q)
            }
            PopupEntry::Cv(e) => {
                e.key.to_lowercase().starts_with(q) || e.title.to_lowercase().starts_with(q)
            }
        }
    }
}

type OnCompleteCb = Rc<RefCell<Option<Box<dyn Fn(PopupEntry)>>>>;

#[derive(Clone)]
pub struct BibPopup {
    popover: Popover,
    list_box: ListBox,
    scroll: ScrolledWindow,
    bib_entries: Rc<RefCell<Vec<BibEntry>>>,
    cv_entries: Rc<RefCell<Vec<skrizhal_core::CvEntry>>>,
    on_complete: OnCompleteCb,
    filtered_entries: Rc<RefCell<Vec<PopupEntry>>>,
}

impl BibPopup {
    pub fn new(
        parent: &impl IsA<gtk4::Widget>,
        bib_entries: Rc<RefCell<Vec<BibEntry>>>,
        cv_entries: Rc<RefCell<Vec<skrizhal_core::CvEntry>>>,
    ) -> Self {
        let popover = Popover::new();
        popover.set_has_arrow(false);
        popover.set_autohide(false);
        popover.set_parent(parent);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Browse);
        list_box.set_activate_on_single_click(true);
        list_box.set_focusable(true);

        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&list_box));
        scroll.set_min_content_width(300);
        scroll.set_min_content_height(60);
        scroll.set_max_content_height(280);
        scroll.set_propagate_natural_height(true);

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.set_margin_top(2);
        outer.set_margin_bottom(2);
        outer.append(&scroll);
        popover.set_child(Some(&outer));

        let on_complete: OnCompleteCb = Rc::new(RefCell::new(None));
        let filtered_entries: Rc<RefCell<Vec<PopupEntry>>> = Rc::new(RefCell::new(Vec::new()));

        // Key controller on the list_box so Tab/Return work when popup has focus
        {
            let on_complete_kc = on_complete.clone();
            let filtered_kc = filtered_entries.clone();
            let list_kc = list_box.clone();
            let popover_kc = popover.clone();
            let scroll_kc = scroll.clone();

            let kc = EventControllerKey::new();
            kc.connect_key_pressed(move |_, key, _, _mods| {
                use gtk4::gdk::Key;
                match key {
                    Key::Tab | Key::Return | Key::KP_Enter => {
                        let idx = list_kc
                            .selected_row()
                            .map(|r| r.index() as usize)
                            .unwrap_or(0);
                        let entry = filtered_kc
                            .borrow()
                            .get(idx)
                            .cloned()
                            .or_else(|| filtered_kc.borrow().first().cloned());
                        if let Some(entry) = entry {
                            popover_kc.popdown();
                            if let Some(f) = on_complete_kc.borrow().as_ref() {
                                f(entry);
                            }
                        }
                        glib::Propagation::Stop
                    }
                    Key::Escape => {
                        popover_kc.popdown();
                        glib::Propagation::Stop
                    }
                    Key::Down => {
                        let cur = list_kc.selected_row().map(|r| r.index()).unwrap_or(-1);
                        if let Some(row) = list_kc.row_at_index(cur + 1) {
                            list_kc.select_row(Some(&row));
                            scroll_row_into_view(&scroll_kc, &list_kc, &row);
                        }
                        glib::Propagation::Stop
                    }
                    Key::Up => {
                        let cur = list_kc.selected_row().map(|r| r.index()).unwrap_or(1);
                        if let Some(row) = list_kc.row_at_index((cur - 1).max(0)) {
                            list_kc.select_row(Some(&row));
                            scroll_row_into_view(&scroll_kc, &list_kc, &row);
                        }
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            });
            list_box.add_controller(kc);
        }

        // Double-click (or Enter when list has focus) triggers insertion
        {
            let filtered_ra = filtered_entries.clone();
            let on_complete_ra = on_complete.clone();
            let popover_ra = popover.clone();
            list_box.connect_row_activated(move |_, row| {
                let idx = row.index() as usize;
                let entry = filtered_ra.borrow().get(idx).cloned();
                if let Some(entry) = entry {
                    popover_ra.popdown();
                    if let Some(f) = on_complete_ra.borrow().as_ref() {
                        f(entry);
                    }
                }
            });
        }

        Self {
            popover,
            list_box,
            scroll,
            bib_entries,
            cv_entries,
            on_complete,
            filtered_entries,
        }
    }

    pub fn set_on_complete(&self, f: impl Fn(PopupEntry) + 'static) {
        *self.on_complete.borrow_mut() = Some(Box::new(f));
    }

    pub fn show_filtered(&self, query: &str, x: i32, y: i32, above: bool, source: PopupSource) {
        self.clear_rows();
        self.filtered_entries.borrow_mut().clear();

        // Collect matched entries into owned data and release the borrow before
        // any GTK widget ops — popover.popup() / select_row / append_row can
        // cascade through signals back into Zerkalo code that tries to borrow
        // entries again, causing a BorrowError panic.
        let q = query.to_lowercase();
        let shown: Vec<PopupEntry> = {
            let mut matched: Vec<PopupEntry> = match source {
                PopupSource::Bib => self
                    .bib_entries
                    .borrow()
                    .iter()
                    .cloned()
                    .map(PopupEntry::Bib)
                    .collect(),
                PopupSource::Cv => self
                    .cv_entries
                    .borrow()
                    .iter()
                    .cloned()
                    .map(PopupEntry::Cv)
                    .collect(),
            };
            matched.retain(|e| q.is_empty() || e.search_haystack().to_lowercase().contains(&q));
            matched.sort_by_key(|e| if e.is_prefix_match(&q) { 0u8 } else { 1u8 });
            matched
        };

        if shown.is_empty() {
            if self.popover.is_visible() {
                self.popover.popdown();
            }
            return;
        }

        for entry in &shown {
            self.filtered_entries.borrow_mut().push(entry.clone());
            self.append_row(entry);
        }

        if let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.select_row(Some(&row));
        }

        self.popover.set_position(if above {
            PositionType::Top
        } else {
            PositionType::Bottom
        });
        self.popover
            .set_pointing_to(Some(&Rectangle::new(x, y, 1, 1)));

        if !self.popover.is_visible() {
            self.popover.popup();
        }
    }

    pub fn hide(&self) {
        if self.popover.is_visible() {
            self.popover.popdown();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.popover.is_visible()
    }

    pub fn first_filtered_entry(&self) -> Option<PopupEntry> {
        self.filtered_entries.borrow().first().cloned()
    }

    /// Entries matching `query`, best first, without touching the popup — so a
    /// caller can decide whether it's worth showing a list at all, and what to
    /// offer as inline ghost text.
    pub fn matches_for(&self, query: &str, source: PopupSource) -> Vec<PopupEntry> {
        let q = query.to_lowercase();
        let mut matched: Vec<PopupEntry> = match source {
            PopupSource::Bib => self
                .bib_entries
                .borrow()
                .iter()
                .cloned()
                .map(PopupEntry::Bib)
                .collect(),
            PopupSource::Cv => self
                .cv_entries
                .borrow()
                .iter()
                .cloned()
                .map(PopupEntry::Cv)
                .collect(),
        };
        matched.retain(|e| q.is_empty() || e.search_haystack().to_lowercase().contains(&q));
        matched.sort_by_key(|e| if e.is_prefix_match(&q) { 0u8 } else { 1u8 });
        matched
    }

    /// The entry whose *key* continues what's been typed — the only kind of
    /// match ghost text can be drawn for, since it's appended to the query.
    pub fn ghost_entry(&self, query: &str, source: PopupSource) -> Option<PopupEntry> {
        if query.is_empty() {
            return None;
        }
        let q = query.to_lowercase();
        self.matches_for(query, source)
            .into_iter()
            .filter(|e| e.key_text().to_lowercase().starts_with(&q))
            .min_by_key(|e| e.key_text().chars().count())
    }

    pub fn move_selection(&self, delta: i32) {
        let current_idx = self.list_box.selected_row().map(|r| r.index()).unwrap_or(0);
        let next_idx = (current_idx + delta).max(0);
        if let Some(row) = self.list_box.row_at_index(next_idx) {
            self.list_box.select_row(Some(&row));
            scroll_row_into_view(&self.scroll, &self.list_box, &row);
        }
    }

    pub fn selected_entry(&self) -> Option<PopupEntry> {
        let row = self.list_box.selected_row()?;
        let idx = row.index() as usize;
        self.filtered_entries.borrow().get(idx).cloned()
    }

    fn clear_rows(&self) {
        while let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.remove(&row);
        }
    }

    fn append_row(&self, entry: &PopupEntry) {
        let row = ListBoxRow::new();
        row.set_activatable(true);

        let row_box = GtkBox::new(Orientation::Vertical, 2);
        row_box.set_margin_top(5);
        row_box.set_margin_bottom(5);
        row_box.set_margin_start(10);
        row_box.set_margin_end(10);

        match entry {
            PopupEntry::Bib(e) => {
                // Primary label: "Smith et al., 2019" — what academics search by
                let citation_lbl = Label::new(None);
                citation_lbl.set_markup(&format!(
                    "<b>{}</b>",
                    glib::markup_escape_text(&format_author_year(e))
                ));
                citation_lbl.set_halign(Align::Start);
                citation_lbl.set_xalign(0.0);
                row_box.append(&citation_lbl);

                if !e.title.is_empty() {
                    let title_lbl = Label::new(Some(&truncate(&e.title, 50)));
                    title_lbl.set_halign(Align::Start);
                    title_lbl.set_xalign(0.0);
                    title_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                    title_lbl.add_css_class("dim-label");
                    title_lbl.add_css_class("caption");
                    row_box.append(&title_lbl);
                }
            }
            PopupEntry::Cv(e) => {
                // Primary label: the entry's title — what a CV author
                // searches by, mirroring the bib popup's author-year primary.
                let title_lbl = Label::new(None);
                let title_text = if e.title.is_empty() {
                    e.key.as_str()
                } else {
                    &e.title
                };
                title_lbl.set_markup(&format!(
                    "<b>{}</b>",
                    glib::markup_escape_text(&truncate(title_text, 50))
                ));
                title_lbl.set_halign(Align::Start);
                title_lbl.set_xalign(0.0);
                row_box.append(&title_lbl);

                let subtitle_parts: Vec<&str> = [e.organization.as_deref(), e.date.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect();
                if !subtitle_parts.is_empty() {
                    let sub_lbl = Label::new(Some(&truncate(&subtitle_parts.join(" · "), 50)));
                    sub_lbl.set_halign(Align::Start);
                    sub_lbl.set_xalign(0.0);
                    sub_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                    sub_lbl.add_css_class("dim-label");
                    sub_lbl.add_css_class("caption");
                    row_box.append(&sub_lbl);
                }
            }
        }

        let key_lbl = Label::new(Some(entry.key()));
        key_lbl.set_halign(Align::Start);
        key_lbl.set_xalign(0.0);
        key_lbl.add_css_class("caption");
        key_lbl.add_css_class("dim-label");
        row_box.append(&key_lbl);

        row.set_child(Some(&row_box));
        self.list_box.append(&row);
    }
}

fn scroll_row_into_view(scroll: &ScrolledWindow, list_box: &ListBox, row: &ListBoxRow) {
    let adj = scroll.vadjustment();
    if let Some(bounds) = row.compute_bounds(list_box) {
        let row_top = bounds.y() as f64;
        let row_bottom = row_top + bounds.height() as f64;
        let page_top = adj.value();
        let page_bottom = page_top + adj.page_size();
        if row_top < page_top {
            adj.set_value(row_top);
        } else if row_bottom > page_bottom {
            adj.set_value(row_bottom - adj.page_size());
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max - 1).collect();
        format!("{t}\u{2026}")
    }
}

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::gdk::Rectangle;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, Popover, PositionType,
    ScrolledWindow, SelectionMode, Separator,
};

use crate::lsp::CompletionItem;

#[derive(Clone)]
pub struct LspPopup {
    popover: Popover,
    list_box: ListBox,
    scroll: ScrolledWindow,
    /// Everything on offer, in arrival order.
    items: Rc<RefCell<Vec<CompletionItem>>>,
    /// What the list currently shows: the matches for `filter_prefix`, best
    /// first. Row N is `shown[N]` — there are no hidden rows, so a row index can
    /// never point at something the user can't see.
    shown: Rc<RefCell<Vec<CompletionItem>>>,
    filter_prefix: Rc<RefCell<String>>,
    on_complete: Rc<RefCell<Option<Box<dyn Fn(CompletionItem)>>>>,
    on_selection_changed: Rc<RefCell<Option<Box<dyn Fn(Option<CompletionItem>)>>>>,
    /// The name last chosen for the current prefix, if any. Whatever the
    /// ranking thinks, a name the user has picked for this prefix before is the
    /// one they mean — the same reasoning as VS Code's recentlyUsedByPrefix.
    preferred: Rc<RefCell<Option<String>>>,
    /// Names already used in the document. `#col` means something different in
    /// a file that already calls `#columns` than in a fresh one (VS Code calls
    /// this a locality bonus).
    local_names: Rc<RefCell<std::collections::HashSet<String>>>,
    /// Matches before truncation, so the footer can admit when it's showing a
    /// slice: "8 of 137" tells you to keep typing, "3 of 3" to arrow down.
    total_matches: Rc<Cell<usize>>,
    footer: Label,
}

impl LspPopup {
    pub fn new(parent: &impl IsA<gtk4::Widget>) -> Self {
        let popover = Popover::new();
        popover.set_has_arrow(false);
        popover.set_autohide(false);
        popover.set_parent(parent);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Browse);
        list_box.set_activate_on_single_click(true);

        // Wide enough for a name and a one-line description, tall enough for
        // eight of them. The previous 300×180 was small in the wrong way: rows
        // wrapped their description over three lines, so two entries filled the
        // box and everything else was behind a scrollbar. Restraint has to come
        // from showing few things briefly, not from clipping what's shown —
        // every editor that does this well (VS Code, nvim-cmp, Zed) uses
        // single-line rows and moves the prose elsewhere.
        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&list_box));
        scroll.set_min_content_width(430);
        scroll.set_max_content_width(430);
        scroll.set_min_content_height(28);
        scroll.set_max_content_height(8 * 26);
        scroll.set_propagate_natural_height(true);

        // One caption-sized line of keys. The old footer was a full-size label
        // that made the popup feel like a dialog; this is small enough to read
        // as chrome, but the keys still need saying somewhere.
        let hint = Label::new(Some(FOOTER_KEYS));
        hint.add_css_class("dim-label");
        hint.add_css_class("caption");
        hint.set_margin_top(3);
        hint.set_margin_bottom(1);
        hint.set_margin_start(10);
        hint.set_margin_end(10);
        hint.set_xalign(0.0);

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.set_margin_top(2);
        outer.set_margin_bottom(2);
        outer.append(&scroll);
        outer.append(&Separator::new(Orientation::Horizontal));
        outer.append(&hint);
        popover.set_child(Some(&outer));

        let items: Rc<RefCell<Vec<CompletionItem>>> = Rc::new(RefCell::new(Vec::new()));
        let filter_prefix: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let on_complete: Rc<RefCell<Option<Box<dyn Fn(CompletionItem)>>>> =
            Rc::new(RefCell::new(None));

        let shown: Rc<RefCell<Vec<CompletionItem>>> = Rc::new(RefCell::new(Vec::new()));
        let on_selection_changed: Rc<RefCell<Option<Box<dyn Fn(Option<CompletionItem>)>>>> =
            Rc::new(RefCell::new(None));

        let p = Self {
            popover, list_box, scroll, items, shown, filter_prefix, on_complete,
            on_selection_changed,
            preferred: Rc::new(RefCell::new(None)),
            local_names: Rc::new(RefCell::new(std::collections::HashSet::new())),
            total_matches: Rc::new(Cell::new(0)),
            footer: hint,
        };

        // Double-click (or Enter key on the list) triggers completion
        {
            let shown2 = p.shown.clone();
            let cb2 = p.on_complete.clone();
            p.list_box.connect_row_activated(move |_, row| {
                let idx = row.index() as usize;
                if let Some(item) = shown2.borrow().get(idx).cloned() {
                    if let Some(f) = cb2.borrow().as_ref() { f(item); }
                }
            });
        }

        // Moving through the list re-describes the highlighted entry, so the
        // status line always explains what's actually selected.
        {
            let shown3 = p.shown.clone();
            let cb3 = p.on_selection_changed.clone();
            p.list_box.connect_row_selected(move |_, row| {
                let item = row.and_then(|r| shown3.borrow().get(r.index() as usize).cloned());
                if let Some(f) = cb3.borrow().as_ref() { f(item); }
            });
        }

        p
    }

    pub fn set_on_complete(&self, f: impl Fn(CompletionItem) + 'static) {
        *self.on_complete.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_selection_changed(&self, f: impl Fn(Option<CompletionItem>) + 'static) {
        *self.on_selection_changed.borrow_mut() = Some(Box::new(f));
    }

    /// The name previously chosen for the prefix being typed, if any.
    pub fn set_preferred_name(&self, name: Option<String>) {
        *self.preferred.borrow_mut() = name;
    }

    /// Names that already appear in the document being edited.
    pub fn set_local_names(&self, names: std::collections::HashSet<String>) {
        *self.local_names.borrow_mut() = names;
    }

    /// Replace the popup contents with a new master item list, without showing
    /// anything. Resets any active filter — call `apply_filter` afterwards.
    pub fn load_items(&self, mut new_items: Vec<CompletionItem>) {
        *self.filter_prefix.borrow_mut() = String::new();

        if new_items.is_empty() {
            if self.popover.is_visible() {
                self.popover.popdown();
            }
            *self.items.borrow_mut() = Vec::new();
            self.rebuild_rows(Vec::new(), "");
            return;
        }

        new_items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        *self.items.borrow_mut() = new_items;
        self.apply_filter("");
    }

    /// Show the already-loaded list at (x, y). `above`: true = the popup sits
    /// above the cursor (PositionType::Top), false = below.
    pub fn show_at(&self, x: i32, y: i32, above: bool) {
        if self.shown.borrow().is_empty() { return; }
        self.popover.set_position(if above { PositionType::Top } else { PositionType::Bottom });
        self.popover.set_pointing_to(Some(&Rectangle::new(x, y, 1, 1)));
        if !self.popover.is_visible() {
            self.popover.popup();
        }
    }

    /// Re-filter to `prefix` and rebuild the rows in best-match-first order.
    /// Safe to call while the popup is visible.
    pub fn apply_filter(&self, prefix: &str) {
        let lprefix = prefix.to_lowercase();
        *self.filter_prefix.borrow_mut() = lprefix.clone();

        let mut scored: Vec<(NameMatch, CompletionItem)> = self
            .items
            .borrow()
            .iter()
            .filter_map(|item| match_name(&item.label, &lprefix).map(|m| (m, item.clone())))
            .collect();
        // Best first: what the user chose here last time, then whether the name
        // is already used in this document, then how the query matched, where,
        // and finally the shortest name (an exact-ish match beats a long name
        // that merely contains the query).
        let preferred = self.preferred.borrow().clone();
        let local = self.local_names.borrow();
        let key = |m: &NameMatch, item: &CompletionItem| {
            (
                preferred.as_deref() != Some(item.label.as_str()),
                !local.contains(&item.label),
                m.rank,
                m.start,
                item.label.chars().count(),
                item.label.to_lowercase(),
            )
        };
        scored.sort_by(|a, b| key(&a.0, &a.1).cmp(&key(&b.0, &b.1)));
        drop(local);

        self.total_matches.set(scored.len());
        scored.truncate(MAX_ROWS);

        self.update_footer(self.total_matches.get(), scored.len());

        let matches: Vec<(NameMatch, CompletionItem)> = scored;
        let items: Vec<CompletionItem> = matches.iter().map(|(_, i)| i.clone()).collect();
        let highlights: Vec<Vec<usize>> = matches.into_iter().map(|(m, _)| m.positions).collect();
        self.rebuild_rows_with(items, highlights);
    }

    /// "8 of 137 · keys" when the list is a slice of the matches, plain keys
    /// when it isn't. Being shown 8 of 137 is the cue to type another letter
    /// rather than reach for the arrow keys.
    fn update_footer(&self, total: usize, shown: usize) {
        if total > shown {
            self.footer.set_text(&format!("{shown} of {total} · {FOOTER_KEYS}"));
        } else {
            self.footer.set_text(FOOTER_KEYS);
        }
    }

    fn rebuild_rows(&self, items: Vec<CompletionItem>, _prefix: &str) {
        self.rebuild_rows_with(items, Vec::new());
    }

    fn rebuild_rows_with(&self, items: Vec<CompletionItem>, highlights: Vec<Vec<usize>>) {
        self.clear_rows();
        for (idx, item) in items.iter().enumerate() {
            let empty = Vec::new();
            self.append_row(item, highlights.get(idx).unwrap_or(&empty));
        }
        *self.shown.borrow_mut() = items;
        if let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.select_row(Some(&row));
        } else {
            self.list_box.select_row(None::<&ListBoxRow>);
        }
    }

    /// Merge additional items into the master list (dedup by name), keeping the
    /// current filter. Used when LSP results arrive after the popup was already
    /// showing local snippets.
    pub fn merge_items(&self, new_items: Vec<CompletionItem>) {
        let any_new = {
            let existing = self.items.borrow();
            new_items.iter().any(|ni| !existing.iter().any(|ei| ei.label == ni.label))
        };
        if !any_new { return; }

        {
            let mut all = self.items.borrow_mut();
            for item in new_items {
                if !all.iter().any(|ei| ei.label == item.label) {
                    all.push(item);
                }
            }
        }
        let prefix = self.filter_prefix.borrow().clone();
        self.apply_filter(&prefix);
    }

    /// The item the inline ghost suggestion should offer for `prefix`: the
    /// shortest name that starts with it, so `#e` suggests `emph` rather than
    /// whatever happens to sort first alphabetically. Prefix-only by design —
    /// ghost text is drawn as a continuation of what's already typed.
    pub fn best_match(&self, prefix: &str) -> Option<CompletionItem> {
        if prefix.is_empty() { return None; }
        let lprefix = prefix.to_lowercase();
        let items = self.items.borrow();
        let mut candidates = items
            .iter()
            .filter(|i| i.label.to_lowercase().starts_with(&lprefix));
        // A name chosen for this prefix before wins outright — the ghost is a
        // prediction, and the best evidence for it is what happened last time.
        if let Some(pref) = self.preferred.borrow().as_deref() {
            if let Some(hit) = candidates.clone().find(|i| i.label == pref) {
                return Some(hit.clone());
            }
        }
        let local = self.local_names.borrow();
        candidates
            .min_by_key(|i| {
                (!local.contains(&i.label), i.label.chars().count(), i.label.to_lowercase())
            })
            .cloned()
    }

    /// The item to describe when there's no ghost to show: the best match on
    /// any of the weaker rankings, so `#break` can still say what `pagebreak` does.
    pub fn describable_match(&self, prefix: &str) -> Option<CompletionItem> {
        self.best_match(prefix)
            .or_else(|| self.shown.borrow().first().cloned())
    }

    pub fn match_count(&self, prefix: &str) -> usize {
        let lprefix = prefix.to_lowercase();
        if lprefix.is_empty() {
            return self.items.borrow().len();
        }
        self.items
            .borrow()
            .iter()
            .filter(|i| match_name(&i.label, &lprefix).is_some())
            .count()
    }

    pub fn hide(&self) {
        if self.popover.is_visible() {
            self.popover.popdown();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.popover.is_visible()
    }

    pub fn selected_item(&self) -> Option<CompletionItem> {
        let row = self.list_box.selected_row()?;
        self.shown.borrow().get(row.index() as usize).cloned()
    }

    pub fn first_item(&self) -> Option<CompletionItem> {
        self.shown.borrow().first().cloned()
    }

    pub fn move_selection(&self, delta: i32) {
        let count = self.shown.borrow().len() as i32;
        if count == 0 { return; }
        let current = self.list_box.selected_row().map(|r| r.index()).unwrap_or(0);
        let next = (current + delta).clamp(0, count - 1);
        if let Some(row) = self.list_box.row_at_index(next) {
            self.list_box.select_row(Some(&row));
            scroll_row_into_view(&self.scroll, &self.list_box, &row);
        }
    }

    fn clear_rows(&self) {
        while let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.remove(&row);
        }
    }

    /// One line per entry: name (with the matched characters emboldened, as
    /// VS Code does), then the description, ellipsized rather than wrapped, then
    /// the kind. Wrapping was what made the box feel cramped — a row that can
    /// grow to three lines turns eight visible entries into two.
    fn append_row(&self, item: &CompletionItem, highlight: &[usize]) {
        let row = ListBoxRow::new();
        row.set_activatable(true);

        let row_box = GtkBox::new(Orientation::Horizontal, 8);
        row_box.set_margin_top(3);
        row_box.set_margin_bottom(3);
        row_box.set_margin_start(10);
        row_box.set_margin_end(10);

        let name_lbl = Label::new(None);
        name_lbl.set_markup(&highlighted_markup(&item.label, highlight));
        name_lbl.set_halign(Align::Start);
        name_lbl.set_xalign(0.0);
        name_lbl.set_valign(Align::Center);
        name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        row_box.append(&name_lbl);

        if let Some(ref detail) = item.detail {
            let detail_lbl = Label::new(Some(detail));
            detail_lbl.set_halign(Align::Start);
            detail_lbl.set_xalign(0.0);
            detail_lbl.set_valign(Align::Center);
            detail_lbl.set_hexpand(true);
            detail_lbl.add_css_class("dim-label");
            detail_lbl.add_css_class("caption");
            detail_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            row_box.append(&detail_lbl);
        }

        let kind_str = kind_label(item.kind);
        if !kind_str.is_empty() {
            let kind_lbl = Label::new(Some(kind_str));
            kind_lbl.add_css_class("dim-label");
            kind_lbl.add_css_class("caption");
            kind_lbl.set_halign(Align::End);
            kind_lbl.set_valign(Align::Center);
            if item.detail.is_none() {
                kind_lbl.set_hexpand(true);
            }
            row_box.append(&kind_lbl);
        }

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

const FOOTER_KEYS: &str = "↑↓ select · Tab insert · Esc dismiss";

/// Longest list the popup will build in one go. Beyond this the extra rows are
/// unreachable in practice — the user narrows the query instead of scrolling.
const MAX_ROWS: usize = 50;

/// Shortest query allowed to match by loose subsequence. One or two characters
/// match nearly everything that way, which is how a list becomes noise.
const MIN_SUBSEQUENCE_QUERY: usize = 3;

pub struct NameMatch {
    /// 0 = prefix, 1 = word start, 2 = anywhere in the name, 3 = subsequence.
    pub rank: u8,
    /// Character index of the first matched character; earlier is better.
    pub start: usize,
    /// Character indices to embolden in the row.
    pub positions: Vec<usize>,
}

/// Match `query` (already lowercase) against a completion's *name* only.
///
/// Names, never descriptions. Matching descriptions as well seemed generous —
/// `#quote` finding the block snippet — but it mostly produced matches with no
/// visible cause: typing `#column` offered `dropcap`, because "decorative"
/// contains "co". A suggestion the user can't connect to what they typed reads
/// as the editor being wrong, and there's nowhere in a one-line row to show
/// that the match came from prose they can't see. VS Code draws the same line:
/// it filters on the item's word (or an explicit filterText), and treats
/// documentation as something to display, not to search.
pub fn match_name(name: &str, query: &str) -> Option<NameMatch> {
    if query.is_empty() {
        return Some(NameMatch { rank: 0, start: 0, positions: Vec::new() });
    }
    let lname: Vec<char> = name.to_lowercase().chars().collect();
    let q: Vec<char> = query.chars().collect();

    let find_at = |from: usize| -> Option<usize> {
        if q.len() > lname.len() { return None; }
        (from..=lname.len() - q.len()).find(|&i| lname[i..i + q.len()] == q[..])
    };

    if let Some(at) = find_at(0) {
        let positions: Vec<usize> = (at..at + q.len()).collect();
        if at == 0 {
            return Some(NameMatch { rank: 0, start: 0, positions });
        }
        // A run starting at a word boundary (`page-break`, `pageBreak`) is a
        // deliberate-looking match; one starting mid-word is weaker but real.
        let prev = lname[at - 1];
        let boundary = !prev.is_alphanumeric()
            || name.chars().nth(at).is_some_and(|c| c.is_uppercase());
        return Some(NameMatch { rank: if boundary { 1 } else { 2 }, start: at, positions });
    }

    if q.len() < MIN_SUBSEQUENCE_QUERY {
        return None;
    }
    let mut positions = Vec::with_capacity(q.len());
    let mut qi = 0;
    for (i, c) in lname.iter().enumerate() {
        if qi < q.len() && *c == q[qi] {
            positions.push(i);
            qi += 1;
        }
    }
    if qi == q.len() {
        let start = positions[0];
        Some(NameMatch { rank: 3, start, positions })
    } else {
        None
    }
}

/// Pango markup for a name with its matched characters in bold.
fn highlighted_markup(name: &str, positions: &[usize]) -> String {
    let mut out = String::with_capacity(name.len() + positions.len() * 7);
    for (i, ch) in name.chars().enumerate() {
        let escaped = glib::markup_escape_text(&ch.to_string()).to_string();
        if positions.contains(&i) {
            out.push_str("<b>");
            out.push_str(&escaped);
            out.push_str("</b>");
        } else {
            out.push_str(&escaped);
        }
    }
    out
}

fn kind_label(kind: u8) -> &'static str {
    match kind {
        2  => "Method",
        3  => "Function",
        4  => "Constructor",
        5  => "Field",
        6  => "Variable",
        7  => "Class",
        8  => "Interface",
        9  => "Module",
        10 => "Property",
        12 => "Value",
        13 => "Enum",
        14 => "Keyword",
        15 => "Snippet",
        _  => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rank(name: &str, query: &str) -> Option<u8> {
        match_name(name, query).map(|m| m.rank)
    }

    #[test]
    fn prefix_beats_everything_else() {
        assert_eq!(rank("pagebreak", "page"), Some(0));
        assert_eq!(rank("pagebreak", "break"), Some(2));
        assert_eq!(rank("page-break", "break"), Some(1));
        assert_eq!(rank("pageBreak", "break"), Some(1));
    }

    #[test]
    fn subsequence_needs_a_real_query() {
        // "cl" is a subsequence of "colbreak" but far too short to mean it.
        assert_eq!(rank("colbreak", "cl"), None);
        assert_eq!(rank("colbreak", "clbrk"), Some(3));
    }

    #[test]
    fn unrelated_names_do_not_match() {
        // The bug this guards: `#column` used to surface `dropcap`, because
        // matching also read the description ("decorative" contains "co").
        assert_eq!(rank("dropcap", "column"), None);
        assert_eq!(rank("dropcap", "col"), None);
        assert_eq!(rank("dropcap", "co"), None);
        assert_eq!(rank("outline", "column"), None);
    }

    #[test]
    fn positions_mark_what_matched() {
        let m = match_name("pagebreak", "break").unwrap();
        assert_eq!(m.positions, vec![4, 5, 6, 7, 8]);
        assert_eq!(m.start, 4);
    }

    #[test]
    fn empty_query_matches_all() {
        assert_eq!(rank("anything", ""), Some(0));
    }
}

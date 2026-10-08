//! Inline completion: citation and language-server suggestions, ghost text and the popups that carry them.

use super::*;

// ── Built-in academic snippets ────────────────────────────────────────────────
// (match_key, display_label, insert_text_with_leading_#)
// (match_key, display_label, description, insert_text)
pub(super) const ACADEMIC_SNIPPETS: &[(&str, &str, &str, &str)] = &[
    ("figure", "Figure",
     "Image with caption and cross-reference label",
     "#figure(\n  image(\"\", width: 80%),\n  caption: [Caption text],\n) <fig:label>"),
    ("table", "Table",
     "Table with a header row and cross-reference label",
     "#figure(\n  table(\n    columns: (auto, auto),\n    table.header([*Column 1*], [*Column 2*]),\n    [Cell 1], [Cell 2],\n  ),\n  caption: [Table title],\n) <tab:label>"),
    ("footnote", "Footnote",
     "Inline footnote — appears at the bottom of the page",
     "#footnote[Note text]"),
    ("bibliography", "Bibliography",
     "Bibliography section from a .bib file",
     "#bibliography(\"refs.bib\")"),
    ("pagebreak", "Page break",
     "Force content to start on a new page",
     "#pagebreak()"),
    ("line", "Horizontal rule",
     "A line across the full width of the page",
     "#line(length: 100%)"),
    ("outline", "Table of Contents",
     "Auto-generated table of contents (headings up to depth 3)",
     "#outline(title: [Contents], depth: 3)"),
    ("lorem", "Lorem ipsum",
     "100 words of placeholder text",
     "#lorem(100)"),
    ("set", "Set rule",
     "Change text size and font for the rest of the document",
     "#set text(size: 11pt, font: \"Liberation Serif\")"),
    ("show", "Show rule",
     "Transform how an element is displayed (example: bold headings)",
     "#show heading: it => strong(it)"),
    ("block", "Block / quote",
     "Indented block — use for block quotations",
     "#block(inset: (left: 2em))[\n  Quoted text\n]"),
    ("dropcap", "Drop cap",
     "Large decorative first letter. Requires Droplet enabled in template settings → Packages.",
     "#dropcap[\n  First paragraph text here.\n]"),
];

pub(super) const CV_SNIPPETS: &[(&str, &str, &str, &str)] = &[
    ("job", "#job",
     "Work experience entry — title, company, years, description",
     "#job(\n  \"Job Title\",\n  \"Company Name\",\n  \"2022\u{2013}present\",\n  [Description of role and key accomplishments.]\n)"),
    ("edu", "#edu",
     "Education entry — degree, institution, years",
     "#edu(\n  \"Degree\",\n  \"Institution Name\",\n  \"2016\u{2013}2020\",\n)"),
    ("skill", "#skill",
     "Skills category row",
     "#skill(\"Languages\", (\"Rust\", \"Python\", \"Kotlin\"))"),
    ("section", "#section",
     "CV section — heading + content block",
     "#section(\"Section Title\")[\n  \n]"),
    ("award", "#award",
     "Award or honour entry — title, organisation, year, optional description",
     "#award(\n  \"Award Name\",\n  \"Awarding Organisation\",\n  \"2023\",\n  desc: [Brief description.]\n)"),
];

/// State the `@`/`!`-trigger citation autocomplete produces in `open_file`,
/// consumed by the LSP autocomplete, key controller, right-click menu, and
/// final `EditorTab` construction sections below it.
pub(super) struct CitationAutocomplete {
    pub(super) bib_popup: BibPopup,
    pub(super) ac_mark: Rc<RefCell<Option<gtk4::TextMark>>>,
    pub(super) completing: Rc<RefCell<bool>>,
    pub(super) ghost_label: Label,
    pub(super) ghost_item: Rc<RefCell<Option<CompletionItem>>>,
    pub(super) completion_suppressed_at: Rc<Cell<i32>>,
    pub(super) ghost_bib_entry: Rc<RefCell<Option<PopupEntry>>>,
}

/// State the `#`-trigger LSP autocomplete produces in `open_file`, consumed
/// by the key controller and right-click menu sections below it.
pub(super) struct LspAutocomplete {
    pub(super) lsp_popup: LspPopup,
    pub(super) lsp_mark: Rc<RefCell<Option<gtk4::TextMark>>>,
    pub(super) lsp_completing: Rc<RefCell<bool>>,
}

/// Built-in snippets as completion items. The item's name is the Typst
/// identifier — `pagebreak`, `outline` — not the human title, because that's
/// what gets typed after `#` and what the ghost text has to continue; matching
/// on the title meant `#pagebreak` found nothing while `#page break` was
/// unsayable. The title leads the description instead, so the list still reads
/// as "Page break — force content to start on a new page".
pub(super) fn snippet_items(cv_mode: bool) -> Vec<CompletionItem> {
    let source = if cv_mode {
        CV_SNIPPETS
    } else {
        ACADEMIC_SNIPPETS
    };
    source
        .iter()
        .map(|(name, title, desc, body)| {
            let title = title.trim_start_matches('#');
            // Skip the title when it's just the name respaced ("Page break" for
            // `pagebreak`) — repeating it reads as two dashes and no content.
            // Drop the title when the description already says it, so the hint
            // doesn't read "outline — Table of Contents — Auto-generated table
            // of contents".
            let redundant = title.replace(' ', "").eq_ignore_ascii_case(name)
                || desc.to_lowercase().contains(&title.to_lowercase());
            let detail = if redundant {
                desc.to_string()
            } else {
                format!("{title} — {desc}")
            };
            CompletionItem {
                label: name.to_string(),
                kind: 15,
                detail: Some(detail),
                insert_text: Some(body.to_string()),
            }
        })
        .collect()
}

/// Number of characters typed after `#` before the completion *list* joins the
/// inline ghost suggestion. At one character everything still matches, so the
/// list would just be a wall of options over the text being written.
pub(super) const MIN_POPUP_PREFIX: usize = 2;

/// Draw `item`'s remaining characters as dim ghost text right after the cursor,
/// or hide the ghost when there's nothing left to suggest.
/// One-line preview of what an item will actually insert: newlines and runs of
/// whitespace collapsed, cut to something that fits after the cursor.
///
/// The ghost used to show only the rest of the *name*, which made Tab a leap of
/// faith — `#fig` + Tab lands eight lines of figure scaffolding, and nothing
/// said so beforehand.
pub(super) fn insertion_preview(item: &CompletionItem, prefix: &str) -> Option<String> {
    let raw = item.insert_text.as_deref().unwrap_or(&item.label);
    let flat = flatten_snippet(raw);
    let flat = flat.trim_start_matches('#');
    // Only usable as ghost text if it continues what's already typed.
    let rest = flat.strip_prefix(prefix).or_else(|| {
        let lower = flat.to_lowercase();
        lower.starts_with(prefix).then(|| &flat[prefix.len()..])
    })?;
    if rest.is_empty() {
        return None;
    }
    const MAX: usize = 56;
    if rest.chars().count() > MAX {
        let cut: String = rest.chars().take(MAX - 1).collect();
        Some(format!("{}…", cut.trim_end()))
    } else {
        Some(rest.to_string())
    }
}

/// A multi-line snippet body as one readable line: indentation collapsed, and
/// no gaps left hanging inside brackets ("figure( image" reads as a typo).
pub(super) fn flatten_snippet(raw: &str) -> String {
    let joined = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    joined
        .replace("( ", "(")
        .replace(" )", ")")
        .replace("[ ", "[")
        .replace(" ]", "]")
        .replace(" ,", ",")
}

/// The signature line for an item, when it has one. Language-server items carry
/// the real signature in `detail`; a built-in snippet's is derived from what it
/// inserts. Mid-line, `figure(body, caption: [..])` answers more than a
/// sentence of prose does.
pub(super) fn item_signature(item: &CompletionItem) -> Option<String> {
    if let Some(detail) = item.detail.as_deref() {
        if detail.contains('(') && !detail.contains(" — ") {
            return Some(detail.to_string());
        }
    }
    let flat = flatten_snippet(item.insert_text.as_deref()?);
    let flat = flat.trim_start_matches('#').to_string();
    flat.contains('(').then_some(flat)
}

pub(super) fn set_ghost(
    view: &View,
    ghost: &Label,
    slot: &Rc<RefCell<Option<CompletionItem>>>,
    hint: &Label,
    buf: &Buffer,
    item: Option<CompletionItem>,
    prefix: &str,
) {
    let remainder = item.as_ref().and_then(|i| {
        insertion_preview(i, prefix).or_else(|| {
            i.label
                .get(prefix.len()..)
                .filter(|r| !r.is_empty())
                .map(str::to_string)
        })
    });
    let Some(remainder) = remainder else {
        clear_ghost(ghost, slot, hint);
        return;
    };
    let cursor = buf.iter_at_offset(buf.cursor_position());
    // The ghost is drawn over the view, so it would cover whatever follows the
    // cursor. Only offer it when the rest of the line is empty.
    {
        let mut line_end = cursor;
        if !line_end.ends_line() {
            line_end.forward_to_line_end();
        }
        if !buf.text(&cursor, &line_end, false).trim().is_empty() {
            clear_ghost(ghost, slot, hint);
            return;
        }
    }
    let loc = view.iter_location(&cursor);
    ghost.set_text(&remainder);
    view.move_overlay(ghost, loc.x(), loc.y());
    ghost.set_visible(true);
    *slot.borrow_mut() = item;
}

/// Citation ghost: the rest of the key drawn after what's typed. Kept separate
/// from the `#` ghost's slot so Tab knows which kind of completion it's taking.
pub(super) fn set_citation_ghost(
    view: &View,
    ghost: &Label,
    slot: &Rc<RefCell<Option<crate::ui::bib_popup::PopupEntry>>>,
    hint: &Label,
    buf: &Buffer,
    entry: Option<crate::ui::bib_popup::PopupEntry>,
    query: &str,
) {
    let remainder = entry.as_ref().and_then(|e| {
        e.key_text()
            .get(query.len()..)
            .filter(|r| !r.is_empty())
            .map(str::to_string)
    });
    let Some(remainder) = remainder else {
        clear_citation_ghost(ghost, slot, hint);
        return;
    };
    let cursor = buf.iter_at_offset(buf.cursor_position());
    let mut line_end = cursor;
    if !line_end.ends_line() {
        line_end.forward_to_line_end();
    }
    if !buf.text(&cursor, &line_end, false).trim().is_empty() {
        clear_citation_ghost(ghost, slot, hint);
        return;
    }
    let loc = view.iter_location(&cursor);
    ghost.set_text(&remainder);
    view.move_overlay(ghost, loc.x(), loc.y());
    ghost.set_visible(true);
    *slot.borrow_mut() = entry;
}

pub(super) fn clear_citation_ghost(
    ghost: &Label,
    slot: &Rc<RefCell<Option<crate::ui::bib_popup::PopupEntry>>>,
    hint: &Label,
) {
    ghost.set_visible(false);
    *slot.borrow_mut() = None;
    hint.set_text("");
}

/// Take down the inline suggestion and the status line together — they're one
/// affordance, and a hint left behind describes something no longer on offer.
pub(super) fn clear_ghost(ghost: &Label, slot: &Rc<RefCell<Option<CompletionItem>>>, hint: &Label) {
    ghost.set_visible(false);
    *slot.borrow_mut() = None;
    hint.set_text("");
}

/// The status-bar line that says what the current suggestion is for and which
/// key takes it. The ghost alone shows *that* something is on offer but not
/// what it does — and the status bar can carry a sentence without covering a
/// single character of the document.
///
/// `has_list` distinguishes the two stages: with a list open, arrows and Escape
/// are live too.
/// Shared shape for both completion hints: **name** — what it is · keys.
pub(super) fn completion_hint_markup(
    name: &str,
    what: &str,
    has_ghost: bool,
    has_list: bool,
) -> String {
    let what = if what.chars().count() > 46 {
        let cut: String = what.chars().take(45).collect();
        format!("{}…", cut.trim_end())
    } else {
        what.to_string()
    };
    // The status bar shares its row with the word count and the rest — spelling
    // the keys out in full pushed the description off the end.
    let keys = match (has_ghost, has_list) {
        (_, true) => "Tab insert · ↑↓ select · Esc",
        (true, false) => "Tab insert · Esc",
        (false, false) => "Esc dismiss",
    };
    format!(
        "<b>{}</b> — {}   ·   {}",
        glib::markup_escape_text(name),
        glib::markup_escape_text(&what),
        keys,
    )
}

/// Citation/CV equivalent of `set_completion_hint`: same line, same keys, so
/// `@` and `#` behave alike rather than one of them being the polished half.
pub(super) fn set_citation_hint(
    hint: &Label,
    entry: Option<&crate::ui::bib_popup::PopupEntry>,
    has_ghost: bool,
    has_list: bool,
) {
    match entry {
        Some(e) => hint.set_markup(&completion_hint_markup(
            &e.key_text(),
            &e.describe(),
            has_ghost,
            has_list,
        )),
        None => hint.set_text(""),
    }
}

pub(super) fn set_completion_hint(
    hint: &Label,
    item: Option<&CompletionItem>,
    prefix: &str,
    has_ghost: bool,
    has_list: bool,
    lsp_ready: bool,
) {
    // Without a language server the only completions are the handful of
    // built-in snippets. Saying so turns "why is nothing offered?" into a
    // fact about the setup — the startup log said it, where nobody looks.
    // Only said where the line has room: when something is being described, the
    // description earns the space, and this would just be truncated away.
    let scope = if lsp_ready {
        ""
    } else {
        "   ·   built-in snippets only (tinymist not running)"
    };
    let text = match item {
        Some(item) => {
            // Signature first when there is one: mid-line, the argument list is
            // what you need. Prose is the fallback, trimmed so the keys at the
            // end survive.
            let what = item_signature(item)
                .or_else(|| item.detail.clone())
                .unwrap_or_else(|| item.label.clone());
            completion_hint_markup(&item.label, &what, has_ghost, has_list)
        }
        // A bare `#` matches everything, so there's nothing to describe yet —
        // say what to do instead, which is the moment the question arises.
        None if prefix.is_empty() => {
            format!("Typst function — keep typing to search, Tab takes the suggestion{scope}")
        }
        None => String::new(),
    };
    hint.set_markup(&text);
}

/// Names the document already invokes with `#`. Used as a ranking bonus: in a
/// file that already calls `#columns`, `#col` most likely means that again.
pub(super) fn names_used_in(buf: &Buffer) -> std::collections::HashSet<String> {
    let (start, end) = buf.bounds();
    let text = buf.text(&start, &end, false);
    let mut names = std::collections::HashSet::new();
    let mut rest = text.as_str();
    while let Some(at) = rest.find('#') {
        rest = &rest[at + 1..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if !name.is_empty() {
            names.insert(name);
        }
    }
    names
}

/// Note the `#` the cursor is inside, so suggestions for it stay dismissed.
pub(super) fn suppress_current_completion(
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
    slot: &Rc<Cell<i32>>,
) {
    let offset = mark
        .borrow()
        .as_ref()
        .map(|m| buf.iter_at_mark(m).offset())
        .unwrap_or(-1);
    slot.set(offset);
}

pub(super) fn lsp_hash_prefix(buffer: &Buffer) -> String {
    let cursor = buffer.iter_at_offset(buffer.cursor_position());
    let mut temp = cursor;
    loop {
        if !temp.backward_char() {
            break;
        }
        let ch = temp.char();
        if ch == '#' {
            return buffer
                .text(&temp, &cursor, false)
                .to_string()
                .trim_start_matches('#')
                .to_lowercase();
        }
        if !(ch.is_alphanumeric() || ch == '_' || ch == '-') {
            break;
        }
    }
    String::new()
}

pub(super) fn strip_snippets(s: &str) -> String {
    // Remove LSP snippet placeholders: $0, $1, ${1:...}, etc.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' {
            match chars.peek() {
                Some('{') => {
                    chars.next(); // consume '{'
                                  // skip until matching '}'
                    for c in chars.by_ref() {
                        if c == '}' {
                            break;
                        }
                    }
                }
                Some(c) if c.is_ascii_digit() => {
                    // consume digits
                    while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        chars.next();
                    }
                }
                _ => out.push(ch),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// True if `ln` is a Typst heading line (starts with `=`).
/// The text of the current paragraph up to `cursor` — how far back it is worth
/// looking for a bracket or quote that is still open. Capped, so a document
/// with no blank lines can't make a keystroke scan the whole file.
pub(super) fn text_before_in_paragraph(buf: &Buffer, cursor: &gtk4::TextIter) -> String {
    const MAX_LINES: i32 = 60;
    let cursor_line = cursor.line();
    let mut first = cursor_line;
    while first > 0 && cursor_line - first < MAX_LINES {
        let Some(prev) = buf.iter_at_line(first - 1) else {
            break;
        };
        let mut end = prev;
        if !end.ends_line() {
            end.forward_to_line_end();
        }
        if buf.text(&prev, &end, false).trim().is_empty() {
            break;
        }
        first -= 1;
    }
    let start = buf.iter_at_line(first).unwrap_or_else(|| buf.start_iter());
    buf.text(&start, cursor, false).to_string()
}

pub(super) fn dismiss_popup(
    buf: &Buffer,
    popup: &BibPopup,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
) {
    if let Some(m) = mark.borrow_mut().take() {
        buf.delete_mark(&m);
    }
    popup.hide();
}

pub(super) fn dismiss_popup_only(
    popup: &BibPopup,
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
) {
    if let Some(m) = mark.borrow_mut().take() {
        buf.delete_mark(&m);
    }
    popup.hide();
}

/// Replaces the text between `mark` and the cursor with `text`, then resets
/// completion state. Shared by `do_bib_complete` and `do_lsp_complete` — the
/// only difference between the two triggers is what `text` ends up being.
pub(super) fn insert_completion_text(
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
    completing: &Rc<RefCell<bool>>,
    view: &View,
    text: &str,
) {
    *completing.borrow_mut() = true;
    let mark_opt = mark.borrow().clone();
    if let Some(ref m) = mark_opt {
        let mut start = buf.iter_at_mark(m);
        let mut end = buf.iter_at_offset(buf.cursor_position());
        buf.begin_user_action();
        buf.delete(&mut start, &mut end);
        buf.insert_at_cursor(text);
        buf.end_user_action();
        buf.delete_mark(m);
    }
    *mark.borrow_mut() = None;
    view.grab_focus();
    *completing.borrow_mut() = false;
}

pub(super) fn do_bib_complete(
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
    completing: &Rc<RefCell<bool>>,
    popup: &BibPopup,
    view: &View,
    entry: &PopupEntry,
) {
    insert_completion_text(buf, mark, completing, view, &entry.insert_text());
    popup.hide();
}

pub(super) fn do_lsp_complete(
    buf: &Buffer,
    mark: &Rc<RefCell<Option<gtk4::TextMark>>>,
    completing: &Rc<RefCell<bool>>,
    popup: &LspPopup,
    view: &View,
    item: CompletionItem,
) {
    let raw = item.insert_text.as_deref().unwrap_or(&item.label);
    let cleaned = strip_snippets(raw);
    let final_text = if cleaned.starts_with('#') {
        cleaned
    } else {
        format!("#{cleaned}")
    };
    insert_completion_text(buf, mark, completing, view, &final_text);
    popup.hide();
}

impl EditorPane {
    /// Called from app_window when a completion response arrives. Shows the
    /// popup on the currently-active tab's view.
    pub fn show_lsp_completions(&self, items: Vec<CompletionItem>) {
        let current = match self.notebook.current_page() {
            Some(p) => p,
            None => return,
        };
        // Collect everything we need from state, then drop the borrow before any
        // GTK widget ops — popup.popup() / show_items can cascade through GTK and
        // fire signals that re-enter Zerkalo callbacks trying borrow_mut on state.
        pub(super) struct TabInfo {
            view: sourceview5::View,
            buffer: sourceview5::Buffer,
            lsp_popup: crate::ui::lsp_popup::LspPopup,
            popup_visible: bool,
            ghost_label: Label,
            ghost_item: Rc<RefCell<Option<CompletionItem>>>,
        }
        let tab_info: Option<TabInfo> = {
            let state = self.state.borrow();
            state
                .tabs
                .values()
                .find(|tab| self.notebook.page_num(&tab.notebook_page) == Some(current))
                .map(|tab| TabInfo {
                    view: tab.view.clone(),
                    buffer: tab.buffer.clone(),
                    lsp_popup: tab.lsp_popup.clone(),
                    popup_visible: tab.lsp_popup.is_visible(),
                    ghost_label: tab.ghost_label.clone(),
                    ghost_item: tab.ghost_item.clone(),
                })
        };
        let Some(ti) = tab_info else { return };

        let prefix = lsp_hash_prefix(&ti.buffer);
        let cursor = ti.buffer.iter_at_offset(ti.buffer.cursor_position());
        let loc = ti.view.iter_location(&cursor);
        let (wx, wy_bottom) = ti.view.buffer_to_window_coords(
            TextWindowType::Widget,
            loc.x(),
            loc.y() + loc.height(),
        );
        let (_, wy_top) = ti
            .view
            .buffer_to_window_coords(TextWindowType::Widget, loc.x(), loc.y());
        let view_h = ti.view.allocated_height();
        let above = wy_bottom > view_h / 2;
        let wy = if above { wy_top } else { wy_bottom };

        if !ti.popup_visible {
            let mut all_items = snippet_items(self.cv_mode.get());
            all_items.extend(items);
            ti.lsp_popup.load_items(all_items);
        } else {
            ti.lsp_popup.merge_items(items);
        }
        ti.lsp_popup.apply_filter(&prefix);

        // Arriving LSP results refine what's on offer; they don't get to open the
        // list on their own before the prefix is worth listing (see MIN_POPUP_PREFIX).
        let list_open =
            prefix.chars().count() >= MIN_POPUP_PREFIX && ti.lsp_popup.match_count(&prefix) > 0;
        if list_open {
            ti.lsp_popup.show_at(wx, wy, above);
        } else {
            ti.lsp_popup.hide();
        }
        let ghosted = ti.lsp_popup.best_match(&prefix);
        set_ghost(
            &ti.view,
            &ti.ghost_label,
            &ti.ghost_item,
            &self.lsp_status_label,
            &ti.buffer,
            ghosted.clone(),
            &prefix,
        );
        set_completion_hint(
            &self.lsp_status_label,
            ti.lsp_popup.describable_match(&prefix).as_ref(),
            &prefix,
            ghosted.is_some(),
            list_open,
            self.lsp_ready.get(),
        );
    }

    pub(super) fn wire_citation_autocomplete(
        &self,
        view: &View,
        buffer: &Buffer,
    ) -> CitationAutocomplete {
        let bib_popup = BibPopup::new(
            view,
            self.bib_entries.clone(),
            self.cv_entries.clone(),
            self.cited_keys.clone(),
        );
        let ac_mark: Rc<RefCell<Option<gtk4::TextMark>>> = Rc::new(RefCell::new(None));
        let completing: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let bib_active_for_open = self.bib_active.clone();

        let buf_complete = buffer.clone();
        let view_complete = view.clone();
        let mark_complete = ac_mark.clone();
        let completing_complete = completing.clone();
        let popup_complete = bib_popup.clone();
        bib_popup.set_on_complete(move |entry| {
            *completing_complete.borrow_mut() = true;
            let mark_opt = mark_complete.borrow().clone();
            if let Some(ref m) = mark_opt {
                let mut start = buf_complete.iter_at_mark(m);
                let mut end = buf_complete.iter_at_offset(buf_complete.cursor_position());
                buf_complete.begin_user_action();
                buf_complete.delete(&mut start, &mut end);
                buf_complete.insert_at_cursor(&entry.insert_text());
                buf_complete.end_user_action();
                buf_complete.delete_mark(m);
            }
            *mark_complete.borrow_mut() = None;
            popup_complete.hide();
            view_complete.grab_focus();
            *completing_complete.borrow_mut() = false;
        });

        // Inline ghost suggestion, fish-shell style: the rest of the best match
        // drawn dim right after the cursor, accepted with Tab. It's an overlay
        // child of the view rather than text in the buffer, so it can't end up
        // saved to the file, counted as words, or sent to the LSP — and because
        // overlay coordinates are buffer coordinates, it scrolls with the text
        // for free.
        let ghost_label = Label::new(None);
        ghost_label.add_css_class("completion-ghost");
        ghost_label.set_visible(false);
        ghost_label.set_can_target(false);
        view.add_overlay(&ghost_label, 0, 0);
        let ghost_item: Rc<RefCell<Option<CompletionItem>>> = Rc::new(RefCell::new(None));
        // Escape means "not for this word". Holds the buffer offset of the `#`
        // it applied to, so suggestions stay away until the cursor leaves that
        // one — every shell autosuggestion behaves this way, and popping back up
        // on the next keystroke made Escape feel broken.
        let completion_suppressed_at: Rc<Cell<i32>> = Rc::new(Cell::new(-1));
        // The citation/CV ghost shares the same label — only one suggestion can
        // be under the cursor at a time — but keeps its own slot so Tab knows
        // which kind of completion it is taking.
        let ghost_bib_entry: Rc<RefCell<Option<PopupEntry>>> = Rc::new(RefCell::new(None));

        let ghost_ac = ghost_label.clone();
        let ghost_bib_ac = ghost_bib_entry.clone();
        let ghost_item_ac = ghost_item.clone();
        let hint_ac = self.lsp_status_label.clone();
        let view_ac = view.clone();
        let popup_ac = bib_popup.clone();
        let mark_ac = ac_mark.clone();
        let completing_ac = completing.clone();
        let bib_active_ac = bib_active_for_open.clone();
        let cv_mode_ac = self.cv_mode.clone();
        buffer.connect_changed(move |buf| {
            if *completing_ac.borrow() {
                return;
            }
            let cursor_pos = buf.cursor_position();
            let cursor_iter = buf.iter_at_offset(cursor_pos);
            let mut temp = cursor_iter;
            let mut found_trigger = false;
            let mut trigger_char = '@';
            let mut at_iter = cursor_iter;
            loop {
                if !temp.backward_char() {
                    break;
                }
                let ch = temp.char();
                if ch == '@' || (ch == '!' && cv_mode_ac.get()) {
                    found_trigger = true;
                    trigger_char = ch;
                    at_iter = temp;
                    break;
                }
                if !(ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ':') {
                    break;
                }
            }
            if !found_trigger {
                *bib_active_ac.borrow_mut() = false;
                clear_citation_ghost(&ghost_ac, &ghost_bib_ac, &hint_ac);
                dismiss_popup(buf, &popup_ac, &mark_ac);
                return;
            }
            let prev_is_word = {
                let mut prev = at_iter;
                if prev.backward_char() {
                    let ch = prev.char();
                    ch.is_alphanumeric() || ch == '_'
                } else {
                    false
                }
            };
            if prev_is_word {
                *bib_active_ac.borrow_mut() = false;
                clear_citation_ghost(&ghost_ac, &ghost_bib_ac, &hint_ac);
                dismiss_popup(buf, &popup_ac, &mark_ac);
                return;
            }
            let query = buf.text(&at_iter, &cursor_iter, false);
            let query = query.trim_start_matches(trigger_char);
            {
                let mut mark_ref = mark_ac.borrow_mut();
                match mark_ref.as_ref() {
                    Some(m) => buf.move_mark(m, &at_iter),
                    None => *mark_ref = Some(buf.create_mark(None::<&str>, &at_iter, true)),
                }
            }
            // Position popup below cursor when in upper half of view,
            // above cursor when in lower half — so it never lands on the cursor line.
            let loc = view_ac.iter_location(&cursor_iter);
            let (wx, wy_bottom) = view_ac.buffer_to_window_coords(
                TextWindowType::Widget,
                loc.x(),
                loc.y() + loc.height(),
            );
            let (_, wy_top) =
                view_ac.buffer_to_window_coords(TextWindowType::Widget, loc.x(), loc.y());
            let view_h = view_ac.allocated_height();
            // above=true: popup uses PositionType::Top, its bottom lands at wy_top (cursor top)
            // above=false: popup uses PositionType::Bottom, its top lands at wy_bottom (cursor bottom)
            let above = wy_bottom > view_h / 2;
            let wy = if above { wy_top } else { wy_bottom };
            let source = if trigger_char == '!' {
                PopupSource::Cv
            } else {
                PopupSource::Bib
            };

            // Same rules as `#`: inline suggestion first, list once the query is
            // worth listing. A bare `@` used to drop the whole bibliography over
            // the text.
            // `@fig-1` or `@intro` naming a label this document defines is a
            // cross-reference, not a citation: stay quiet rather than offer
            // bibliography entries that happen to match.
            let is_local_label = source == PopupSource::Bib && !query.is_empty() && {
                let (s, e) = buf.bounds();
                crate::citation_keys::labels_in(buf.text(&s, &e, false).as_str()).contains(query)
            };
            let matches = if is_local_label {
                Vec::new()
            } else {
                popup_ac.matches_for(query, source)
            };
            let ghost_entry = if is_local_label {
                None
            } else {
                popup_ac.ghost_entry(query, source)
            };
            let list_open = query.chars().count() >= MIN_POPUP_PREFIX && !matches.is_empty();
            if list_open {
                popup_ac.show_filtered(query, wx, wy, above, source);
            } else {
                popup_ac.hide();
            }
            *ghost_item_ac.borrow_mut() = None;
            set_citation_ghost(
                &view_ac,
                &ghost_ac,
                &ghost_bib_ac,
                &hint_ac,
                buf,
                ghost_entry.clone(),
                query,
            );
            set_citation_hint(
                &hint_ac,
                ghost_entry.as_ref().or_else(|| matches.first()),
                ghost_entry.is_some(),
                list_open,
            );
            *bib_active_ac.borrow_mut() = popup_ac.is_visible();
        });

        CitationAutocomplete {
            bib_popup,
            ac_mark,
            completing,
            ghost_label,
            ghost_item,
            completion_suppressed_at,
            ghost_bib_entry,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn wire_lsp_autocomplete(
        &self,
        view: &View,
        buffer: &Buffer,
        path: &Path,
        ghost_label: &Label,
        ghost_item: &Rc<RefCell<Option<CompletionItem>>>,
        completion_suppressed_at: &Rc<Cell<i32>>,
        ghost_bib_entry: &Rc<RefCell<Option<PopupEntry>>>,
    ) -> LspAutocomplete {
        let lsp_popup = LspPopup::new(view);
        let lsp_mark: Rc<RefCell<Option<gtk4::TextMark>>> = Rc::new(RefCell::new(None));
        let lsp_completing: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let lsp_comp_gen: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));

        let update_ghost = {
            let view = view.clone();
            let ghost = ghost_label.clone();
            let slot = ghost_item.clone();
            let hint = self.lsp_status_label.clone();
            move |buf: &Buffer, item: Option<CompletionItem>, prefix: &str| {
                set_ghost(&view, &ghost, &slot, &hint, buf, item, prefix);
            }
        };
        let hide_ghost = {
            let ghost = ghost_label.clone();
            let slot = ghost_item.clone();
            let hint = self.lsp_status_label.clone();
            move || clear_ghost(&ghost, &slot, &hint)
        };

        // Arrowing through the list re-describes the highlighted entry in the
        // status bar, the way VS Code's details panel tracks its selection —
        // except this one costs no screen space over the document.
        {
            let hint_sel = self.lsp_status_label.clone();
            let ghost_sel = ghost_label.clone();
            let buf_sel = buffer.clone();
            let lsp_ready_sel = self.lsp_ready.clone();
            lsp_popup.set_on_selection_changed(move |item| {
                let prefix = lsp_hash_prefix(&buf_sel);
                set_completion_hint(
                    &hint_sel,
                    item.as_ref(),
                    &prefix,
                    ghost_sel.is_visible(),
                    true,
                    lsp_ready_sel.get(),
                );
            });
        }

        // Remember what was chosen for the prefix that was typed, so the next
        // time it's typed the ghost offers the same thing first.
        let remember_pick = {
            let picks = self.completion_picks.clone();
            let root = self.project_root.clone();
            move |prefix: &str, label: &str| {
                if prefix.is_empty() {
                    return;
                }
                let changed = picks
                    .borrow()
                    .get(prefix)
                    .map(|existing| existing != label)
                    .unwrap_or(true);
                if !changed {
                    return;
                }
                picks
                    .borrow_mut()
                    .insert(prefix.to_string(), label.to_string());
                let Some(root_dir) = root.borrow().clone() else {
                    return;
                };
                let mut pcfg = crate::config::ProjectConfig::load(&root_dir).unwrap_or_default();
                pcfg.completion_picks = picks.borrow().clone();
                let _ = pcfg.save(&root_dir);
            }
        };

        // LSP on_complete: replace #prefix with the chosen insertion text
        {
            let buf2 = buffer.clone();
            let view2 = view.clone();
            let mark2 = lsp_mark.clone();
            let comp2 = lsp_completing.clone();
            let popup2 = lsp_popup.clone();
            let ghost2 = ghost_label.clone();
            let ghost_item2 = ghost_item.clone();
            let hint2 = self.lsp_status_label.clone();
            let remember2 = remember_pick.clone();
            lsp_popup.set_on_complete(move |item| {
                remember2(&lsp_hash_prefix(&buf2), &item.label);
                clear_ghost(&ghost2, &ghost_item2, &hint2);
                *comp2.borrow_mut() = true;
                let mark_opt = mark2.borrow().clone();
                if let Some(ref m) = mark_opt {
                    let mut start = buf2.iter_at_mark(m); // position of '#'
                    let mut end = buf2.iter_at_offset(buf2.cursor_position());
                    let insert_text = item.insert_text.as_deref().unwrap_or(&item.label);
                    let insert_text = strip_snippets(insert_text);
                    let final_text = if insert_text.starts_with('#') {
                        insert_text
                    } else {
                        format!("#{insert_text}")
                    };
                    buf2.begin_user_action();
                    buf2.delete(&mut start, &mut end);
                    buf2.insert_at_cursor(&final_text);
                    buf2.end_user_action();
                    buf2.delete_mark(m);
                }
                *mark2.borrow_mut() = None;
                popup2.hide();
                view2.grab_focus();
                *comp2.borrow_mut() = false;
            });
        }

        // Detect #word context and fire on_completion_needed
        {
            let lsp_mark3 = lsp_mark.clone();
            let lsp_popup3 = lsp_popup.clone();
            let lsp_completing3 = lsp_completing.clone();
            let lsp_gen3 = lsp_comp_gen.clone();
            let on_comp_cb = self.on_completion_needed.clone();
            let path_for_lsp = path.to_path_buf();
            let view_lsp = view.clone();
            let cv_mode_for_lsp = self.cv_mode.clone();
            let update_ghost_lsp = update_ghost.clone();
            let hide_ghost_lsp = hide_ghost.clone();
            let hint_lbl_lsp = self.lsp_status_label.clone();
            let lsp_ready_lsp = self.lsp_ready.clone();
            let picks_lsp = self.completion_picks.clone();
            let suppressed_at = completion_suppressed_at.clone();
            let ghost_bib_lsp = ghost_bib_entry.clone();
            buffer.connect_changed(move |buf| {
                if *lsp_completing3.borrow() {
                    return;
                }
                let cursor_pos = buf.cursor_position();
                let cursor_iter = buf.iter_at_offset(cursor_pos);
                let mut temp = cursor_iter;
                let mut found_hash = false;
                let mut hash_iter = cursor_iter;

                loop {
                    if !temp.backward_char() {
                        break;
                    }
                    let ch = temp.char();
                    if ch == '#' {
                        found_hash = true;
                        hash_iter = temp;
                        break;
                    }
                    if !(ch.is_alphanumeric() || ch == '_' || ch == '-') {
                        break;
                    }
                }

                if found_hash {
                    // Escape suppresses suggestions for *this* `#` only; typing
                    // on past it, or starting another one, brings them back.
                    if suppressed_at.get() == hash_iter.offset() {
                        lsp_popup3.hide();
                        hide_ghost_lsp();
                        return;
                    }
                    suppressed_at.set(-1);

                    // Track the '#' position
                    {
                        let mut mark_ref = lsp_mark3.borrow_mut();
                        match mark_ref.as_ref() {
                            Some(m) => buf.move_mark(m, &hash_iter),
                            None => {
                                *mark_ref = Some(buf.create_mark(None::<&str>, &hash_iter, true))
                            }
                        }
                    }

                    // Load the built-in snippets without waiting for the LSP, but
                    // don't put a list on screen for a bare `#` — at one typed
                    // character everything still matches, so the list is noise on
                    // top of the text. The ghost suggestion carries that stage;
                    // the list joins in once the prefix narrows things down.
                    let prefix = lsp_hash_prefix(buf);
                    let loc = view_lsp.iter_location(&cursor_iter);
                    let (wx, wy_bottom) = view_lsp.buffer_to_window_coords(
                        TextWindowType::Widget,
                        loc.x(),
                        loc.y() + loc.height(),
                    );
                    let (_, wy_top) =
                        view_lsp.buffer_to_window_coords(TextWindowType::Widget, loc.x(), loc.y());
                    let view_h = view_lsp.allocated_height();
                    let above = wy_bottom > view_h / 2;
                    let wy = if above { wy_top } else { wy_bottom };
                    let snippets = snippet_items(cv_mode_for_lsp.get());

                    // Names already written in this document rank above ones
                    // that aren't, and a name previously chosen for this exact
                    // prefix outranks everything.
                    lsp_popup3.set_local_names(names_used_in(buf));
                    lsp_popup3.set_preferred_name(picks_lsp.borrow().get(&prefix).cloned());

                    if lsp_popup3.is_visible() {
                        lsp_popup3.apply_filter(&prefix);
                    } else {
                        lsp_popup3.load_items(snippets);
                        lsp_popup3.apply_filter(&prefix);
                    }

                    let matches = lsp_popup3.match_count(&prefix);
                    let list_open = prefix.chars().count() >= MIN_POPUP_PREFIX && matches > 0;
                    if list_open {
                        lsp_popup3.show_at(wx, wy, above);
                    } else {
                        lsp_popup3.hide();
                    }
                    let ghosted = lsp_popup3.best_match(&prefix);
                    *ghost_bib_lsp.borrow_mut() = None;
                    update_ghost_lsp(buf, ghosted.clone(), &prefix);
                    set_completion_hint(
                        &hint_lbl_lsp,
                        lsp_popup3.describable_match(&prefix).as_ref(),
                        &prefix,
                        ghosted.is_some(),
                        list_open,
                        lsp_ready_lsp.get(),
                    );

                    let line = cursor_iter.line() as u32 + 1;
                    // LSP positions are UTF-16 code units by default (we don't
                    // advertise a different `general.positionEncodings`), but
                    // `line_offset()` counts Unicode codepoints — the two only
                    // agree for text entirely within the Basic Multilingual
                    // Plane. Count UTF-16 units up to the cursor instead, so
                    // completions stay aligned on lines with e.g. emoji before
                    // the cursor.
                    let mut line_start = cursor_iter;
                    line_start.set_line_offset(0);
                    let text_before_cursor = buf.text(&line_start, &cursor_iter, false);
                    let col = text_before_cursor.encode_utf16().count() as u32 + 1;

                    *lsp_gen3.borrow_mut() += 1;
                    let my_gen = *lsp_gen3.borrow();
                    let gen4 = lsp_gen3.clone();
                    let ocb = on_comp_cb.clone();
                    let p = path_for_lsp.clone();

                    glib::timeout_add_local(Duration::from_millis(150), move || {
                        if *gen4.borrow() == my_gen {
                            if let Some(f) = ocb.borrow().as_ref() {
                                f(p.clone(), line, col);
                            }
                        }
                        glib::ControlFlow::Break
                    });
                } else {
                    // No longer in # context — clear mark and hide popup
                    if let Some(m) = lsp_mark3.borrow_mut().take() {
                        buf.delete_mark(&m);
                    }
                    lsp_popup3.hide();
                    hide_ghost_lsp();
                    hint_lbl_lsp.set_text("");
                }
            });
        }

        LspAutocomplete {
            lsp_popup,
            lsp_mark,
            lsp_completing,
        }
    }
}

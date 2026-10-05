//! ☰ → **Recover…** — one place to get something back.
//!
//! The window opens on a plain question ("What do you want back?") with a few
//! lines saying how safe the work is, and leads to the right place:
//! an earlier version of the open document (with search across them), a saved
//! copy made when files were replaced with GitHub's, the Trash, or the latest
//! writing from another computer. Bringing something back never overwrites: it
//! makes a new file beside the original, and replacing the text in the editor is
//! a second, confirmed choice that keeps the current text first.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Image, Label, ListBox, ListBoxRow, Orientation, Paned,
    ScrolledWindow, SearchEntry, SelectionMode, TextTag, TextView, WrapMode,
};
use libadwaita as adw;

use crate::recover::{self, Version};

/// What the window can ask the rest of the app to do.
pub struct RecoverActions {
    pub open_trash: Rc<dyn Fn()>,
    pub get_latest: Rc<dyn Fn()>,
    pub open_file: Rc<dyn Fn(PathBuf)>,
    /// Replaces the text of the open document, as one undoable edit.
    pub replace_active_text: Rc<dyn Fn(String)>,
}

/// What is open when the window is asked for.
pub struct RecoverInput {
    pub project_root: PathBuf,
    /// The open document and its text as it is now.
    pub document: Option<(PathBuf, String)>,
}

fn nav_row(title: &str, subtitle: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .activatable(true)
        .build();
    row.add_suffix(&Image::from_icon_name("go-next-symbolic"));
    row
}

fn page(title: &str, child: &impl IsA<gtk4::Widget>) -> adw::NavigationPage {
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(child));
    adw::NavigationPage::new(&toolbar, title)
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn show(parent: &impl IsA<gtk4::Window>, input: RecoverInput, actions: RecoverActions) {
    let window = adw::Window::builder()
        .title("Recover")
        .transient_for(parent)
        .modal(true)
        .default_width(880)
        .default_height(640)
        .build();
    let toasts = adw::ToastOverlay::new();
    let nav = adw::NavigationView::new();
    toasts.set_child(Some(&nav));
    window.set_content(Some(&toasts));

    let input = Rc::new(input);
    let actions = Rc::new(actions);
    nav.push(&front_page(&window, &nav, &toasts, &input, &actions));
    window.present();
}

fn front_page(
    window: &adw::Window,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    input: &Rc<RecoverInput>,
    actions: &Rc<RecoverActions>,
) -> adw::NavigationPage {
    let prefs = adw::PreferencesPage::new();

    // How safe is the work?
    let safe = adw::PreferencesGroup::new();
    safe.set_title("Is my work safe?");
    let status = recover::status(&input.project_root);
    for line in recover::status_lines(&status, chrono::Local::now()) {
        let row = adw::ActionRow::builder().title(&line).build();
        row.set_title_lines(0);
        safe.add(&row);
    }
    prefs.add(&safe);

    // What do you want back?
    let group = adw::PreferencesGroup::new();
    group.set_title("What do you want back?");

    let doc_name = input
        .document
        .as_ref()
        .map(|(p, _)| file_name(p))
        .unwrap_or_default();
    let versions_row = nav_row(
        "An earlier version of this document",
        if input.document.is_some() {
            &doc_name
        } else {
            "Open a document first"
        },
    );
    versions_row.set_sensitive(input.document.is_some());
    group.add(&versions_row);

    let trash_row = nav_row(
        "A document I deleted",
        "Opens the Trash in the Library, where deleted documents wait",
    );
    group.add(&trash_row);

    let copies = recover::saved_copies(&input.project_root);
    let copies_row = nav_row(
        "Something that disappeared when I replaced my files with GitHub's",
        &match copies.len() {
            0 => "Nothing yet — Zerkalo keeps a saved copy whenever you do that".to_string(),
            1 => "1 saved copy".to_string(),
            n => format!("{n} saved copies"),
        },
    );
    copies_row.set_sensitive(!copies.is_empty());
    group.add(&copies_row);

    let latest_row = nav_row(
        "My latest writing from my other computer",
        "Brings everything backed up online down to this computer",
    );
    group.add(&latest_row);
    prefs.add(&group);

    {
        let (nav, toasts, input, actions) =
            (nav.clone(), toasts.clone(), input.clone(), actions.clone());
        versions_row.connect_activated(move |_| {
            nav.push(&versions_page(&nav, &toasts, &input, &actions));
        });
    }
    {
        let (w, actions) = (window.clone(), actions.clone());
        trash_row.connect_activated(move |_| {
            w.close();
            (actions.open_trash)();
        });
    }
    {
        let (nav, toasts, input, actions) =
            (nav.clone(), toasts.clone(), input.clone(), actions.clone());
        let copies = Rc::new(copies);
        copies_row.connect_activated(move |_| {
            nav.push(&copies_page(&toasts, &input, &actions, &copies));
        });
    }
    {
        let (w, actions) = (window.clone(), actions.clone());
        latest_row.connect_activated(move |_| {
            w.close();
            (actions.get_latest)();
        });
    }

    page("Recover", &prefs)
}

// ── An earlier version of the open document ──────────────────────────────────

fn versions_page(
    _nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    input: &Rc<RecoverInput>,
    actions: &Rc<RecoverActions>,
) -> adw::NavigationPage {
    let (file, current) = input.document.clone().unwrap_or_default();
    let versions: Rc<Vec<Version>> = Rc::new(recover::versions(&input.project_root, &file));
    let texts: Rc<RefCell<Vec<Option<String>>>> = Rc::new(RefCell::new(vec![None; versions.len()]));
    let loaded = Rc::new(Cell::new(false));
    let visible: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(vec![true; versions.len()]));
    let selected: Rc<Cell<Option<usize>>> = Rc::new(Cell::new(None));

    let outer = GtkBox::new(Orientation::Vertical, 0);

    if versions.is_empty() {
        let status = adw::StatusPage::builder()
            .icon_name("document-open-recent-symbolic")
            .title("No earlier versions yet")
            .description(
                "Zerkalo keeps a version each time you press Ctrl+S, and the sync button \
                 keeps more. Earlier versions of this document will appear here.",
            )
            .vexpand(true)
            .build();
        outer.append(&status);
        return page("Earlier versions", &outer);
    }

    let search = SearchEntry::new();
    search.set_placeholder_text(Some("Find a word or phrase in earlier versions…"));
    search.set_margin_start(12);
    search.set_margin_end(12);
    search.set_margin_top(8);
    search.set_margin_bottom(4);
    outer.append(&search);
    let search_note = Label::new(None);
    search_note.add_css_class("caption");
    search_note.add_css_class("dim-label");
    search_note.set_xalign(0.0);
    search_note.set_margin_start(14);
    search_note.set_margin_bottom(6);
    outer.append(&search_note);

    let split = Paned::new(Orientation::Horizontal);
    split.set_vexpand(true);
    split.set_position(320);
    split.set_shrink_start_child(false);
    split.set_shrink_end_child(false);

    // Left: the timeline.
    let list = ListBox::new();
    list.set_selection_mode(SelectionMode::Single);
    list.add_css_class("navigation-sidebar");
    let now = chrono::Local::now();
    for v in versions.iter() {
        let row = ListBoxRow::new();
        let b = GtkBox::new(Orientation::Vertical, 2);
        b.set_margin_top(6);
        b.set_margin_bottom(6);
        b.set_margin_start(10);
        b.set_margin_end(10);
        let when = Label::new(Some(&recover::friendly_when(v.when, now)));
        when.set_xalign(0.0);
        when.add_css_class("heading");
        let kind = Label::new(Some(v.kind.label()));
        kind.set_xalign(0.0);
        kind.add_css_class("caption");
        kind.add_css_class("dim-label");
        b.append(&when);
        b.append(&kind);
        row.set_child(Some(&b));
        list.append(&row);
    }
    {
        let visible = visible.clone();
        list.set_filter_func(move |row| {
            visible
                .borrow()
                .get(row.index() as usize)
                .copied()
                .unwrap_or(true)
        });
    }
    let list_scroll = ScrolledWindow::new();
    list_scroll.set_child(Some(&list));
    list_scroll.set_min_content_width(260);
    split.set_start_child(Some(&list_scroll));

    // Right: what you'd get.
    let right = GtkBox::new(Orientation::Vertical, 6);
    right.set_margin_start(12);
    right.set_margin_end(12);
    right.set_margin_top(4);
    right.set_margin_bottom(12);
    let info = Label::new(Some("Pick a version on the left to see what it holds."));
    info.set_xalign(0.0);
    info.set_wrap(true);
    right.append(&info);
    let legend = Label::new(Some(
        "Green lines would come back. Red lines are in your document now but not in that version.",
    ));
    legend.add_css_class("caption");
    legend.add_css_class("dim-label");
    legend.set_xalign(0.0);
    legend.set_wrap(true);
    right.append(&legend);

    let buf = gtk4::TextBuffer::new(None);
    let colors = crate::ui::theme::diff_colors();
    let tag_removed = TextTag::new(Some("removed"));
    tag_removed.set_property("background", colors.removed_bg);
    tag_removed.set_property("foreground", colors.removed_fg);
    tag_removed.set_property("strikethrough", true);
    let tag_added = TextTag::new(Some("added"));
    tag_added.set_property("background", colors.added_bg);
    tag_added.set_property("foreground", colors.added_fg);
    tag_added.set_property("underline", gtk4::pango::Underline::Single);
    buf.tag_table().add(&tag_removed);
    buf.tag_table().add(&tag_added);
    let view = TextView::with_buffer(&buf);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.set_wrap_mode(WrapMode::WordChar);
    view.set_left_margin(8);
    view.set_right_margin(8);
    let view_scroll = ScrolledWindow::new();
    view_scroll.set_child(Some(&view));
    view_scroll.set_vexpand(true);
    right.append(&view_scroll);

    let buttons = GtkBox::new(Orientation::Horizontal, 8);
    buttons.set_halign(Align::End);
    let replace_btn = Button::with_label("Replace what's here…");
    replace_btn.set_tooltip_text(Some(
        "Put this version's text in the editor in place of the current text. The current text is kept first.",
    ));
    let copy_btn = Button::with_label("Bring back as a copy");
    copy_btn.add_css_class("suggested-action");
    copy_btn.set_tooltip_text(Some(
        "Make a new document beside this one holding this version. Nothing here changes.",
    ));
    replace_btn.set_sensitive(false);
    copy_btn.set_sensitive(false);
    buttons.append(&replace_btn);
    buttons.append(&copy_btn);
    right.append(&buttons);
    split.set_end_child(Some(&right));
    outer.append(&split);

    // Selecting a version shows what it would bring back and take out.
    {
        let (versions, texts, selected, file, current) = (
            versions.clone(),
            texts.clone(),
            selected.clone(),
            file.clone(),
            current.clone(),
        );
        let (buf, info, copy_btn, replace_btn) = (
            buf.clone(),
            info.clone(),
            copy_btn.clone(),
            replace_btn.clone(),
        );
        list.connect_row_selected(move |_, row| {
            let Some(row) = row else { return };
            let idx = row.index() as usize;
            let Some(v) = versions.get(idx) else { return };
            let text = {
                let mut t = texts.borrow_mut();
                if t[idx].is_none() {
                    t[idx] = recover::text_of(v, &file);
                }
                t[idx].clone()
            };
            let Some(text) = text else {
                info.set_text("This version couldn't be read.");
                copy_btn.set_sensitive(false);
                replace_btn.set_sensitive(false);
                return;
            };
            selected.set(Some(idx));
            // What the version holds that the document doesn't (comes back) is
            // "+", what the document has that the version doesn't (goes) is "-".
            let diff = crate::ui::diff_render::simple_diff(&current, &text);
            crate::ui::diff_render::render_clean_diff(&buf, &diff);
            info.set_text(&format!(
                "{} {}",
                recover::describe_changes(&text, &current),
                v.kind.label()
            ));
            copy_btn.set_sensitive(true);
            replace_btn.set_sensitive(true);
        });
    }

    // Looking through every version for a phrase: read them in the background.
    {
        let (versions, texts, loaded, file) = (
            versions.clone(),
            texts.clone(),
            loaded.clone(),
            file.clone(),
        );
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<Option<String>>>(1);
        let vs: Vec<Version> = versions.iter().cloned().collect();
        std::thread::spawn(move || {
            let all: Vec<Option<String>> = vs.iter().map(|v| recover::text_of(v, &file)).collect();
            let _ = tx.send(all);
        });
        let search_c = search.clone();
        let rx = Rc::new(rx);
        glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            match rx.try_recv() {
                Ok(all) => {
                    *texts.borrow_mut() = all;
                    loaded.set(true);
                    // Re-run any search typed while reading.
                    if !search_c.text().is_empty() {
                        search_c.emit_by_name::<()>("search-changed", &[]);
                    }
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    }
    {
        let (texts, visible, loaded, list, search_note) = (
            texts.clone(),
            visible.clone(),
            loaded.clone(),
            list.clone(),
            search_note.clone(),
        );
        let total = versions.len();
        search.connect_search_changed(move |s| {
            let q = s.text().to_string();
            if q.trim().is_empty() {
                visible.borrow_mut().iter_mut().for_each(|v| *v = true);
                search_note.set_text("");
            } else if !loaded.get() {
                search_note.set_text("Still reading through the earlier versions…");
            } else {
                let t = texts.borrow();
                let mut hits = 0;
                let mut vis = visible.borrow_mut();
                for (i, v) in vis.iter_mut().enumerate() {
                    *v = t[i]
                        .as_deref()
                        .is_some_and(|x| recover::text_matches(x, &q));
                    if *v {
                        hits += 1;
                    }
                }
                search_note.set_text(&match hits {
                    0 => "No earlier version has those words.".to_string(),
                    n => format!("{n} of {total} versions have those words — newest first."),
                });
            }
            list.invalidate_filter();
        });
    }

    // Bring it back as a copy: the safe default.
    {
        let (versions, texts, selected, file, actions, toasts) = (
            versions.clone(),
            texts.clone(),
            selected.clone(),
            file.clone(),
            actions.clone(),
            toasts.clone(),
        );
        copy_btn.connect_clicked(move |_| {
            let Some(idx) = selected.get() else { return };
            let (Some(v), Some(text)) = (versions.get(idx), texts.borrow()[idx].clone()) else {
                return;
            };
            match recover::bring_back_as_copy(&file, v.when, &text) {
                Ok(path) => {
                    toasts.add_toast(adw::Toast::new(&format!(
                        "Made “{}” beside this document and opened it",
                        file_name(&path)
                    )));
                    (actions.open_file)(path);
                }
                Err(e) => {
                    toasts.add_toast(adw::Toast::new(&format!("Couldn't make the copy: {e}")))
                }
            }
        });
    }

    // Replace what's here: confirmed, current text kept first, undoable.
    {
        let (texts, selected, file, current, actions, toasts, root) = (
            texts.clone(),
            selected.clone(),
            file.clone(),
            current.clone(),
            actions.clone(),
            toasts.clone(),
            input.project_root.clone(),
        );
        let replace_btn_c = replace_btn.clone();
        replace_btn.connect_clicked(move |btn| {
            let Some(idx) = selected.get() else { return };
            let Some(text) = texts.borrow()[idx].clone() else {
                return;
            };
            let parent = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
            let dlg = adw::MessageDialog::new(
                parent.as_ref(),
                Some("Replace what's here with this version?"),
                Some(
                    "Your document as it is now is kept as a saved version first, so you can \
                     get it back — and Ctrl+Z undoes this as well.",
                ),
            );
            dlg.add_response("cancel", "Cancel");
            dlg.add_response("replace", "Replace");
            dlg.set_response_appearance("replace", adw::ResponseAppearance::Destructive);
            dlg.set_default_response(Some("cancel"));
            dlg.set_close_response("cancel");
            let (file, current, actions, toasts, root, btn) = (
                file.clone(),
                current.clone(),
                actions.clone(),
                toasts.clone(),
                root.clone(),
                replace_btn_c.clone(),
            );
            dlg.connect_response(None, move |dlg, response| {
                dlg.close();
                if response != "replace" {
                    return;
                }
                crate::ui::snapshot_dialog::save_snapshot(&root, &file, &current);
                (actions.replace_active_text)(text.clone());
                toasts.add_toast(adw::Toast::new(
                    "Replaced. Your previous text is kept in Saved Versions, and Ctrl+Z undoes it.",
                ));
                btn.set_sensitive(false);
            });
            dlg.present();
        });
    }

    page("Earlier versions", &outer)
}

// ── Saved copies ─────────────────────────────────────────────────────────────

fn copies_page(
    toasts: &adw::ToastOverlay,
    input: &Rc<RecoverInput>,
    actions: &Rc<RecoverActions>,
    copies: &Rc<Vec<recover::SavedCopy>>,
) -> adw::NavigationPage {
    let prefs = adw::PreferencesPage::new();
    let intro = adw::PreferencesGroup::new();
    intro.set_description(Some(
        "When you replace this computer's files with GitHub's, Zerkalo first keeps everything that \
         was only here. Bring any file back as a new document beside where it was — nothing is overwritten.",
    ));
    prefs.add(&intro);
    let now = chrono::Local::now();
    for copy in copies.iter() {
        let g = adw::PreferencesGroup::new();
        g.set_title(&format!("Kept {}", recover::friendly_when(copy.when, now)));
        if copy.files.is_empty() {
            g.set_description(Some(
                "Nothing in this saved copy differs from your files now.",
            ));
        }
        for rel in &copy.files {
            let row = adw::ActionRow::builder().title(rel).build();
            let btn = Button::with_label("Bring back as a copy");
            btn.set_valign(Align::Center);
            let (root, branch, rel, actions, toasts) = (
                input.project_root.clone(),
                copy.branch.clone(),
                rel.clone(),
                actions.clone(),
                toasts.clone(),
            );
            btn.connect_clicked(move |b| {
                match recover::bring_back_from_saved_copy(&root, &branch, &rel) {
                    Ok(path) => {
                        toasts.add_toast(adw::Toast::new(&format!(
                            "Made “{}” and opened it",
                            file_name(&path)
                        )));
                        b.set_sensitive(false);
                        (actions.open_file)(path);
                    }
                    Err(e) => toasts.add_toast(adw::Toast::new(&e)),
                }
            });
            row.add_suffix(&btn);
            g.add(&row);
        }
        prefs.add(&g);
    }
    page("Saved copies", &prefs)
}

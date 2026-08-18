//! Sidebar panel for the comment threads in `crate::comments`. Owns its
//! `CommentThread` and persistence; the caller (`app_window`) is
//! responsible for tracking the editor's cursor position and active
//! document, since the panel has no reason to know about `EditorPane`
//! directly — matches the narrow-callback idiom every other panel in this
//! codebase already uses (`outline_panel`, `dep_graph`, etc.).
//!
//! v1 scope: comments only, no inline gutter markers in the editor itself
//! — `editor_pane.rs` is this codebase's largest and most fragility-prone
//! file (see `REFACTOR-PLAN.md`'s own caution about it), and the sidebar
//! list already covers the "advisor reviews a draft" workflow this exists
//! for. Gutter markers are a natural follow-up once this is in real use,
//! not a first-version requirement.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Label, ListBox, ListBoxRow, Orientation, Popover, Revealer,
    RevealerTransitionType, ScrolledWindow, SelectionMode, Separator, TextView, WrapMode,
};

use crate::comments::{suggestion_removes_text, CommentThread, SuggestionKind, SuggestionStatus};

type JumpCb = Rc<RefCell<Option<Box<dyn Fn(u32)>>>>;
type RequestAnchorCb = Rc<RefCell<Option<Box<dyn Fn() -> (u32, String)>>>>;
/// `(anchor_line, suggestion text)` — fired after an accept/reject decision
/// that requires removing text from the live document. Only called when
/// `suggestion_removes_text` says so; keeping-as-is needs no document edit.
type ApplySuggestionCb = Rc<RefCell<Option<Box<dyn Fn(u32, String)>>>>;

#[derive(Clone)]
pub struct CommentsPanel {
    widget: GtkBox,
    list_box: ListBox,
    count_label: Label,
    add_btn: Button,
    collapse_btn: Button,
    revealer: Revealer,
    current_path: Rc<RefCell<Option<PathBuf>>>,
    thread: Rc<RefCell<CommentThread>>,
    on_jump: JumpCb,
    /// Fired when the user confirms the "Add Comment" popover, to ask
    /// `app_window` (the only thing that tracks `EditorPane`'s cursor) for
    /// the current `(line, that line's text)` to anchor the new comment to.
    on_request_anchor: RequestAnchorCb,
    on_apply_suggestion: ApplySuggestionCb,
    on_collapse_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
}

impl CommentsPanel {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let header = GtkBox::new(Orientation::Horizontal, 6);
        header.set_margin_start(10);
        header.set_margin_end(10);
        header.set_margin_top(6);
        header.set_margin_bottom(6);
        let title = Label::new(Some("Comments"));
        title.set_xalign(0.0);
        title.add_css_class("heading");
        header.append(&title);
        let count_label = Label::new(None);
        count_label.add_css_class("caption");
        count_label.add_css_class("dim-label");
        header.append(&count_label);
        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        header.append(&spacer);
        let add_btn = Button::from_icon_name("list-add-symbolic");
        add_btn.add_css_class("flat");
        add_btn.set_tooltip_text(Some("Add a comment at the cursor's current line"));
        add_btn.update_property(&[gtk4::accessible::Property::Label(
            "Add a comment at the cursor's current line",
        )]);
        header.append(&add_btn);

        let collapse_btn = Button::from_icon_name("pan-down-symbolic");
        collapse_btn.add_css_class("flat");
        collapse_btn.set_tooltip_text(Some("Hide Comments"));
        collapse_btn.update_property(&[gtk4::accessible::Property::Label("Hide Comments")]);
        header.append(&collapse_btn);

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&header);
        widget.append(&Separator::new(Orientation::Horizontal));

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
        list_box.add_css_class("navigation-sidebar");
        scroll.set_child(Some(&list_box));

        let revealer = Revealer::new();
        revealer.set_transition_type(RevealerTransitionType::SlideDown);
        revealer.set_reveal_child(true);
        revealer.set_vexpand(true);
        revealer.set_child(Some(&scroll));
        widget.append(&revealer);

        let on_collapse_toggle: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> =
            Rc::new(RefCell::new(None));
        {
            let revealer_c = revealer.clone();
            let collapse_btn_c = collapse_btn.clone();
            let on_collapse_toggle_c = on_collapse_toggle.clone();
            collapse_btn.connect_clicked(move |_| {
                let now_collapsed = revealer_c.reveals_child();
                revealer_c.set_reveal_child(!now_collapsed);
                collapse_btn_c.set_icon_name(if now_collapsed {
                    "pan-end-symbolic"
                } else {
                    "pan-down-symbolic"
                });
                collapse_btn_c.set_tooltip_text(Some(if now_collapsed {
                    "Show Comments"
                } else {
                    "Hide Comments"
                }));
                if let Some(f) = on_collapse_toggle_c.borrow().as_ref() {
                    f(now_collapsed);
                }
            });
        }

        let panel = Self {
            widget,
            list_box,
            count_label,
            add_btn: add_btn.clone(),
            collapse_btn,
            revealer,
            current_path: Rc::new(RefCell::new(None)),
            thread: Rc::new(RefCell::new(CommentThread::default())),
            on_jump: Rc::new(RefCell::new(None)),
            on_request_anchor: Rc::new(RefCell::new(None)),
            on_apply_suggestion: Rc::new(RefCell::new(None)),
            on_collapse_toggle,
        };

        {
            let p = panel.clone();
            add_btn.connect_clicked(move |btn| {
                if p.current_path.borrow().is_none() {
                    return;
                }
                let p2 = p.clone();
                open_composer_popover(btn, "Add Comment", "Add", move |body| {
                    p2.add_comment_here(body);
                });
            });
        }

        panel.rebuild(&[]);
        panel
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    /// Restores a persisted collapsed/expanded state — called once at
    /// startup, before the user has clicked anything.
    pub fn set_collapsed(&self, collapsed: bool) {
        self.revealer.set_reveal_child(!collapsed);
        self.collapse_btn.set_icon_name(if collapsed {
            "pan-end-symbolic"
        } else {
            "pan-down-symbolic"
        });
        self.collapse_btn.set_tooltip_text(Some(if collapsed {
            "Show Comments"
        } else {
            "Hide Comments"
        }));
    }

    /// Fires with the new collapsed state whenever the user clicks the
    /// header's collapse toggle, so the caller can persist it.
    pub fn set_on_collapse_toggle(&self, f: impl Fn(bool) + 'static) {
        *self.on_collapse_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_jump(&self, f: impl Fn(u32) + 'static) {
        *self.on_jump.borrow_mut() = Some(Box::new(f));
    }

    /// Loads (or continues showing) `typ_path`'s comment thread, re-locating
    /// every anchor against `content`'s current lines and persisting any
    /// drift so it isn't silently recomputed on every keystroke. Called on
    /// tab switch and on the same debounced-edit cadence `outline_panel`
    /// already uses.
    pub fn update(&self, typ_path: &Path, content: &str) {
        let is_same_doc = self.current_path.borrow().as_deref() == Some(typ_path);
        let mut t = if is_same_doc {
            // Already loaded for this document — relocate the in-memory
            // thread rather than reloading from disk, so an unsaved
            // (not-yet-persisted-by-relocate) edit to the thread itself
            // isn't clobbered by a stale on-disk read mid-session. (Nothing
            // currently mutates the thread outside `add_comment_here`/
            // reply/resolve, which already save immediately, but this
            // keeps the invariant correct if that ever changes.)
            self.thread.borrow().clone()
        } else {
            CommentThread::load(typ_path)
        };
        let lost = t.relocate_all(content);
        t.save(typ_path);
        *self.current_path.borrow_mut() = Some(typ_path.to_path_buf());
        *self.thread.borrow_mut() = t;
        self.add_btn.set_sensitive(true);
        self.rebuild(&lost);
    }

    /// The panel doesn't know about `EditorPane`'s cursor directly (matching
    /// every other panel's narrow-callback idiom) — when the "Add Comment"
    /// popover confirms, this asks `app_window`, via `set_on_request_anchor`,
    /// for the current `(line, that line's text)` to anchor the comment to.
    pub fn set_on_request_anchor(&self, f: impl Fn() -> (u32, String) + 'static) {
        *self.on_request_anchor.borrow_mut() = Some(Box::new(f));
    }

    /// Fired when accepting/rejecting a suggestion means removing its text
    /// from the document (see `crate::comments::suggestion_removes_text`) —
    /// the panel doesn't touch `EditorPane` directly, matching every other
    /// callback here.
    pub fn set_on_apply_suggestion(&self, f: impl Fn(u32, String) + 'static) {
        *self.on_apply_suggestion.borrow_mut() = Some(Box::new(f));
    }

    fn add_comment_here(&self, body: String) {
        if body.trim().is_empty() {
            return;
        }
        let Some(path) = self.current_path.borrow().clone() else {
            return;
        };
        let Some((line, snippet)) = self.on_request_anchor.borrow().as_ref().map(|f| f()) else {
            return;
        };
        let mut t = self.thread.borrow_mut();
        t.add(line, snippet, body);
        t.save(&path);
        drop(t);
        self.rebuild(&[]);
    }

    fn reply_to(&self, id: u64, body: String) {
        let Some(path) = self.current_path.borrow().clone() else {
            return;
        };
        if body.trim().is_empty() {
            return;
        }
        let mut t = self.thread.borrow_mut();
        t.reply(id, body);
        t.save(&path);
        drop(t);
        self.rebuild(&[]);
    }

    fn set_resolved(&self, id: u64, resolved: bool) {
        let Some(path) = self.current_path.borrow().clone() else {
            return;
        };
        let mut t = self.thread.borrow_mut();
        t.set_resolved(id, resolved);
        t.save(&path);
        drop(t);
        self.rebuild(&[]);
    }

    fn resolve_suggestion(&self, id: u64, accepted: bool) {
        let Some(path) = self.current_path.borrow().clone() else {
            return;
        };
        let mut t = self.thread.borrow_mut();
        let Some(sugg) = t
            .comments
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| c.suggestion.clone())
        else {
            return;
        };
        let line = t
            .comments
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.anchor_line)
            .unwrap_or(0);
        let status = if accepted {
            SuggestionStatus::Accepted
        } else {
            SuggestionStatus::Rejected
        };
        t.set_suggestion_status(id, status);
        t.save(&path);
        drop(t);
        if suggestion_removes_text(&sugg.kind, accepted) {
            if let Some(f) = self.on_apply_suggestion.borrow().as_ref() {
                f(line, sugg.text.clone());
            }
        }
        self.rebuild(&[]);
    }

    fn delete(&self, id: u64) {
        let Some(path) = self.current_path.borrow().clone() else {
            return;
        };
        let mut t = self.thread.borrow_mut();
        t.delete(id);
        t.save(&path);
        drop(t);
        self.rebuild(&[]);
    }

    fn rebuild(&self, lost_ids: &[u64]) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let thread = self.thread.borrow();
        let count = thread.comments.len();
        self.count_label.set_text(&if count == 0 {
            String::new()
        } else {
            let open = thread.comments.iter().filter(|c| !c.resolved).count();
            format!("· {open} open")
        });

        if thread.comments.is_empty() {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let msg = if self.current_path.borrow().is_some() {
                "No comments yet.\n\nClick + to leave a note at the cursor's\ncurrent line — useful when reviewing\nsomeone else's draft."
            } else {
                "Open a document to see its comments."
            };
            let lbl = Label::new(Some(msg));
            lbl.add_css_class("dim-label");
            lbl.set_justify(gtk4::Justification::Center);
            lbl.set_margin_top(16);
            lbl.set_margin_bottom(16);
            row.set_child(Some(&lbl));
            self.list_box.append(&row);
            return;
        }

        for comment in thread.comments.iter() {
            let row = ListBoxRow::new();
            row.set_activatable(false);
            row.add_css_class("fond-card");
            row.add_css_class("fond-row");

            let outer = GtkBox::new(Orientation::Vertical, 4);
            outer.set_margin_start(8);
            outer.set_margin_end(8);
            outer.set_margin_top(6);
            outer.set_margin_bottom(6);

            let anchor_row = GtkBox::new(Orientation::Horizontal, 6);
            let jump_btn = Button::new();
            jump_btn.add_css_class("flat");
            let anchor_text = if comment.anchor_snippet.trim().is_empty() {
                format!("line {}", comment.anchor_line)
            } else {
                format!(
                    "line {} · {}",
                    comment.anchor_line,
                    truncate(&comment.anchor_snippet, 40)
                )
            };
            let anchor_lbl = Label::new(Some(&anchor_text));
            anchor_lbl.add_css_class("caption");
            anchor_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            jump_btn.set_child(Some(&anchor_lbl));
            let on_jump = self.on_jump.clone();
            let line = comment.anchor_line;
            jump_btn.connect_clicked(move |_| {
                if let Some(f) = on_jump.borrow().as_ref() {
                    f(line);
                }
            });
            anchor_row.append(&jump_btn);
            if lost_ids.contains(&comment.id) {
                let warn = Label::new(Some("⚠ anchor lost"));
                warn.add_css_class("caption");
                warn.add_css_class("error");
                warn.set_tooltip_text(Some(
                    "The text this comment was attached to couldn't be found anymore — showing its last known line",
                ));
                anchor_row.append(&warn);
            }
            outer.append(&anchor_row);

            if let Some(sugg) = &comment.suggestion {
                let (verb, css) = match sugg.kind {
                    SuggestionKind::Insertion => ("Insert", "success"),
                    SuggestionKind::Deletion => ("Delete", "error"),
                };
                let sugg_row = GtkBox::new(Orientation::Horizontal, 4);
                let kind_lbl = Label::new(Some(verb));
                kind_lbl.add_css_class("caption");
                kind_lbl.add_css_class(css);
                sugg_row.append(&kind_lbl);
                let text_lbl = Label::new(Some(&format!(
                    "\u{201c}{}\u{201d}",
                    truncate(&sugg.text, 60)
                )));
                text_lbl.set_xalign(0.0);
                text_lbl.set_wrap(true);
                text_lbl.set_selectable(true);
                sugg_row.append(&text_lbl);
                outer.append(&sugg_row);
            }

            if !comment.body.trim().is_empty() {
                let body_lbl = Label::new(Some(&comment.body));
                body_lbl.set_xalign(0.0);
                body_lbl.set_wrap(true);
                body_lbl.set_selectable(true);
                outer.append(&body_lbl);
            }

            for reply in &comment.replies {
                let reply_box = GtkBox::new(Orientation::Horizontal, 4);
                reply_box.set_margin_start(12);
                let bullet = Label::new(Some("↳"));
                bullet.add_css_class("dim-label");
                let reply_lbl = Label::new(Some(&reply.body));
                reply_lbl.set_xalign(0.0);
                reply_lbl.set_wrap(true);
                reply_lbl.set_selectable(true);
                reply_box.append(&bullet);
                reply_box.append(&reply_lbl);
                outer.append(&reply_box);
            }

            let action_row = GtkBox::new(Orientation::Horizontal, 6);
            action_row.set_margin_top(2);

            let reply_btn = Button::with_label("Reply");
            reply_btn.add_css_class("flat");
            reply_btn.add_css_class("caption");
            {
                let panel = self.clone();
                let id = comment.id;
                reply_btn.connect_clicked(move |btn| {
                    let panel = panel.clone();
                    open_composer_popover(btn, "Reply", "Reply", move |body| {
                        panel.reply_to(id, body);
                    });
                });
            }
            action_row.append(&reply_btn);

            match comment.suggestion.as_ref().map(|s| s.status.clone()) {
                Some(SuggestionStatus::Pending) => {
                    // Not `.flat`: libadwaita's `button.suggested-action` sets
                    // white (accent-fg) text unconditionally, and `.flat`
                    // drops the accent background that text is meant to sit
                    // on — combined they render invisible white-on-white.
                    // Every other suggested-action button in this codebase is
                    // non-flat for the same reason; found live, not by
                    // inspection, when this one came out as a blank gap in a
                    // screenshot next to a working flat destructive-action.
                    let accept_btn = Button::with_label("Accept");
                    accept_btn.add_css_class("caption");
                    accept_btn.add_css_class("suggested-action");
                    {
                        let panel = self.clone();
                        let id = comment.id;
                        accept_btn.connect_clicked(move |_| panel.resolve_suggestion(id, true));
                    }
                    action_row.append(&accept_btn);

                    let reject_btn = Button::with_label("Reject");
                    reject_btn.add_css_class("flat");
                    reject_btn.add_css_class("caption");
                    reject_btn.add_css_class("destructive-action");
                    {
                        let panel = self.clone();
                        let id = comment.id;
                        reject_btn.connect_clicked(move |_| panel.resolve_suggestion(id, false));
                    }
                    action_row.append(&reject_btn);
                }
                Some(SuggestionStatus::Accepted) => {
                    let badge = Label::new(Some("✓ accepted"));
                    badge.add_css_class("caption");
                    badge.add_css_class("dim-label");
                    action_row.append(&badge);
                }
                Some(SuggestionStatus::Rejected) => {
                    let badge = Label::new(Some("✗ rejected"));
                    badge.add_css_class("caption");
                    badge.add_css_class("dim-label");
                    action_row.append(&badge);
                }
                None => {
                    let resolve_btn = Button::with_label(if comment.resolved {
                        "Reopen"
                    } else {
                        "Resolve"
                    });
                    resolve_btn.add_css_class("flat");
                    resolve_btn.add_css_class("caption");
                    {
                        let panel = self.clone();
                        let id = comment.id;
                        let now_resolved = !comment.resolved;
                        resolve_btn.connect_clicked(move |_| {
                            panel.set_resolved(id, now_resolved);
                        });
                    }
                    action_row.append(&resolve_btn);

                    if comment.resolved {
                        let badge = Label::new(Some("✓ resolved"));
                        badge.add_css_class("caption");
                        badge.add_css_class("dim-label");
                        action_row.append(&badge);
                    }
                }
            }

            let delete_btn = Button::from_icon_name("user-trash-symbolic");
            delete_btn.add_css_class("flat");
            delete_btn.set_tooltip_text(Some("Delete this comment"));
            {
                let panel = self.clone();
                let id = comment.id;
                delete_btn.connect_clicked(move |_| {
                    panel.delete(id);
                });
            }
            action_row.append(&delete_btn);

            outer.append(&action_row);
            row.set_child(Some(&outer));
            self.list_box.append(&row);
        }
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let short: String = s.chars().take(max_chars).collect();
        format!("{short}…")
    }
}

/// A small popover with a multi-line text view and a confirm button —
/// shared shape for both "Add Comment" and "Reply", matching
/// `ref_manager.rs`'s `open_rename_popover` pattern (single-line there;
/// multi-line here since a comment body is prose, not an identifier).
fn open_composer_popover(
    anchor: &impl IsA<gtk4::Widget>,
    heading: &str,
    confirm_label: &str,
    on_confirm: impl Fn(String) + 'static,
) {
    let popover = Popover::new();
    popover.set_parent(anchor);

    let vbox = GtkBox::new(Orientation::Vertical, 6);
    vbox.set_margin_top(10);
    vbox.set_margin_bottom(10);
    vbox.set_margin_start(10);
    vbox.set_margin_end(10);
    vbox.set_width_request(260);

    let label = Label::new(Some(heading));
    label.set_xalign(0.0);
    label.add_css_class("caption");
    vbox.append(&label);

    let text_view = TextView::new();
    text_view.set_wrap_mode(WrapMode::WordChar);
    text_view.set_top_margin(4);
    text_view.set_bottom_margin(4);
    text_view.set_left_margin(6);
    text_view.set_right_margin(6);
    text_view.add_css_class("card");
    let scroll = ScrolledWindow::new();
    scroll.set_min_content_height(60);
    scroll.set_max_content_height(160);
    scroll.set_child(Some(&text_view));
    vbox.append(&scroll);

    let confirm_btn = Button::with_label(confirm_label);
    confirm_btn.add_css_class("suggested-action");
    vbox.append(&confirm_btn);

    popover.set_child(Some(&vbox));

    let do_confirm: Rc<dyn Fn()> = {
        let buffer = text_view.buffer();
        let popover = popover.clone();
        Rc::new(move || {
            let (start, end) = buffer.bounds();
            let text = buffer.text(&start, &end, false).to_string();
            on_confirm(text);
            popover.popdown();
        })
    };
    {
        let f = do_confirm.clone();
        confirm_btn.connect_clicked(move |_| f());
    }

    popover.popup();
    text_view.grab_focus();
}

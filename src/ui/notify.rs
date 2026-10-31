//! Every toast goes through here, so that a message stays long enough to be
//! read and can be read again afterwards.
//!
//! libadwaita's toasts vanish after five seconds unless told otherwise, and
//! call sites had picked anything from two to fifteen — so a 25-word message
//! could be gone before it was finished, and once gone it was gone. Here the
//! time on screen follows the length of the message, a message with a button
//! gets long enough to reach the button, "Messages stay until I close them"
//! (Settings → Comfort) keeps every one, and each is kept in a short list of
//! recent messages.

use std::cell::RefCell;
use std::collections::VecDeque;

use adw::prelude::*;
use libadwaita as adw;

const KEEP: usize = 30;

thread_local! {
    static RECENT: RefCell<VecDeque<(chrono::DateTime<chrono::Local>, String)>> =
        const { RefCell::new(VecDeque::new()) };
    static BRIEF: RefCell<Option<adw::Toast>> = const { RefCell::new(None) };
}

/// Seconds a message of `words` words stays up: about a second for every
/// three words, at least 6 and at most 20, and at least 10 when it has a
/// button to press.
pub fn seconds_for(words: usize, has_button: bool) -> u32 {
    let base = (words as u32).div_ceil(3).clamp(6, 20);
    if has_button {
        base.max(10)
    } else {
        base
    }
}

pub trait Say {
    /// Shows `toast` for as long as its words need, and keeps it in the list
    /// of recent messages. A toast set to stay (timeout 0) still stays.
    fn say(&self, toast: adw::Toast);
}

impl Say for adw::ToastOverlay {
    fn say(&self, toast: adw::Toast) {
        let text = toast.title().map(|t| t.to_string()).unwrap_or_default();
        let words = text.split_whitespace().count();
        let stays = toast.timeout() == 0 || crate::config::shared().borrow().comfort.messages_stay;
        toast.set_timeout(if stays {
            0
        } else {
            seconds_for(words, toast.button_label().is_some())
        });
        remember(&text);
        self.add_toast(toast);
    }
}

/// A readout that the next one replaces straight away — "Text size 14" while
/// someone presses Ctrl+= a few times — rather than a message to read. Not
/// kept in the recent list.
pub fn say_briefly(overlay: &adw::ToastOverlay, text: &str) {
    BRIEF.with(|b| {
        if let Some(old) = b.borrow_mut().take() {
            old.dismiss();
        }
        let t = adw::Toast::new(text);
        t.set_timeout(2);
        *b.borrow_mut() = Some(t.clone());
        overlay.add_toast(t);
    });
}

fn remember(text: &str) {
    if text.is_empty() {
        return;
    }
    RECENT.with(|r| {
        let mut r = r.borrow_mut();
        r.push_back((chrono::Local::now(), text.to_string()));
        while r.len() > KEEP {
            r.pop_front();
        }
    });
}

/// "9:42" or "9:42 pm" — whatever the desktop's own clock shows.
fn friendly_time(t: &chrono::DateTime<chrono::Local>) -> String {
    t.format("%-I:%M %p").to_string().to_lowercase()
}

/// A popover with what Zerkalo has said lately, newest first, so a message
/// that went by too fast can be read again.
pub fn show_recent(parent: &impl IsA<gtk4::Widget>) {
    use gtk4::{Box as GtkBox, Label, Orientation, Popover, ScrolledWindow};

    let list = GtkBox::new(Orientation::Vertical, 14);
    list.set_margin_start(18);
    list.set_margin_end(18);
    list.set_margin_top(16);
    list.set_margin_bottom(16);
    let heading = Label::new(Some("Things Zerkalo has told you"));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    list.append(&heading);
    let items = RECENT.with(|r| r.borrow().iter().rev().cloned().collect::<Vec<_>>());
    if items.is_empty() {
        let none = Label::new(Some(
            "Nothing yet. When Zerkalo tells you something, you can read it again here.",
        ));
        none.add_css_class("dim-label");
        none.set_xalign(0.0);
        none.set_wrap(true);
        none.set_width_chars(34);
        none.set_max_width_chars(44);
        list.append(&none);
    }
    for (time, text) in items {
        let row = GtkBox::new(Orientation::Vertical, 2);
        let when = Label::new(Some(&friendly_time(&time)));
        when.add_css_class("dim-label");
        when.add_css_class("caption");
        when.set_xalign(0.0);
        let msg = Label::new(Some(&text));
        msg.set_wrap(true);
        msg.set_xalign(0.0);
        msg.set_selectable(true);
        msg.set_width_chars(34);
        msg.set_max_width_chars(44);
        row.append(&when);
        row.append(&msg);
        list.append(&row);
    }
    let scroll = ScrolledWindow::new();
    scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
    scroll.set_propagate_natural_height(true);
    scroll.set_propagate_natural_width(true);
    scroll.set_max_content_height(440);
    scroll.set_child(Some(&list));

    let pop = Popover::new();
    pop.set_child(Some(&scroll));
    pop.set_parent(parent);
    pop.connect_closed(|p| p.unparent());
    pop.popup();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer_messages_stay_longer_within_limits() {
        assert_eq!(seconds_for(3, false), 6);
        assert_eq!(seconds_for(27, false), 9);
        assert_eq!(seconds_for(200, false), 20);
        assert_eq!(seconds_for(3, true), 10);
    }

    /// A toast added straight to an overlay skips the reading time and the
    /// recent list, so every one goes through `Say::say` (or `say_briefly`).
    #[test]
    fn every_toast_goes_through_say() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        walk(&root.join("src"), &mut files);
        let mut hits = Vec::new();
        for f in files {
            if f.ends_with("ui/notify.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            let body = text.split("#[cfg(test)]").next().unwrap_or(&text);
            for (n, line) in body.lines().enumerate() {
                if line.contains(".add_toast(") && !line.trim_start().starts_with("//") {
                    hits.push(format!("{}:{}", f.display(), n + 1));
                }
            }
        }
        assert!(
            hits.is_empty(),
            "toasts added without notify::Say:\n{}",
            hits.join("\n")
        );
    }
}

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Label, Orientation, Revealer};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

type Action = Rc<RefCell<Option<Box<dyn Fn()>>>>;

/// A bar above the editor that stays until a failed save goes through — a
/// toast that fades is easy to miss, and the person needs to know their work
/// is not on disk.
#[derive(Clone)]
pub struct SaveProblemBar {
    revealer: Revealer,
    label: Label,
    on_retry: Action,
    on_copy: Action,
}

impl SaveProblemBar {
    pub fn new() -> Self {
        let row = GtkBox::new(Orientation::Horizontal, 8);
        row.add_css_class("save-problem");

        let icon = Image::from_icon_name("dialog-warning-symbolic");
        icon.set_valign(gtk4::Align::Start);
        icon.set_margin_top(2);
        row.append(&icon);

        let label = Label::new(None);
        label.set_wrap(true);
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.set_selectable(false);
        row.append(&label);

        let retry = Button::with_label("Try again");
        let copy = Button::with_label("Save a copy elsewhere…");
        copy.add_css_class("flat");
        row.append(&copy);
        row.append(&retry);

        let revealer = Revealer::new();
        revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
        revealer.set_child(Some(&row));
        revealer.set_reveal_child(false);

        let on_retry: Action = Rc::new(RefCell::new(None));
        let on_copy: Action = Rc::new(RefCell::new(None));
        for (btn, action) in [(&retry, &on_retry), (&copy, &on_copy)] {
            let action = action.clone();
            btn.connect_clicked(move |_| {
                if let Some(f) = action.borrow().as_ref() {
                    f();
                }
            });
        }

        Self {
            revealer,
            label,
            on_retry,
            on_copy,
        }
    }

    pub fn widget(&self) -> &Revealer {
        &self.revealer
    }

    pub fn show(&self, problems: &[(PathBuf, String)]) {
        if problems.is_empty() {
            self.revealer.set_reveal_child(false);
            return;
        }
        self.label.set_text(&message(problems));
        self.revealer.set_reveal_child(true);
    }

    pub fn set_on_retry(&self, f: impl Fn() + 'static) {
        *self.on_retry.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_copy(&self, f: impl Fn() + 'static) {
        *self.on_copy.borrow_mut() = Some(Box::new(f));
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub fn message(problems: &[(PathBuf, String)]) -> String {
    let [(path, reason)] = problems else {
        let same = problems.iter().all(|(_, r)| *r == problems[0].1);
        let names = |with_reason: bool| {
            problems
                .iter()
                .map(|(p, r)| {
                    if with_reason {
                        format!("{} ({r})", file_name(p))
                    } else {
                        file_name(p)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        return if same {
            format!(
                "Couldn't save {} files: {} — {}. Your changes are still here.",
                problems.len(),
                names(false),
                problems[0].1
            )
        } else {
            format!(
                "Couldn't save {} files: {}. Your changes are still here.",
                problems.len(),
                names(true)
            )
        };
    };
    format!(
        "Couldn't save {} — {reason}. Your changes are still here.",
        file_name(path)
    )
}

/// `name.typ` → `name.typ`, or `name (copy).typ`, `name (copy 2).typ` … when
/// something already sits at that name in `dir`.
pub fn free_copy_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let original = Path::new(name);
    let stem = original
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    let ext = original
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut n = 1;
    loop {
        let tag = if n == 1 {
            "copy".to_string()
        } else {
            format!("copy {n}")
        };
        let candidate = dir.join(format!("{stem} ({tag}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, reason: &str) -> (PathBuf, String) {
        (
            PathBuf::from(format!("/home/x/Zerkalo/{name}")),
            reason.into(),
        )
    }

    #[test]
    fn one_file_names_the_file_and_the_reason() {
        assert_eq!(
            message(&[p("essay.typ", "the disk is full")]),
            "Couldn't save essay.typ — the disk is full. Your changes are still here."
        );
    }

    #[test]
    fn several_files_with_one_reason_say_it_once() {
        let m = message(&[
            p("a.typ", "the disk is full"),
            p("b.typ", "the disk is full"),
        ]);
        assert_eq!(
            m,
            "Couldn't save 2 files: a.typ, b.typ — the disk is full. Your changes are still here."
        );
    }

    #[test]
    fn several_files_with_different_reasons_list_each() {
        let m = message(&[
            p("a.typ", "the disk is full"),
            p("b.typ", "that location is read-only"),
        ]);
        assert!(m.contains("a.typ (the disk is full), b.typ (that location is read-only)"));
    }

    #[test]
    fn copy_never_overwrites_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            free_copy_path(dir.path(), "essay.typ"),
            dir.path().join("essay.typ")
        );
        std::fs::write(dir.path().join("essay.typ"), "").unwrap();
        assert_eq!(
            free_copy_path(dir.path(), "essay.typ"),
            dir.path().join("essay (copy).typ")
        );
        std::fs::write(dir.path().join("essay (copy).typ"), "").unwrap();
        assert_eq!(
            free_copy_path(dir.path(), "essay.typ"),
            dir.path().join("essay (copy 2).typ")
        );
    }
}

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, SelectionMode,
    Separator, TextTag, TextView, WrapMode,
};

#[derive(Clone)]
pub struct HistoryPanel {
    widget: GtkBox,
    list_box: ListBox,
    diff_view: TextView,
    diff_commits: Rc<RefCell<Vec<String>>>,
    project_root: Rc<PathBuf>,
    current_file: Rc<RefCell<Option<PathBuf>>>,
}

impl HistoryPanel {
    pub fn new(project_root: PathBuf) -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let header = GtkBox::new(Orientation::Horizontal, 0);
        header.set_margin_start(10);
        header.set_margin_end(10);
        header.set_margin_top(6);
        header.set_margin_bottom(6);
        let title = Label::new(Some("History"));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("heading");
        header.append(&title);

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&header);
        widget.append(&Separator::new(Orientation::Horizontal));

        // Commit list (upper portion)
        let commit_scroll = ScrolledWindow::new();
        commit_scroll.set_vexpand(true);
        commit_scroll.set_size_request(-1, 200);
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Single);
        list_box.add_css_class("navigation-sidebar");
        commit_scroll.set_child(Some(&list_box));
        widget.append(&commit_scroll);

        widget.append(&Separator::new(Orientation::Horizontal));

        // Diff view with color tags
        let diff_buf = gtk4::TextBuffer::new(None);
        let colors = crate::ui::theme::diff_colors();
        let tag_removed = TextTag::new(Some("removed"));
        tag_removed.set_property("background", colors.removed_bg);
        tag_removed.set_property("foreground", colors.removed_fg);
        let tag_added = TextTag::new(Some("added"));
        tag_added.set_property("background", colors.added_bg);
        tag_added.set_property("foreground", colors.added_fg);
        diff_buf.tag_table().add(&tag_removed);
        diff_buf.tag_table().add(&tag_added);

        let diff_scroll = ScrolledWindow::new();
        diff_scroll.set_vexpand(true);
        let diff_view = TextView::with_buffer(&diff_buf);
        diff_view.set_editable(false);
        diff_view.set_monospace(true);
        diff_view.set_wrap_mode(WrapMode::None);
        diff_view.set_margin_start(6);
        diff_view.set_margin_end(6);
        diff_view.set_margin_top(4);
        diff_view.set_margin_bottom(4);
        diff_scroll.set_child(Some(&diff_view));
        widget.append(&diff_scroll);

        let current_file: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
        let diff_commits: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

        // Wire selection → colored diff
        {
            let commits = diff_commits.clone();
            let root = Rc::new(project_root.clone());
            let file_ref = current_file.clone();
            list_box.connect_row_selected(move |_, row| {
                let Some(row) = row else { return };
                let idx = row.index() as usize;
                let oids = commits.borrow();
                let Some(oid) = oids.get(idx) else { return };
                let Some(ref fp) = *file_ref.borrow() else {
                    return;
                };
                let diff = git_diff_for_commit(&root, fp, oid);
                apply_colored_diff(&diff_buf, &diff);
            });
        }

        Self {
            widget,
            list_box,
            diff_view,
            diff_commits,
            project_root: Rc::new(project_root),
            current_file,
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn load_file_history(&self, file_path: &Path) {
        *self.current_file.borrow_mut() = Some(file_path.to_path_buf());

        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }
        self.diff_view.buffer().set_text("");
        self.diff_commits.borrow_mut().clear();

        let commits = git_log_for_file(&self.project_root, file_path);

        if commits.is_empty() {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let lbl = Label::new(Some("No earlier versions of this file yet.\nSync to start keeping a history of changes."));
            lbl.add_css_class("dim-label");
            lbl.set_justify(gtk4::Justification::Center);
            lbl.set_margin_top(16);
            lbl.set_margin_bottom(16);
            row.set_child(Some(&lbl));
            self.list_box.append(&row);
            return;
        }

        for (oid, summary, date) in commits {
            let row = ListBoxRow::new();

            let row_box = GtkBox::new(Orientation::Vertical, 2);
            row_box.set_margin_start(8);
            row_box.set_margin_end(8);
            row_box.set_margin_top(4);
            row_box.set_margin_bottom(4);

            let msg_lbl = Label::new(Some(&summary));
            msg_lbl.set_xalign(0.0);
            msg_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

            let short_oid = if oid.len() >= 8 { &oid[..8] } else { &oid };
            let meta_lbl = Label::new(Some(&format!("{} · {}", short_oid, date)));
            meta_lbl.add_css_class("caption");
            meta_lbl.add_css_class("dim-label");
            meta_lbl.set_xalign(0.0);

            row_box.append(&msg_lbl);
            row_box.append(&meta_lbl);
            row.set_child(Some(&row_box));

            self.diff_commits.borrow_mut().push(oid);
            self.list_box.append(&row);
        }
    }

    #[allow(dead_code)]
    pub fn refresh(&self) {
        if let Some(path) = self.current_file.borrow().clone() {
            self.load_file_history(&path);
        }
    }
}

fn apply_colored_diff(buf: &gtk4::TextBuffer, diff: &str) {
    let cleaned = crate::ui::diff_render::clean_unified_diff(diff);
    crate::ui::diff_render::render_clean_diff(buf, &cleaned);
}

fn git_log_for_file(root: &Path, file: &Path) -> Vec<(String, String, String)> {
    // Uses git_sync's `-C <repo>` invocation (rather than host_command() +
    // current_dir()) — under flatpak, current_dir() only sets the sandboxed
    // flatpak-spawn wrapper's cwd, not the host git process's, so it isn't a
    // reliable way to point git at the right repo there. `-C` is a host-side
    // git argument and works regardless.
    let out = crate::git_sync::git_cmd(root)
        .args([
            "log",
            "--follow",
            "--format=%H|%s|%cd",
            "--date=short",
            "--",
        ])
        .arg(file)
        .output();

    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|line| {
                // %H (hash) and %cd (date, --date=short) never contain '|', but
                // %s (subject) can if the commit message itself has one — so
                // split the date off the end first, then take the hash off the
                // front, leaving everything in between as the subject.
                let mut from_end = line.rsplitn(2, '|');
                let date = from_end.next()?.to_string();
                let rest = from_end.next()?;
                let mut from_start = rest.splitn(2, '|');
                let oid = from_start.next()?.to_string();
                let summary = from_start.next()?.to_string();
                Some((oid, summary, date))
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn git_diff_for_commit(root: &Path, file: &Path, oid: &str) -> String {
    let out = crate::git_sync::git_cmd(root)
        .args(["show", "--stat", "--patch", oid, "--"])
        .arg(file)
        .output();

    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(e) => format!("Could not load diff: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .expect("git must be on PATH for this test");
        assert!(status.success(), "git {args:?} failed in {root:?}");
    }

    /// Sets up a repo with two commits touching `main.typ`, so history/diff
    /// lookups have real data to find.
    fn repo_with_history() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        git(&root, &["init", "-q"]);
        git(&root, &["config", "user.email", "test@example.com"]);
        git(&root, &["config", "user.name", "Test"]);

        let file = root.join("main.typ");
        std::fs::write(&file, "= First\n").unwrap();
        git(&root, &["add", "main.typ"]);
        git(&root, &["commit", "-q", "-m", "first commit"]);

        std::fs::write(&file, "= First\n\n= Second\n").unwrap();
        git(&root, &["add", "main.typ"]);
        git(&root, &["commit", "-q", "-m", "second commit"]);

        (dir, file)
    }

    #[test]
    fn git_log_for_file_lists_commits_newest_first() {
        let (dir, file) = repo_with_history();
        let commits = git_log_for_file(dir.path(), &file);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].1, "second commit");
        assert_eq!(commits[1].1, "first commit");
    }

    #[test]
    fn git_log_for_file_is_empty_outside_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("untracked.typ");
        std::fs::write(&file, "content").unwrap();
        let commits = git_log_for_file(dir.path(), &file);
        assert!(commits.is_empty());
    }

    #[test]
    fn git_diff_for_commit_shows_the_added_heading() {
        let (dir, file) = repo_with_history();
        let commits = git_log_for_file(dir.path(), &file);
        let newest_oid = &commits[0].0;
        let diff = git_diff_for_commit(dir.path(), &file, newest_oid);
        assert!(diff.contains("+= Second"), "diff was: {diff}");
    }
}

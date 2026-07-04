use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::cell::RefCell;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, SelectionMode, Separator, TextView, TextTag, WrapMode,
};
use libadwaita as adw;
use adw::prelude::*;

const MAX_SNAPSHOTS: usize = 100;

// ── Snapshot paths ────────────────────────────────────────────────────────────

pub fn snapshot_dir(project_root: &Path, file_path: &Path) -> PathBuf {
    let base = shellexpand::tilde("~/.local/share/zerkalo/snapshots").into_owned();
    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let file_stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed");
    PathBuf::from(base).join(project_name).join(file_stem)
}

/// Save a snapshot of `content` for `file_path` under `project_root`.
/// Keeps only the last MAX_SNAPSHOTS snapshots; deletes the oldest when over.
pub fn save_snapshot(project_root: &Path, file_path: &Path, content: &str) {
    let dir = snapshot_dir(project_root, file_path);
    if std::fs::create_dir_all(&dir).is_err() { return; }

    let ts = chrono::Local::now().format("%Y%m%dT%H%M%S%.3f").to_string();
    let snap_path = dir.join(format!("{ts}.typ"));
    let tmp_path = dir.join(format!("{ts}.typ.tmp"));
    if std::fs::write(&tmp_path, content).is_ok() {
        let _ = std::fs::rename(&tmp_path, &snap_path);
    }

    // Prune oldest if over limit
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("typ"))
            .map(|e| e.path())
            .collect();
        files.sort();
        while files.len() > MAX_SNAPSHOTS {
            let _ = std::fs::remove_file(&files[0]);
            files.remove(0);
        }
    }
}

fn list_snapshots(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("typ"))
        .map(|e| e.path())
        .collect();
    files.sort_by(|a, b| b.cmp(a)); // newest first
    files
}

fn simple_diff(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let m = old_lines.len().min(600);
    let n = new_lines.len().min(600);
    let old_lines = &old_lines[..m];
    let new_lines = &new_lines[..n];

    // LCS DP table
    let mut dp = vec![vec![0u16; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if old_lines[i - 1] == new_lines[j - 1] {
                dp[i - 1][j - 1] + 1
            } else {
                dp[i - 1][j].max(dp[i][j - 1])
            };
        }
    }

    // Backtrack
    let mut diff: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (m, n);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            diff.push((' ', old_lines[i - 1]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            diff.push(('+', new_lines[j - 1]));
            j -= 1;
        } else {
            diff.push(('-', old_lines[i - 1]));
            i -= 1;
        }
    }
    diff.reverse();

    // Render with 2-line context around changes
    let changed: Vec<bool> = diff.iter().map(|(c, _)| *c != ' ').collect();
    let mut show = vec![false; diff.len()];
    for k in 0..diff.len() {
        if changed[k] {
            let s = k.saturating_sub(2);
            let e = (k + 3).min(diff.len());
            for idx in s..e { show[idx] = true; }
        }
    }

    let mut out = String::new();
    let mut gap = false;
    for (idx, (ch, line)) in diff.iter().enumerate() {
        if !show[idx] {
            gap = true;
            continue;
        }
        if gap { out.push_str("...\n"); gap = false; }
        match ch {
            '-' => out.push_str(&format!("- {line}\n")),
            '+' => out.push_str(&format!("+ {line}\n")),
            _ => out.push_str(&format!("  {line}\n")),
        }
    }

    if out.is_empty() { "(no differences)".to_string() } else { out }
}

// ── SnapshotDialog ────────────────────────────────────────────────────────────

pub struct SnapshotDialog {
    window: adw::Window,
    on_restore: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
}

impl SnapshotDialog {
    pub fn new(
        parent: &impl IsA<gtk4::Window>,
        project_root: &Path,
        file_path: &Path,
        current_content: &str,
    ) -> Self {
        let window = adw::Window::builder()
            .title("Browse Snapshots")
            .transient_for(parent)
            .modal(true)
            .default_width(800)
            .default_height(600)
            .build();

        let on_restore: Rc<RefCell<Option<Box<dyn Fn(String)>>>> =
            Rc::new(RefCell::new(None));

        let header = adw::HeaderBar::new();
        let close_btn = Button::with_label("Close");
        header.pack_end(&close_btn);

        let content_box = GtkBox::new(Orientation::Vertical, 0);
        content_box.append(&header);

        let body = GtkBox::new(Orientation::Horizontal, 0);
        body.set_hexpand(true);
        body.set_vexpand(true);
        content_box.append(&body);

        // ── Left: snapshot list ───────────────────────────────────────────────
        let left = GtkBox::new(Orientation::Vertical, 0);
        left.set_width_request(220);

        let list_header = Label::new(Some("Snapshots"));
        list_header.add_css_class("heading");
        list_header.set_margin_start(12);
        list_header.set_margin_top(8);
        list_header.set_margin_bottom(8);
        list_header.set_xalign(0.0);
        left.append(&list_header);
        left.append(&Separator::new(Orientation::Horizontal));

        let list_scroll = ScrolledWindow::new();
        list_scroll.set_vexpand(true);
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Single);
        list_box.add_css_class("navigation-sidebar");
        list_scroll.set_child(Some(&list_box));
        left.append(&list_scroll);

        body.append(&left);
        body.append(&Separator::new(Orientation::Vertical));

        // ── Right: diff + restore ─────────────────────────────────────────────
        let right = GtkBox::new(Orientation::Vertical, 0);
        right.set_hexpand(true);

        let diff_scroll = ScrolledWindow::new();
        diff_scroll.set_vexpand(true);
        let diff_buf = gtk4::TextBuffer::new(None);
        let tag_removed = TextTag::new(Some("removed"));
        tag_removed.set_property("background", "#5c1f1f");
        tag_removed.set_property("foreground", "#ff9999");
        let tag_added = TextTag::new(Some("added"));
        tag_added.set_property("background", "#1a3a1a");
        tag_added.set_property("foreground", "#99dd99");
        diff_buf.tag_table().add(&tag_removed);
        diff_buf.tag_table().add(&tag_added);
        let diff_view = TextView::with_buffer(&diff_buf);
        diff_view.set_editable(false);
        diff_view.set_monospace(true);
        diff_view.set_wrap_mode(WrapMode::None);
        diff_view.set_margin_start(8);
        diff_view.set_margin_end(8);
        diff_view.set_margin_top(6);
        diff_view.set_margin_bottom(6);
        diff_scroll.set_child(Some(&diff_view));
        right.append(&diff_scroll);

        right.append(&Separator::new(Orientation::Horizontal));

        let restore_bar = GtkBox::new(Orientation::Horizontal, 8);
        restore_bar.set_margin_start(12);
        restore_bar.set_margin_end(12);
        restore_bar.set_margin_top(8);
        restore_bar.set_margin_bottom(8);
        let restore_info = Label::new(Some("Select a snapshot to see changes"));
        restore_info.add_css_class("dim-label");
        restore_info.set_hexpand(true);
        restore_info.set_xalign(0.0);
        let restore_btn = Button::with_label("Restore");
        restore_btn.add_css_class("suggested-action");
        restore_btn.set_sensitive(false);
        restore_bar.append(&restore_info);
        restore_bar.append(&restore_btn);
        right.append(&restore_bar);

        body.append(&right);

        window.set_content(Some(&content_box));

        // ── Populate snapshot list ─────────────────────────────────────────────
        let dir = snapshot_dir(project_root, file_path);
        let snapshots = list_snapshots(&dir);
        let current_text = current_content.to_string();

        let selected_content: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let snapshot_paths: Rc<Vec<PathBuf>> = Rc::new(snapshots.clone());

        for snap_path in &snapshots {
            let name = snap_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            // Format: YYYYMMDDTHHMMSS.mmm → "YYYY-MM-DD HH:MM:SS"
            let display = if name.len() >= 15 {
                format!(
                    "{}-{}-{} {}:{}:{}",
                    &name[0..4], &name[4..6], &name[6..8],
                    &name[9..11], &name[11..13], &name[13..15]
                )
            } else {
                name.clone()
            };

            let row = ListBoxRow::new();
            let lbl = Label::new(Some(&display));
            lbl.set_xalign(0.0);
            lbl.set_margin_start(12);
            lbl.set_margin_top(6);
            lbl.set_margin_bottom(6);
            row.set_child(Some(&lbl));
            list_box.append(&row);
        }

        // Single-click selection drives the diff view
        {
            let paths = snapshot_paths.clone();
            let current_clone = current_text.clone();
            let buf = diff_buf.clone();
            let sel = selected_content.clone();
            let restore_btn_c = restore_btn.clone();
            let info_c = restore_info.clone();
            list_box.connect_row_selected(move |_, row| {
                let Some(row) = row else { return };
                let idx = row.index() as usize;
                let Some(snap_path) = paths.get(idx) else { return };
                let Ok(snap_text) = std::fs::read_to_string(snap_path) else { return };
                let diff = simple_diff(&snap_text, &current_clone);
                buf.set_text("");
                let mut iter = buf.start_iter();
                for line in diff.lines() {
                    let tag_name = if line.starts_with("- ") {
                        Some("removed")
                    } else if line.starts_with("+ ") {
                        Some("added")
                    } else {
                        None
                    };
                    let line_with_nl = format!("{line}\n");
                    if let Some(name) = tag_name {
                        if let Some(tag) = buf.tag_table().lookup(name) {
                            buf.insert_with_tags(&mut iter, &line_with_nl, &[&tag]);
                        } else {
                            buf.insert(&mut iter, &line_with_nl);
                        }
                    } else {
                        buf.insert(&mut iter, &line_with_nl);
                    }
                }
                *sel.borrow_mut() = Some(snap_text.clone());
                restore_btn_c.set_sensitive(true);
                let wc = snap_text.split_whitespace().count();
                info_c.set_text(&format!("{wc} words in this snapshot"));
            });
        }

        if snapshots.is_empty() {
            let row = ListBoxRow::new();
            row.set_activatable(false);
            row.set_selectable(false);
            let lbl = Label::new(Some("No snapshots yet.\nSnapshots are saved automatically on Ctrl+S."));
            lbl.add_css_class("dim-label");
            lbl.set_justify(gtk4::Justification::Center);
            lbl.set_margin_top(16);
            lbl.set_margin_bottom(16);
            row.set_child(Some(&lbl));
            list_box.append(&row);
        }

        // ── Wire restore button ───────────────────────────────────────────────
        {
            let sel = selected_content.clone();
            let cb = on_restore.clone();
            let win = window.clone();
            restore_btn.connect_clicked(move |_| {
                if let Some(ref text) = *sel.borrow() {
                    if let Some(f) = cb.borrow().as_ref() {
                        f(text.clone());
                    }
                    win.close();
                }
            });
        }

        {
            let win = window.clone();
            close_btn.connect_clicked(move |_| win.close());
        }

        Self { window, on_restore }
    }

    pub fn set_on_restore(&self, f: impl Fn(String) + 'static) {
        *self.on_restore.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.window.present();
    }
}

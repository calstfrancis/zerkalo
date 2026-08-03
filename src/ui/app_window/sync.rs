//! Git sync, the GitHub token prompt, and the backup-remote manager.
//! Split out of `app_window.rs`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation};
use libadwaita as adw;
use adw::prelude::*;

use crate::config::Config;
use crate::git_sync;
use super::show_alert;

pub(super) fn do_sync(
    root: PathBuf,
    window: adw::ApplicationWindow,
    overlay: adw::ToastOverlay,
    btn: Button,
    token: Option<String>,
    current_config: Rc<RefCell<Config>>,
) {
    use std::sync::mpsc::TryRecvError;

    btn.set_sensitive(false);

    let root_for_thread = root.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel::<git_sync::SyncResult>(1);
    std::thread::spawn(move || {
        tx.send(git_sync::sync(&root_for_thread, token.as_deref())).ok();
    });

    let rx = Rc::new(rx);
    glib::timeout_add_local(Duration::from_millis(100), move || match rx.try_recv() {
        Ok(result) => {
            btn.set_sensitive(true);
            show_sync_result(&window, &overlay, result, root.clone(), current_config.clone());
            glib::ControlFlow::Break
        }
        Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(TryRecvError::Disconnected) => {
            btn.set_sensitive(true);
            glib::ControlFlow::Break
        }
    });
}

fn show_sync_result(
    window: &adw::ApplicationWindow,
    overlay: &adw::ToastOverlay,
    result: git_sync::SyncResult,
    root: PathBuf,
    current_config: Rc<RefCell<Config>>,
) {
    if let Some(err) = result.error {
        show_alert(window, "Sync Failed", &err);
        return;
    }
    if !result.push_errors.is_empty() {
        let detail = result.push_errors.join("\n");
        if result.auth_failed {
            show_github_token_dialog(
                window,
                overlay,
                root,
                current_config,
                "GitHub authentication failed. Enter a Personal Access Token (PAT) to continue.\n\nGenerate one at github.com → Settings → Developer settings → Personal access tokens.",
            );
            return;
        }
        let is_conflict = detail.contains("CONFLICT") || detail.contains("Pull failed");
        if result.pushed {
            let summary = result.commit_message.lines().next().unwrap_or("Synced").to_string();
            overlay.add_toast(adw::Toast::new(&format!("Synced — {summary}")));
            show_alert(window, "Some remotes failed", &detail);
        } else if is_conflict {
            show_alert(
                window,
                "Merge conflict — sync aborted",
                "Remote changes conflict with your local edits. Your work is safe and unchanged.\n\nResolve the conflict by editing the file manually or force-pushing from the command line.",
            );
        } else {
            show_alert(window, "Push Failed", &detail);
        }
        return;
    }
    if result.pushed {
        let summary = result.commit_message.lines().next().unwrap_or("Synced").to_string();
        overlay.add_toast(adw::Toast::new(&format!("Synced — {summary}")));
    } else if result.committed {
        overlay.add_toast(adw::Toast::new("Committed locally — no remote push"));
    } else {
        overlay.add_toast(adw::Toast::new("Nothing to sync"));
    }
}

fn show_github_token_dialog(
    window: &adw::ApplicationWindow,
    overlay: &adw::ToastOverlay,
    root: PathBuf,
    current_config: Rc<RefCell<Config>>,
    message: &str,
) {
    let dialog = adw::Window::builder()
        .title("GitHub Login")
        .transient_for(window)
        .modal(true)
        .default_width(480)
        .default_height(300)
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);

    let label = gtk4::Label::new(Some(message));
    label.set_wrap(true);
    label.set_margin_top(12);
    label.set_margin_bottom(8);
    label.set_margin_start(16);
    label.set_margin_end(16);
    label.set_xalign(0.0);

    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some("ghp_xxxxxxxxxxxxxxxxxxxx"));
    entry.set_visibility(false);
    entry.set_margin_start(16);
    entry.set_margin_end(16);
    entry.set_margin_bottom(12);

    let hint = gtk4::Label::new(Some("Your token is stored locally and never shared."));
    hint.add_css_class("caption");
    hint.add_css_class("dim-label");
    hint.set_margin_start(16);
    hint.set_margin_end(16);
    hint.set_margin_bottom(16);
    hint.set_xalign(0.0);

    let save_btn = Button::with_label("Save & Sync");
    save_btn.add_css_class("suggested-action");
    save_btn.set_margin_start(16);
    save_btn.set_margin_end(16);
    save_btn.set_margin_bottom(16);

    let cancel_btn = Button::with_label("Cancel");
    cancel_btn.add_css_class("flat");
    header.pack_start(&cancel_btn);

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    vbox.append(&header);
    vbox.append(&label);
    vbox.append(&entry);
    vbox.append(&hint);
    vbox.append(&save_btn);
    dialog.set_content(Some(&vbox));

    let dialog_cancel = dialog.clone();
    cancel_btn.connect_clicked(move |_| dialog_cancel.close());

    let dialog_save = dialog.clone();
    let entry_save = entry.clone();
    let overlay_retry = overlay.clone();
    let window_retry = window.clone();
    save_btn.connect_clicked(move |btn| {
        let tok = entry_save.text().to_string();
        if tok.is_empty() { return; }

        let _ = crate::secret_store::save_github_token(&tok);

        btn.set_sensitive(false);
        dialog_save.close();

        // Auto-retry the sync with the new token — no need to click again.
        let root_thread = root.clone();
        let root_result = root.clone();
        let win2 = window_retry.clone();
        let ov2 = overlay_retry.clone();
        let cfg2 = current_config.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel::<git_sync::SyncResult>(1);
        std::thread::spawn(move || { tx.send(git_sync::sync(&root_thread, Some(&tok))).ok(); });
        let rx = Rc::new(rx);
        glib::timeout_add_local(Duration::from_millis(100), move || {
            use std::sync::mpsc::TryRecvError;
            match rx.try_recv() {
                Ok(result) => {
                    show_sync_result(&win2, &ov2, result, root_result.clone(), cfg2.clone());
                    glib::ControlFlow::Break
                }
                Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
    });

    dialog.present();
}

pub(super) fn show_backup_remote_dialog(window: &adw::ApplicationWindow, repo_path: &std::path::Path) {
    let dialog = adw::Window::builder()
        .title("Git Remotes")
        .transient_for(window)
        .modal(true)
        .default_width(520)
        .default_height(600)
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    let close_btn = Button::with_label("Close");
    close_btn.add_css_class("flat");
    header.pack_start(&close_btn);

    let page = adw::PreferencesPage::new();

    // ── Primary remote (origin / GitHub) ─────────────────────────────────────
    let origin_group = adw::PreferencesGroup::new();
    origin_group.set_title("Primary Remote");
    origin_group.set_description(Some(
        "Every sync pushes here first. Paste a GitHub HTTPS URL.",
    ));

    let origin_entry = adw::EntryRow::new();
    origin_entry.set_title("URL");
    if let Some(url) = git_sync::get_remote_url(repo_path, "origin") {
        origin_entry.set_text(&url);
    }

    let origin_status = Label::new(None);
    origin_status.set_xalign(0.0);
    origin_status.set_margin_top(4);
    origin_status.add_css_class("dim-label");

    let origin_apply = Button::with_label("Apply");
    origin_apply.add_css_class("suggested-action");
    origin_apply.set_halign(Align::End);
    {
        let entry = origin_entry.clone();
        let lbl = origin_status.clone();
        let root = repo_path.to_path_buf();
        origin_apply.connect_clicked(move |_| {
            let url = entry.text().to_string();
            if url.is_empty() {
                lbl.set_label("Enter a URL first.");
                return;
            }
            let _ = git_sync::remove_remote(&root, "origin");
            match git_sync::add_named_remote(&root, "origin", &url) {
                Ok(()) => {
                    lbl.set_label(&format!("✓ Origin set: {url}"));
                    lbl.remove_css_class("error");
                    lbl.add_css_class("success");
                }
                Err(e) => {
                    lbl.set_label(&format!("Error: {e}"));
                    lbl.remove_css_class("success");
                    lbl.add_css_class("error");
                }
            }
        });
    }

    let origin_suffix = GtkBox::new(Orientation::Vertical, 6);
    origin_suffix.set_margin_top(8);
    origin_suffix.set_margin_bottom(4);
    origin_suffix.append(&origin_status);
    origin_suffix.append(&origin_apply);

    origin_group.add(&origin_entry);
    origin_group.add(&{
        let row = adw::PreferencesRow::new();
        row.set_child(Some(&origin_suffix));
        row
    });
    page.add(&origin_group);

    // ── Additional remotes ────────────────────────────────────────────────────
    let current_group = adw::PreferencesGroup::new();
    current_group.set_title("Additional Remotes");

    let root_for_rebuild = repo_path.to_path_buf();
    // Track only the rows we explicitly added so we can safely remove them
    // without touching PreferencesGroup's internal header widgets.
    let tracked_rows: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::new(RefCell::new(Vec::new()));

    let rebuild_current = {
        let group = current_group.clone();
        let root = root_for_rebuild.clone();
        let tracked = tracked_rows.clone();
        move || {
            for row in tracked.borrow().iter() {
                group.remove(row);
            }
            tracked.borrow_mut().clear();

            let remotes = git_sync::list_backup_remotes(&root);
            if remotes.is_empty() {
                let row = adw::ActionRow::new();
                row.set_title("No backup remotes configured");
                row.add_css_class("dim-label");
                group.add(&row);
                tracked.borrow_mut().push(row);
            } else {
                for (name, url) in remotes {
                    let row = adw::ActionRow::new();
                    row.set_title(&name);
                    row.set_subtitle(&url);
                    let rm_btn = Button::from_icon_name("user-trash-symbolic");
                    rm_btn.add_css_class("flat");
                    rm_btn.add_css_class("destructive-action");
                    rm_btn.set_valign(Align::Center);
                    rm_btn.set_tooltip_text(Some("Remove this backup remote"));
                    let root2 = root.clone();
                    let tracked2 = tracked.clone();
                    let group2 = group.clone();
                    rm_btn.connect_clicked(move |_| {
                        let _ = git_sync::remove_remote(&root2, &name);
                        for r in tracked2.borrow().iter() { group2.remove(r); }
                        tracked2.borrow_mut().clear();
                        let remotes2 = git_sync::list_backup_remotes(&root2);
                        if remotes2.is_empty() {
                            let ph = adw::ActionRow::new();
                            ph.set_title("No backup remotes configured");
                            ph.add_css_class("dim-label");
                            group2.add(&ph);
                            tracked2.borrow_mut().push(ph);
                        } else {
                            for (n, u) in remotes2 {
                                let r = adw::ActionRow::new();
                                r.set_title(&n);
                                r.set_subtitle(&u);
                                group2.add(&r);
                                tracked2.borrow_mut().push(r);
                            }
                        }
                    });
                    row.add_suffix(&rm_btn);
                    group.add(&row);
                    tracked.borrow_mut().push(row);
                }
            }
        }
    };
    let rebuild_current = Rc::new(rebuild_current);
    rebuild_current();
    page.add(&current_group);

    // ── Add a new backup remote ───────────────────────────────────────────────
    let add_group = adw::PreferencesGroup::new();
    add_group.set_title("Add a Backup Remote");
    add_group.set_description(Some(
        "Sync pushes here in addition to the primary remote. Enter a name and a URL or local path.",
    ));

    let name_row = adw::EntryRow::new();
    name_row.set_title("Remote name");
    name_row.set_text("backup");

    let url_row = adw::EntryRow::new();
    url_row.set_title("URL or path");

    // Folder-picker button
    let pick_btn = Button::from_icon_name("document-open-symbolic");
    pick_btn.set_valign(Align::Center);
    pick_btn.add_css_class("flat");
    pick_btn.set_tooltip_text(Some("Browse for a local folder"));
    {
        let row_c = url_row.clone();
        let win_c = window.clone();
        pick_btn.connect_clicked(move |_| {
            let fd = gtk4::FileDialog::new();
            let row2 = row_c.clone();
            fd.select_folder(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        row2.set_text(path.to_str().unwrap_or(""));
                    }
                }
            });
        });
    }
    url_row.add_suffix(&pick_btn);

    let status_lbl = Label::new(None);
    status_lbl.set_xalign(0.0);
    status_lbl.set_margin_top(4);
    status_lbl.add_css_class("dim-label");

    let add_btn = Button::with_label("Add Remote");
    add_btn.add_css_class("suggested-action");
    add_btn.set_halign(Align::End);

    let btn_box = gtk4::Box::new(Orientation::Vertical, 6);
    btn_box.set_margin_top(8);
    btn_box.set_margin_bottom(4);
    btn_box.append(&status_lbl);
    btn_box.append(&add_btn);
    let btn_wrapper = adw::ActionRow::new();
    btn_wrapper.set_activatable(false);
    btn_wrapper.add_suffix(&btn_box);

    add_group.add(&name_row);
    add_group.add(&url_row);
    add_group.add(&btn_wrapper);
    page.add(&add_group);

    {
        let root_c = repo_path.to_path_buf();
        let lbl_c = status_lbl.clone();
        let name_r = name_row.clone();
        let url_r = url_row.clone();
        let rebuild_c = rebuild_current.clone();
        add_btn.connect_clicked(move |_| {
            let name = name_r.text().trim().to_string();
            let url  = url_r.text().trim().to_string();
            if name.is_empty() || url.is_empty() {
                lbl_c.set_text("Enter both a name and a URL.");
                return;
            }
            if name == "origin" {
                lbl_c.set_text("\"origin\" is reserved for the primary remote.");
                return;
            }
            match git_sync::add_named_remote(&root_c, &name, &url) {
                Ok(()) => {
                    lbl_c.set_text(&format!("✓ Added «{name}»"));
                    url_r.set_text("");
                    rebuild_c();
                }
                Err(e) => lbl_c.set_text(&format!("Error: {e}")),
            }
        });
    }

    // ── Disroot: privacy-respecting git hosting ───────────────────────────────
    let disroot_group = adw::PreferencesGroup::new();
    disroot_group.set_title("Disroot (git.disroot.org)");
    disroot_group.set_description(Some(
        "Disroot is a non-profit, privacy-respecting community hosting Gitea at \
         git.disroot.org. Free to use. Good for a second off-site copy of your work.",
    ));
    for (title, subtitle) in [
        ("1. Create account", "Register at https://disroot.org/en/register"),
        ("2. Create repository", "Log in to git.disroot.org → New repository"),
        ("3. Copy the clone URL", "Use HTTPS or SSH — shown on the repo page"),
        ("4. Add it below", "Name it \"disroot\", paste the URL above, click Add"),
    ] {
        let row = adw::ActionRow::new();
        row.set_title(title);
        row.set_subtitle(subtitle);
        disroot_group.add(&row);
    }
    // Quick-fill button for Disroot
    let disroot_fill_btn = Button::with_label("Set name to \"disroot\"");
    disroot_fill_btn.add_css_class("flat");
    disroot_fill_btn.set_halign(Align::Start);
    disroot_fill_btn.set_margin_top(4);
    {
        let nr = name_row.clone();
        disroot_fill_btn.connect_clicked(move |_| nr.set_text("disroot"));
    }
    disroot_group.add(&adw::ActionRow::new()); // spacer
    // Can't add a plain Button to PreferencesGroup, so wrap in ActionRow suffix
    let fill_row = adw::ActionRow::new();
    fill_row.set_title("Quick-fill name");
    fill_row.set_activatable(true);
    let nr2 = name_row.clone();
    fill_row.connect_activated(move |_| nr2.set_text("disroot"));
    fill_row.add_suffix(&Button::from_icon_name("go-next-symbolic"));
    // Re-use disroot_fill_btn logic via action row activation
    disroot_group.add(&fill_row);
    page.add(&disroot_group);

    // ── Examples ─────────────────────────────────────────────────────────────
    let hint_group = adw::PreferencesGroup::new();
    hint_group.set_title("Other URL Examples");
    for (name, hint) in [
        ("Local / NAS", "/mnt/backup/my-project  or  /run/media/you/usb/project"),
        ("pCloud / Nextcloud", "Mount the drive, then use the mount path above"),
        ("Codeberg", "git@codeberg.org:username/project.git"),
        ("GitLab", "git@gitlab.com:username/project.git"),
        ("Self-hosted Gitea", "git@my-server.example.com:username/project.git"),
    ] {
        let row = adw::ActionRow::new();
        row.set_title(name);
        row.set_subtitle(hint);
        hint_group.add(&row);
    }
    page.add(&hint_group);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    dialog.set_content(Some(&toolbar));

    let dlg_close = dialog.clone();
    close_btn.connect_clicked(move |_| dlg_close.close());

    dialog.present();
}


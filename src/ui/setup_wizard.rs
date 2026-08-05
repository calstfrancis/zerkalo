use std::path::Path;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    AlertDialog, Align, Box as GtkBox, Button, Image, Label, LinkButton, Orientation,
    ScrolledWindow, Separator, SignalListItemFactory, StringObject, Switch,
};
use libadwaita as adw;
use adw::prelude::*;

use super::github_signin;

pub struct SetupWizard {
    window: adw::Window,
}

// Preference order for the Default Fonts step's initial selection when
// nothing has been chosen yet — common, broadly-available names first, so a
// first-time user doesn't land on whatever happens to sort alphabetically
// first in their system font list (often an obscure font).
const SANS_FONT_PRIORITY: &[&str] = &["Noto Sans", "DejaVu Sans", "Cantarell", "Liberation Sans", "Arial", "Inter"];
const SERIF_FONT_PRIORITY: &[&str] = &["Noto Serif", "Liberation Serif", "DejaVu Serif", "Linux Libertine", "Times New Roman", "Georgia"];

impl SetupWizard {
    pub fn new(
        parent: &impl IsA<gtk4::Window>,
        work_dir: &Path,
        current_sans_font: &str,
        current_serif_font: &str,
        on_fonts_saved: impl Fn(String, String) + 'static,
    ) -> Self {
        let window = adw::Window::builder()
            .title("Setup & Onboarding")
            .transient_for(parent)
            .modal(true)
            .default_width(640)
            .default_height(620)
            .build();

        let header = adw::HeaderBar::new();
        header.add_css_class("fond-chrome");

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);

        let body = GtkBox::new(Orientation::Vertical, 20);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_top(16);
        body.set_margin_bottom(16);

        let intro = Label::new(Some(
            "Let's make sure Zerkalo is set up for academic writing and version control.",
        ));
        intro.set_wrap(true);
        intro.set_xalign(0.0);
        intro.add_css_class("dim-label");
        body.append(&intro);

        // Progress summary — filled in once every section below has reported
        // whether it's already configured, so there's an at-a-glance answer
        // to "what's left" instead of having to scan five sections by eye.
        let summary_lbl = Label::new(None);
        summary_lbl.set_wrap(true);
        summary_lbl.set_xalign(0.0);
        summary_lbl.add_css_class("caption");
        body.append(&summary_lbl);

        body.append(&subheading_label("Account & Sync"));

        // ── Section 1: Git identity ────────────────────────────────────────
        let (git_group, git_complete) = git_identity_group();
        body.append(&git_group);

        // ── Section 2: GitHub repository ──────────────────────────────────
        let (repo_group, repo_complete) = github_repo_group(&window, work_dir);
        body.append(&repo_group);

        // ── Section 3: Backup remote ───────────────────────────────────────
        let (backup_group, backup_complete) = backup_remote_group(work_dir);
        body.append(&backup_group);

        body.append(&subheading_label("Editor Preferences"));

        // ── Section 4: Default Fonts ────────────────────────────────────────
        let (fonts_group, fonts_complete) =
            default_fonts_group(current_sans_font, current_serif_font, on_fonts_saved);
        body.append(&fonts_group);

        // ── Section 5: Optional tools ──────────────────────────────────────
        let (tools_group, tools_complete, tool_rechecks) = optional_tools_group();
        body.append(&tools_group);

        // Fill in the progress summary now that every section has reported in.
        let sections: [(&str, bool); 5] = [
            ("Git Identity", git_complete),
            ("GitHub Repository", repo_complete),
            ("Local Backup", backup_complete),
            ("Default Fonts", fonts_complete),
            ("Tools", tools_complete),
        ];
        let done = sections.iter().filter(|(_, c)| *c).count();
        if done == sections.len() {
            summary_lbl.set_label(&format!("✓ All {done} sections set up."));
            summary_lbl.add_css_class("success");
        } else {
            let remaining: Vec<&str> = sections.iter().filter(|(_, c)| !*c).map(|(n, _)| *n).collect();
            summary_lbl.set_label(&format!(
                "{done} of {} sections set up — still to do: {}",
                sections.len(),
                remaining.join(", "),
            ));
            summary_lbl.add_css_class("dim-label");
        }

        // Auto-scroll to the first incomplete section so reopening the wizard
        // when only e.g. Tools is left doesn't require scrolling past four
        // already-configured sections every time.
        let first_incomplete: Option<adw::PreferencesGroup> = [
            (&git_group, git_complete),
            (&repo_group, repo_complete),
            (&backup_group, backup_complete),
            (&fonts_group, fonts_complete),
            (&tools_group, tools_complete),
        ]
        .into_iter()
        .find_map(|(g, c)| if !c { Some(g.clone()) } else { None });

        // Clamp caps the natural-width request so long content (e.g. the
        // unwrapped install-hint commands below) can't force the window open
        // 2-3x wider than intended — matches WelcomeWindow's use of Clamp for
        // the same reason. Without this, GTK sizes the window to fit the
        // widest child's natural width even though default_width is 640.
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(580);
        clamp.set_child(Some(&body));

        scroll.set_child(Some(&clamp));

        if let Some(target) = first_incomplete {
            let scroll_c = scroll.clone();
            window.connect_map(move |_| {
                let scroll_c2 = scroll_c.clone();
                let target2 = target.clone();
                // Deferred one tick: compute_bounds needs the widget tree to
                // have been allocated at least once, which hasn't happened
                // yet at the point "map" itself fires.
                glib::idle_add_local_once(move || {
                    if let Some(bounds) = target2.compute_bounds(&scroll_c2) {
                        scroll_c2.vadjustment().set_value(bounds.y() as f64);
                    }
                });
            });
        }

        // Re-check missing tools whenever the window regains focus, so
        // installing something in a terminal and alt-tabbing back updates
        // the status without an extra manual "Verify" click.
        {
            let rechecks = tool_rechecks;
            window.connect_is_active_notify(move |w| {
                if w.is_active() {
                    for f in &rechecks {
                        f();
                    }
                }
            });
        }

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.append(&scroll);
        outer.append(&Separator::new(Orientation::Horizontal));

        let footer = GtkBox::new(Orientation::Horizontal, 0);
        footer.set_margin_start(16);
        footer.set_margin_end(16);
        footer.set_margin_top(8);
        footer.set_margin_bottom(12);
        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        footer.append(&spacer);
        let done_btn = Button::with_label("Done");
        done_btn.add_css_class("suggested-action");
        done_btn.add_css_class("pill");
        footer.append(&done_btn);
        outer.append(&footer);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&outer));
        window.set_content(Some(&toolbar_view));

        let win_c = window.clone();
        done_btn.connect_clicked(move |_| win_c.close());

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }

    /// Returns true when the wizard should auto-show on startup.
    pub fn should_show(work_dir: &Path) -> bool {
        // Missing git identity
        if !has_git_identity() {
            return true;
        }
        // Work directory is not in a git repo or has no remote
        if !has_git_remote(work_dir) {
            return true;
        }
        false
    }
}

// ── Section builders ──────────────────────────────────────────────────────────

fn subheading_label(text: &str) -> Label {
    let lbl = Label::new(Some(text));
    lbl.set_xalign(0.0);
    lbl.add_css_class("heading");
    lbl.set_margin_top(4);
    lbl
}

/// Small icon (+ an "Optional" badge, for sections that aren't required)
/// shown at the end of a PreferencesGroup's title row — helps the section
/// list read at a glance instead of as five identical-looking text blocks.
fn group_header_suffix(icon_name: &str, optional: bool) -> GtkBox {
    let suffix = GtkBox::new(Orientation::Horizontal, 6);
    suffix.set_valign(Align::Center);
    if optional {
        let badge = Label::new(Some("Optional"));
        badge.add_css_class("dim-label");
        badge.add_css_class("caption");
        suffix.append(&badge);
    }
    let icon = Image::from_icon_name(icon_name);
    icon.add_css_class("dim-label");
    suffix.append(&icon);
    suffix
}

fn git_identity_group() -> (adw::PreferencesGroup, bool) {
    let group = adw::PreferencesGroup::new();
    group.set_title("Git Identity");
    group.set_description(Some(
        "Git records your name and email on every save. Set these once, globally.",
    ));
    group.set_header_suffix(Some(&group_header_suffix("avatar-default-symbolic", false)));

    let (current_name, current_email) = git_identity();
    let complete = !current_name.is_empty() && !current_email.is_empty();

    let name_row = adw::EntryRow::new();
    name_row.set_title("Name");
    name_row.set_text(&current_name);

    let email_row = adw::EntryRow::new();
    email_row.set_title("Email");
    email_row.set_text(&current_email);

    let status_lbl = Label::new(None);
    status_lbl.set_xalign(0.0);
    status_lbl.set_margin_top(4);
    status_lbl.set_wrap(true);
    if complete {
        status_lbl.set_label("✓ Git identity is set.");
        status_lbl.add_css_class("success");
    } else {
        status_lbl.set_label("Enter your name and email, then click Apply.");
        status_lbl.add_css_class("dim-label");
    }

    let apply_btn = Button::with_label("Apply");
    apply_btn.set_halign(Align::End);
    apply_btn.add_css_class("suggested-action");

    {
        let name_c = name_row.clone();
        let email_c = email_row.clone();
        let lbl_c = status_lbl.clone();
        apply_btn.connect_clicked(move |_| {
            let name = name_c.text().to_string();
            let email = email_c.text().to_string();
            match set_git_identity(&name, &email) {
                Ok(()) => {
                    lbl_c.set_label("✓ Git identity saved.");
                    lbl_c.remove_css_class("error");
                    lbl_c.add_css_class("success");
                }
                Err(e) => {
                    lbl_c.set_label(&format!("Error: {e}"));
                    lbl_c.remove_css_class("success");
                    lbl_c.add_css_class("error");
                }
            }
        });
    }

    group.add(&name_row);
    group.add(&email_row);

    // suffix container for the Apply button + status
    let suffix_box = GtkBox::new(Orientation::Vertical, 6);
    suffix_box.set_margin_top(8);
    suffix_box.set_margin_bottom(4);
    suffix_box.append(&status_lbl);
    suffix_box.append(&apply_btn);

    // Wrap in a plain ActionRow-style container via a custom box
    let wrapper = adw::ActionRow::new();
    wrapper.set_activatable(false);
    wrapper.add_suffix(&suffix_box);
    group.add(&wrapper);

    (group, complete)
}

fn github_repo_group(parent: &adw::Window, work_dir: &Path) -> (adw::PreferencesGroup, bool) {
    let work_dir = work_dir.to_path_buf();

    let group = adw::PreferencesGroup::new();
    group.set_title("GitHub Repository");
    group.set_description(Some(
        "Back up your work and collaborate by connecting to a GitHub repository.",
    ));
    group.set_header_suffix(Some(&group_header_suffix("network-server-symbolic", false)));

    let is_repo = git2::Repository::discover(&work_dir).is_ok();
    let remote_url = get_git_remote(&work_dir);
    let complete = remote_url.is_some();

    // ── Row: repo status ────────────────────────────────────────────────
    let repo_row = adw::ActionRow::new();
    repo_row.set_title("Local repository");
    if is_repo {
        repo_row.set_subtitle("✓ Git repository found in work directory");
    } else {
        repo_row.set_subtitle("No git repository — click to initialise one");
        let init_btn = Button::with_label("git init");
        init_btn.set_valign(Align::Center);
        init_btn.add_css_class("suggested-action");
        let work_dir_c = work_dir.clone();
        let row_c = repo_row.clone();
        init_btn.connect_clicked(move |btn| {
            match git2::Repository::init(&work_dir_c) {
                Ok(_) => {
                    row_c.set_subtitle("✓ Git repository initialised");
                    btn.set_sensitive(false);
                }
                Err(e) => {
                    row_c.set_subtitle(&format!("Error: {e}"));
                }
            }
        });
        repo_row.add_suffix(&init_btn);
    }
    group.add(&repo_row);

    // ── Declare all remaining widgets up front, so the sign-in handler ──
    // can reach into the "create repository" section below it.

    // GitHub account row
    let account_row = adw::ActionRow::new();
    account_row.set_title("GitHub Account");
    let has_token = crate::secret_store::load_github_token().is_some();
    account_row.set_subtitle(if has_token { "Connected" } else { "Not connected" });

    let signup_link = LinkButton::with_label(
        "https://github.com/signup",
        "Don't have an account? Create one (free) ↗",
    );
    signup_link.add_css_class("flat");
    signup_link.add_css_class("caption");

    let signin_btn = Button::with_label(if has_token { "Reconnect" } else { "Sign in with GitHub" });
    signin_btn.set_valign(Align::Center);
    signin_btn.add_css_class("suggested-action");

    // Create-a-repository section (the primary path)
    let create_row = adw::EntryRow::new();
    create_row.set_title("New repository name");
    let default_name = work_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("zerkalo-project")
        .to_string();
    create_row.set_text(&default_name);

    let private_switch = Switch::new();
    private_switch.set_active(true);
    private_switch.set_valign(Align::Center);
    let private_label = Label::new(Some("Private"));

    let create_status_lbl = Label::new(None);
    create_status_lbl.set_xalign(0.0);
    create_status_lbl.set_margin_top(4);
    create_status_lbl.set_wrap(true);
    create_status_lbl.add_css_class("dim-label");
    create_status_lbl.set_label(if has_token {
        "Creates a repository on your GitHub account and links it here."
    } else {
        "Sign in with GitHub above, then create a repository here."
    });

    let create_btn = Button::with_label("Create & Link");
    create_btn.set_halign(Align::End);
    create_btn.add_css_class("suggested-action");

    // Fallback: paste an existing repo's URL (demoted behind an expander)
    let remote_entry = adw::EntryRow::new();
    remote_entry.set_title("Remote URL (GitHub)");
    if let Some(ref url) = remote_url {
        remote_entry.set_text(url);
    }

    let status_lbl = Label::new(None);
    status_lbl.set_xalign(0.0);
    status_lbl.set_margin_top(4);
    status_lbl.set_wrap(true);
    match &remote_url {
        Some(url) => {
            status_lbl.set_label(&format!("✓ Remote: {url}"));
            status_lbl.add_css_class("success");
        }
        None => {
            status_lbl.set_label("Paste the URL of a repository you already created on GitHub.");
            status_lbl.add_css_class("dim-label");
        }
    }

    let apply_btn = Button::with_label("Apply");
    apply_btn.set_halign(Align::End);
    apply_btn.add_css_class("suggested-action");

    // ── Wire up the sign-in button now that create_row/create_status_lbl exist ──
    {
        let parent = parent.clone();
        let row_c = account_row.clone();
        let signin_btn_c = signin_btn.clone();
        let create_row_c = create_row.clone();
        let create_status_c = create_status_lbl.clone();
        signin_btn.connect_clicked(move |_| {
            let row_c2 = row_c.clone();
            let signin_btn_c2 = signin_btn_c.clone();
            let create_row_c2 = create_row_c.clone();
            let create_status_c2 = create_status_c.clone();
            github_signin::present(&parent, move |username| {
                row_c2.set_subtitle(&format!("Connected as {username}"));
                signin_btn_c2.set_label("Reconnect");
                create_status_c2.set_label("Connected! Pick a name below and click Create & Link to finish.");
                create_status_c2.remove_css_class("dim-label");
                create_status_c2.add_css_class("success");
                create_row_c2.grab_focus();
            });
        });
    }

    // ── Wire up "Create & Link" ─────────────────────────────────────────
    {
        let name_c = create_row.clone();
        let private_c = private_switch.clone();
        let status_c = create_status_lbl.clone();
        let wdir = work_dir.clone();
        let remote_entry_c = remote_entry.clone();
        let remote_status_c = status_lbl.clone();
        let win_for_create = parent.clone();
        create_btn.connect_clicked(move |btn| {
            let Some(token) = crate::secret_store::load_github_token() else {
                status_c.set_label("Sign in with GitHub first.");
                status_c.remove_css_class("success");
                status_c.add_css_class("error");
                return;
            };
            let name = name_c.text().trim().to_string();
            if name.is_empty() {
                status_c.set_label("Enter a repository name.");
                return;
            }
            let private = private_c.is_active();

            let go = {
                let btn = btn.clone();
                let status_c = status_c.clone();
                let wdir = wdir.clone();
                let remote_entry_c = remote_entry_c.clone();
                let remote_status_c = remote_status_c.clone();
                move || {
                    btn.set_sensitive(false);
                    status_c.remove_css_class("error");
                    status_c.set_label("Creating repository…");

                    let (tx, rx) = std::sync::mpsc::sync_channel(1);
                    std::thread::spawn(move || {
                        let _ = tx.send(crate::github_auth::create_repo(&token, &name, private));
                    });

                    let btn = btn.clone();
                    let wdir = wdir.clone();
                    let status_c = status_c.clone();
                    let remote_entry_c = remote_entry_c.clone();
                    let remote_status_c = remote_status_c.clone();
                    glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
                        match rx.try_recv() {
                            Ok(result) => {
                                match result {
                                    Ok(clone_url) => match set_git_remote(&wdir, &clone_url) {
                                        Ok(()) => {
                                            status_c.set_label(&format!("✓ Created and linked: {clone_url}"));
                                            status_c.add_css_class("success");
                                            remote_entry_c.set_text(&clone_url);
                                            remote_status_c.set_label(&format!("✓ Remote: {clone_url}"));
                                            remote_status_c.remove_css_class("dim-label");
                                            remote_status_c.add_css_class("success");
                                        }
                                        Err(e) => {
                                            status_c.set_label(&format!("Repository created, but linking failed: {e}"));
                                            status_c.add_css_class("error");
                                        }
                                    },
                                    Err(e) => {
                                        status_c.set_label(&format!("Error: {e}"));
                                        status_c.add_css_class("error");
                                    }
                                }
                                btn.set_sensitive(true);
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                status_c.set_label("Error: repository creation task ended unexpectedly.");
                                status_c.add_css_class("error");
                                btn.set_sensitive(true);
                                glib::ControlFlow::Break
                            }
                        }
                    });
                }
            };

            if has_git_remote(&wdir) {
                let existing = get_git_remote(&wdir).unwrap_or_default();
                let confirm = AlertDialog::builder()
                    .modal(true)
                    .message("Replace the existing remote?")
                    .detail(format!(
                        "This project is already linked to:\n{existing}\n\nCreating a new repository will replace this link. The old repository on GitHub is not deleted, but this project will stop tracking it."
                    ))
                    .buttons(["Cancel", "Create New Repository"])
                    .cancel_button(0)
                    .default_button(0)
                    .build();
                confirm.choose(Some(&win_for_create), None::<&gtk4::gio::Cancellable>, move |result| {
                    if result == Ok(1) {
                        go();
                    }
                });
                return;
            }
            go();
        });
    }

    // ── Wire up manual "paste URL" apply ────────────────────────────────
    {
        let entry_c = remote_entry.clone();
        let lbl_c = status_lbl.clone();
        let wdir = work_dir.clone();
        apply_btn.connect_clicked(move |_| {
            let url = entry_c.text().to_string();
            if url.is_empty() {
                lbl_c.set_label("Please enter a repository URL.");
                return;
            }
            match set_git_remote(&wdir, &url) {
                Ok(()) => {
                    lbl_c.set_label(&format!("✓ Remote set: {url}"));
                    lbl_c.remove_css_class("error");
                    lbl_c.add_css_class("success");
                }
                Err(e) => {
                    lbl_c.set_label(&format!("Error: {e}"));
                    lbl_c.remove_css_class("success");
                    lbl_c.add_css_class("error");
                }
            }
        });
    }

    // ── Layout, in guided order ──────────────────────────────────────────

    let account_suffix = GtkBox::new(Orientation::Vertical, 4);
    account_suffix.set_halign(Align::End);
    account_suffix.append(&signin_btn);
    account_suffix.append(&signup_link);
    account_row.add_suffix(&account_suffix);
    group.add(&account_row);

    group.add(&create_row);

    let private_box = GtkBox::new(Orientation::Horizontal, 6);
    private_box.set_halign(Align::End);
    private_box.append(&private_label);
    private_box.append(&private_switch);

    let create_suffix = GtkBox::new(Orientation::Vertical, 6);
    create_suffix.set_margin_top(8);
    create_suffix.set_margin_bottom(4);
    create_suffix.append(&create_status_lbl);
    let create_btn_row = GtkBox::new(Orientation::Horizontal, 8);
    create_btn_row.set_halign(Align::End);
    create_btn_row.append(&private_box);
    create_btn_row.append(&create_btn);
    create_suffix.append(&create_btn_row);

    let create_wrapper = adw::ActionRow::new();
    create_wrapper.set_activatable(false);
    create_wrapper.add_suffix(&create_suffix);
    group.add(&create_wrapper);

    let fallback_expander = adw::ExpanderRow::new();
    fallback_expander.set_title("Already have a repository?");
    fallback_expander.set_subtitle("Paste its URL instead of creating a new one");
    fallback_expander.add_row(&remote_entry);

    let fallback_suffix_box = GtkBox::new(Orientation::Vertical, 6);
    fallback_suffix_box.set_margin_top(8);
    fallback_suffix_box.set_margin_bottom(4);
    fallback_suffix_box.set_margin_start(12);
    fallback_suffix_box.set_margin_end(12);
    fallback_suffix_box.append(&status_lbl);
    let fallback_btn_row = GtkBox::new(Orientation::Horizontal, 8);
    fallback_btn_row.set_halign(Align::End);
    fallback_btn_row.append(&apply_btn);
    fallback_suffix_box.append(&fallback_btn_row);
    let fallback_wrapper = adw::ActionRow::new();
    fallback_wrapper.set_activatable(false);
    fallback_wrapper.add_suffix(&fallback_suffix_box);
    fallback_expander.add_row(&fallback_wrapper);

    group.add(&fallback_expander);

    (group, complete)
}

fn backup_remote_group(work_dir: &Path) -> (adw::PreferencesGroup, bool) {
    let work_dir = work_dir.to_path_buf();

    let group = adw::PreferencesGroup::new();
    group.set_title("Local Backup");
    group.set_description(Some(
        "On every sync, push a copy to a second location. Use a mounted drive \
         (pCloud, Nextcloud, USB), an external path, or any git URL.",
    ));
    group.set_header_suffix(Some(&group_header_suffix("drive-harddisk-symbolic", true)));

    let current_url = crate::git_sync::get_remote_url(&work_dir, "backup")
        .unwrap_or_default();
    let complete = !current_url.is_empty();

    let url_row = adw::EntryRow::new();
    url_row.set_title("Path or URL (optional)");
    url_row.set_text(&current_url);

    // Folder-picker button for local paths
    let pick_btn = Button::from_icon_name("document-open-symbolic");
    pick_btn.set_valign(Align::Center);
    pick_btn.add_css_class("flat");
    pick_btn.set_tooltip_text(Some("Browse for a folder"));
    {
        let row_c = url_row.clone();
        pick_btn.connect_clicked(move |_| {
            // The folder picker needs a parent window — we use the default
            // display's active window as a best-effort parent.
            let fd = gtk4::FileDialog::new();
            let row2 = row_c.clone();
            fd.select_folder(None::<&gtk4::Window>, None::<&gtk4::gio::Cancellable>, move |result| {
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
    status_lbl.set_wrap(true);
    if complete {
        status_lbl.set_label(&format!("✓ Backup: {current_url}"));
        status_lbl.add_css_class("success");
    } else {
        status_lbl.set_label("Optional — leave blank to skip.");
        status_lbl.add_css_class("dim-label");
    }

    let apply_btn = Button::with_label("Save");
    apply_btn.set_halign(Align::End);
    apply_btn.add_css_class("suggested-action");

    {
        let entry_c = url_row.clone();
        let lbl_c = status_lbl.clone();
        let wdir = work_dir.clone();
        apply_btn.connect_clicked(move |_| {
            let target = entry_c.text().trim().to_string();
            if target.is_empty() {
                lbl_c.set_label("No backup location set.");
                return;
            }
            match crate::git_sync::add_backup_remote(&wdir, &target) {
                Ok(()) => {
                    lbl_c.set_label(&format!("✓ Backup saved: {target}"));
                    lbl_c.remove_css_class("dim-label");
                    lbl_c.remove_css_class("error");
                    lbl_c.add_css_class("success");
                }
                Err(e) => {
                    lbl_c.set_label(&format!("Error: {e}"));
                    lbl_c.remove_css_class("success");
                    lbl_c.add_css_class("error");
                }
            }
        });
    }

    group.add(&url_row);

    let suffix_box = GtkBox::new(Orientation::Vertical, 6);
    suffix_box.set_margin_top(8);
    suffix_box.set_margin_bottom(4);
    suffix_box.append(&status_lbl);
    suffix_box.append(&apply_btn);
    let wrapper = adw::ActionRow::new();
    wrapper.set_activatable(false);
    wrapper.add_suffix(&suffix_box);

    group.add(&wrapper);

    (group, complete)
}

/// Picks the best initial ComboRow selection: the user's already-chosen font
/// if it's in the list, else the first name from `priority` that's actually
/// available, else index 0 as a last resort.
fn best_font_index(fonts: &[String], current: &str, priority: &[&str]) -> u32 {
    if let Some(i) = fonts.iter().position(|f| f == current) {
        return i as u32;
    }
    for name in priority {
        if let Some(i) = fonts.iter().position(|f| f == name) {
            return i as u32;
        }
    }
    0
}

/// A list-item factory that renders each font name set in its own font, so
/// the Sans/Serif dropdowns preview the choice instead of listing plain text.
fn font_preview_factory() -> SignalListItemFactory {
    let factory = SignalListItemFactory::new();
    factory.connect_setup(move |_, obj| {
        let Some(item) = obj.downcast_ref::<gtk4::ListItem>() else { return };
        let label = Label::new(None);
        label.set_xalign(0.0);
        label.set_margin_start(6);
        label.set_margin_end(6);
        label.set_margin_top(4);
        label.set_margin_bottom(4);
        item.set_child(Some(&label));
    });
    factory.connect_bind(move |_, obj| {
        let Some(item) = obj.downcast_ref::<gtk4::ListItem>() else { return };
        let Some(label) = item.child().and_then(|w| w.downcast::<Label>().ok()) else { return };
        let Some(text) = item.item().and_then(|o| o.downcast::<StringObject>().ok()) else { return };
        let name = text.string().to_string();
        label.set_text(&name);
        let mut desc = gtk4::pango::FontDescription::new();
        desc.set_family(&name);
        let attrs = gtk4::pango::AttrList::new();
        attrs.insert(gtk4::pango::AttrFontDesc::new(&desc));
        label.set_attributes(Some(&attrs));
    });
    factory
}

fn default_fonts_group(
    current_sans: &str,
    current_serif: &str,
    on_save: impl Fn(String, String) + 'static,
) -> (adw::PreferencesGroup, bool) {
    let group = adw::PreferencesGroup::new();
    group.set_title("Default Fonts");
    group.set_description(Some(
        "New documents and template previews use these until you pick a different font \
         per-document. Once set, Font Management won't let you disable either one without \
         choosing a replacement here first.",
    ));
    group.set_header_suffix(Some(&group_header_suffix("font-x-generic-symbolic", true)));

    let complete = !current_sans.is_empty() && !current_serif.is_empty();

    let fonts = super::font_manager::FontManager::enabled_fonts();
    let font_labels: Vec<&str> = fonts.iter().map(|s| s.as_str()).collect();
    let font_model = gtk4::StringList::new(&font_labels);
    let preview_factory = font_preview_factory();

    let sans_row = adw::ComboRow::new();
    sans_row.set_title("Sans-serif");
    sans_row.set_model(Some(&font_model));
    sans_row.set_factory(Some(&preview_factory));
    sans_row.set_list_factory(Some(&preview_factory));
    sans_row.set_selected(best_font_index(&fonts, current_sans, SANS_FONT_PRIORITY));

    let serif_row = adw::ComboRow::new();
    serif_row.set_title("Serif");
    serif_row.set_model(Some(&font_model));
    serif_row.set_factory(Some(&preview_factory));
    serif_row.set_list_factory(Some(&preview_factory));
    serif_row.set_selected(best_font_index(&fonts, current_serif, SERIF_FONT_PRIORITY));

    let status_lbl = Label::new(None);
    status_lbl.set_xalign(0.0);
    status_lbl.set_margin_top(4);
    status_lbl.set_wrap(true);
    if complete {
        status_lbl.set_label(&format!("✓ Sans: {current_sans} · Serif: {current_serif}"));
        status_lbl.add_css_class("success");
    } else {
        status_lbl.set_label("Not set yet — new documents fall back to a built-in font.");
        status_lbl.add_css_class("dim-label");
    }

    let apply_btn = Button::with_label("Save");
    apply_btn.set_halign(Align::End);
    apply_btn.add_css_class("suggested-action");

    {
        let fonts_c = fonts.clone();
        let sans_c = sans_row.clone();
        let serif_c = serif_row.clone();
        let lbl_c = status_lbl.clone();
        apply_btn.connect_clicked(move |_| {
            let sans = fonts_c.get(sans_c.selected() as usize).cloned().unwrap_or_default();
            let serif = fonts_c.get(serif_c.selected() as usize).cloned().unwrap_or_default();
            on_save(sans.clone(), serif.clone());
            lbl_c.set_label(&format!("✓ Sans: {sans} · Serif: {serif}"));
            lbl_c.remove_css_class("dim-label");
            lbl_c.add_css_class("success");
        });
    }

    group.add(&sans_row);
    group.add(&serif_row);

    let suffix_box = GtkBox::new(Orientation::Vertical, 6);
    suffix_box.set_margin_top(8);
    suffix_box.set_margin_bottom(4);
    suffix_box.append(&status_lbl);
    suffix_box.append(&apply_btn);
    let wrapper = adw::ActionRow::new();
    wrapper.set_activatable(false);
    wrapper.add_suffix(&suffix_box);
    group.add(&wrapper);

    (group, complete)
}

fn optional_tools_group() -> (adw::PreferencesGroup, bool, Vec<Rc<dyn Fn()>>) {
    let group = adw::PreferencesGroup::new();
    group.set_title("Tools");
    group.set_description(Some(
        "tinymist and pandoc are bundled with Zerkalo. git is required for sync.",
    ));
    group.set_header_suffix(Some(&group_header_suffix("applications-utilities-symbolic", false)));

    let distro = detect_distro();
    let mut rechecks: Vec<Rc<dyn Fn()>> = Vec::new();
    let mut required_ok = true;

    let (row, ok, recheck) = tool_row("tinymist", "tinymist", "LSP completions — bundled", &distro, ToolKind::Bundled, false);
    group.add(&row);
    if let Some(f) = recheck { rechecks.push(f); }
    let _ = ok;

    let (row, ok, recheck) = tool_row("pandoc", "pandoc", "Export/import — bundled", &distro, ToolKind::Bundled, false);
    group.add(&row);
    if let Some(f) = recheck { rechecks.push(f); }
    let _ = ok;

    let (row, ok, recheck) = tool_row("git", "git", "Version control — required for sync", &distro, ToolKind::Package {
        apt: "git", dnf: "git", pacman: "git", zypper: "git",
    }, true);
    group.add(&row);
    if let Some(f) = recheck { rechecks.push(f); }
    required_ok = required_ok && ok;

    let (row, ok, recheck) = tool_row("hunspell", "hunspell", "Spellcheck — optional", &distro, ToolKind::Package {
        apt: "hunspell", dnf: "hunspell", pacman: "hunspell", zypper: "hunspell",
    }, false);
    group.add(&row);
    if let Some(f) = recheck { rechecks.push(f); }
    let _ = ok;

    let (row, ok, recheck) = tool_row(
        "Skrizhal",
        "",
        "Optional companion app for CV Mode — a structured YAML database of jobs, degrees, and awards you can reuse across résumés",
        &distro,
        ToolKind::Flatpak { app_id: "io.github.calstfrancis.Skrizhal" },
        false,
    );
    group.add(&row);
    if let Some(f) = recheck { rechecks.push(f); }
    let _ = ok;

    (group, required_ok, rechecks)
}

// ── Distro detection ──────────────────────────────────────────────────────────

#[derive(Clone)]
enum Distro {
    Debian,
    Fedora,
    Arch,
    OpenSUSE,
    Unknown,
}

fn detect_distro() -> Distro {
    let content = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let id = content
        .lines()
        .find_map(|l| l.strip_prefix("ID="))
        .unwrap_or("")
        .trim_matches('"')
        .to_lowercase();
    let id_like = content
        .lines()
        .find_map(|l| l.strip_prefix("ID_LIKE="))
        .unwrap_or("")
        .trim_matches('"')
        .to_lowercase();

    if id.contains("ubuntu") || id.contains("debian") || id.contains("mint")
        || id_like.contains("ubuntu") || id_like.contains("debian")
    {
        Distro::Debian
    } else if id.contains("fedora") || id.contains("rhel") || id.contains("centos")
        || id_like.contains("fedora") || id_like.contains("rhel")
    {
        Distro::Fedora
    } else if id.contains("arch") || id.contains("manjaro") || id.contains("endeavour")
        || id_like.contains("arch")
    {
        Distro::Arch
    } else if id.contains("opensuse") || id.contains("suse") || id_like.contains("suse") {
        Distro::OpenSUSE
    } else {
        Distro::Unknown
    }
}

enum ToolKind<'a> {
    Package { apt: &'a str, dnf: &'a str, pacman: &'a str, zypper: &'a str },
    #[allow(dead_code)]
    Cargo { crate_name: &'a str },
    Flatpak { app_id: &'a str },
    Bundled,
}

fn install_hint(distro: &Distro, kind: &ToolKind) -> String {
    match kind {
        ToolKind::Package { apt, dnf, pacman, zypper } => match distro {
            Distro::Debian  => format!("sudo apt install {apt}"),
            Distro::Fedora  => format!("sudo dnf install {dnf}"),
            Distro::Arch    => format!("sudo pacman -S {pacman}"),
            Distro::OpenSUSE => format!("sudo zypper in {zypper}"),
            Distro::Unknown => format!("apt: sudo apt install {apt}  |  dnf: sudo dnf install {dnf}  |  pacman: sudo pacman -S {pacman}"),
        },
        ToolKind::Cargo { crate_name } => {
            if check_command("cargo") {
                format!("cargo install {crate_name}")
            } else {
                format!("Install Rust first (rustup.rs), then: cargo install {crate_name}")
            }
        }
        ToolKind::Flatpak { app_id } => format!(
            "flatpak remote-add --user calstfrancis https://calstfrancis.github.io/flatpak/calstfrancis.flatpakrepo\nflatpak install calstfrancis {app_id}"
        ),
        ToolKind::Bundled => String::new(),
    }
}

fn flatpak_installed(app_id: &str) -> bool {
    crate::git_sync::host_command("flatpak")
        .args(["info", app_id])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Builds one tool row. Returns the row, whether it's currently OK, and — for
/// a missing, non-bundled tool — a re-check closure the caller can invoke
/// later (e.g. when the window regains focus) to refresh its status without
/// requiring the user to click "Verify" again.
fn tool_row(
    name: &str,
    cmd: &str,
    purpose: &str,
    distro: &Distro,
    kind: ToolKind,
    required: bool,
) -> (adw::ActionRow, bool, Option<Rc<dyn Fn()>>) {
    let is_bundled = matches!(kind, ToolKind::Bundled);
    let flatpak_app_id: Option<String> = match &kind {
        ToolKind::Flatpak { app_id } => Some(app_id.to_string()),
        _ => None,
    };
    let hint = install_hint(distro, &kind);
    let ok = if let Some(app_id) = &flatpak_app_id {
        flatpak_installed(app_id)
    } else {
        is_bundled || check_command(cmd)
    };
    let cmd = cmd.to_string();

    let row = adw::ActionRow::new();
    row.set_title(name);
    row.set_subtitle(purpose);

    if ok {
        let icon = Label::new(Some("✓"));
        icon.add_css_class("success");
        row.add_suffix(&icon);
        (row, true, None)
    } else {
        if required {
            let badge = Label::new(Some("Required"));
            badge.add_css_class("error");
            badge.add_css_class("caption");
            badge.set_valign(Align::Center);
            row.add_suffix(&badge);
        }

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.set_valign(Align::Center);

        let hint_box = GtkBox::new(Orientation::Vertical, 4);
        hint_box.set_margin_top(4);
        hint_box.set_margin_bottom(4);

        let hint_row = GtkBox::new(Orientation::Horizontal, 4);
        let hint_lbl = Label::new(Some(&hint));
        hint_lbl.set_xalign(0.0);
        hint_lbl.set_selectable(true);
        hint_lbl.add_css_class("monospace");
        hint_lbl.add_css_class("caption");
        // Some hints are a single long line (the "Unknown" distro fallback
        // joins three package-manager commands; the flatpak hint embeds a
        // full URL) — without wrapping, this one label's minimum width forced
        // the whole window open regardless of the Clamp above.
        hint_lbl.set_wrap(true);
        hint_lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        hint_lbl.set_max_width_chars(40);
        hint_lbl.set_hexpand(true);
        hint_row.append(&hint_lbl);

        let copy_btn = Button::from_icon_name("edit-copy-symbolic");
        copy_btn.add_css_class("flat");
        copy_btn.set_valign(Align::Start);
        copy_btn.set_tooltip_text(Some("Copy command"));
        {
            let hint_c = hint.clone();
            copy_btn.connect_clicked(move |btn| {
                if let Some(display) = gtk4::gdk::Display::default() {
                    display.clipboard().set_text(&hint_c);
                }
                btn.set_icon_name("object-select-symbolic");
                let btn2 = btn.clone();
                glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
                    btn2.set_icon_name("edit-copy-symbolic");
                });
            });
        }
        hint_row.append(&copy_btn);
        hint_box.append(&hint_row);

        let revealer = gtk4::Revealer::new();
        revealer.set_reveal_child(false);
        revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
        revealer.set_child(Some(&hint_box));

        // Status indicator (✗ → ✓ after verify). Required-and-missing reads
        // as an error; optional-and-missing is milder so a missing spellcheck
        // dictionary doesn't look as alarming as a missing git binary.
        let status_lbl = Label::new(Some("✗"));
        status_lbl.add_css_class(if required { "error" } else { "warning" });

        let btn_box = GtkBox::new(Orientation::Horizontal, 4);
        btn_box.set_valign(Align::Center);

        let toggle_btn = Button::with_label("How to install");
        toggle_btn.add_css_class("flat");
        toggle_btn.add_css_class("caption");
        {
            let rev = revealer.clone();
            toggle_btn.connect_clicked(move |_| {
                rev.set_reveal_child(!rev.reveals_child());
            });
        }

        let verify_btn = Button::with_label("Verify");
        verify_btn.add_css_class("flat");
        verify_btn.add_css_class("caption");

        let do_verify: Rc<dyn Fn()> = {
            let cmd = cmd.clone();
            let flatpak_app_id = flatpak_app_id.clone();
            let status = status_lbl.clone();
            let rev = revealer.clone();
            Rc::new(move || {
                let found = if let Some(app_id) = &flatpak_app_id {
                    flatpak_installed(app_id)
                } else {
                    check_command(&cmd)
                };
                if found {
                    status.set_label("✓");
                    status.remove_css_class("error");
                    status.remove_css_class("warning");
                    status.add_css_class("success");
                    rev.set_reveal_child(false);
                } else {
                    status.set_label("✗ not found yet");
                }
            })
        };
        {
            let do_verify_c = do_verify.clone();
            verify_btn.connect_clicked(move |_| do_verify_c());
        }

        btn_box.append(&toggle_btn);
        btn_box.append(&verify_btn);
        outer.append(&btn_box);
        outer.append(&revealer);

        row.add_suffix(&status_lbl);
        row.add_suffix(&outer);

        (row, false, Some(do_verify))
    }
}

// ── Git helpers ───────────────────────────────────────────────────────────────

fn has_git_identity() -> bool {
    let (name, email) = git_identity();
    !name.is_empty() && !email.is_empty()
}

fn git_identity() -> (String, String) {
    let cfg = git2::Config::open_default().ok();
    let name = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.name").ok())
        .unwrap_or_default();
    let email = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.email").ok())
        .unwrap_or_default();
    (name, email)
}

fn set_git_identity(name: &str, email: &str) -> Result<(), String> {
    let mut cfg = git2::Config::open_default().map_err(|e| e.message().to_string())?;
    cfg.set_str("user.name", name).map_err(|e| e.message().to_string())?;
    cfg.set_str("user.email", email).map_err(|e| e.message().to_string())?;
    Ok(())
}

fn has_git_remote(work_dir: &Path) -> bool {
    get_git_remote(work_dir).is_some()
}

fn get_git_remote(work_dir: &Path) -> Option<String> {
    let repo = git2::Repository::discover(work_dir).ok()?;
    let remotes = repo.remotes().ok()?;
    let name = remotes.get(0)?;
    let remote = repo.find_remote(name).ok()?;
    remote.url().map(|s| s.to_string())
}

fn set_git_remote(work_dir: &Path, url: &str) -> Result<(), String> {
    let repo = git2::Repository::discover(work_dir)
        .map_err(|e| e.message().to_string())?;
    // Remove existing origin if present, then add fresh
    let _ = repo.remote_delete("origin");
    repo.remote("origin", url).map_err(|e| e.message().to_string())?;
    Ok(())
}

fn check_command(cmd: &str) -> bool {
    // tinymist may be bundled at a fixed path inside or outside the flatpak
    if cmd == "tinymist" {
        let bundled = ["/app/lib/zerkalo/tinymist", "/usr/lib/zerkalo/tinymist"];
        if bundled.iter().any(|p| std::path::Path::new(p).exists()) {
            return true;
        }
    }
    crate::git_sync::host_command(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

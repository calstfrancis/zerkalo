use std::path::Path;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, LinkButton, Orientation, ScrolledWindow, Separator};
use libadwaita as adw;
use adw::prelude::*;

pub struct SetupWizard {
    window: adw::Window,
}

impl SetupWizard {
    pub fn new(parent: &impl IsA<gtk4::Window>, work_dir: &Path) -> Self {
        let window = adw::Window::builder()
            .title("Setup & Onboarding")
            .transient_for(parent)
            .modal(true)
            .default_width(520)
            .default_height(640)
            .build();

        let header = adw::HeaderBar::new();

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

        // ── Section 1: Git identity ────────────────────────────────────────
        body.append(&git_identity_group());

        // ── Section 2: GitHub repository ──────────────────────────────────
        body.append(&github_repo_group(work_dir));

        // ── Section 3: Backup remote ───────────────────────────────────────
        body.append(&backup_remote_group(work_dir));

        // ── Section 4: Optional tools ──────────────────────────────────────
        body.append(&optional_tools_group());

        scroll.set_child(Some(&body));

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

fn git_identity_group() -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title("Git Identity");
    group.set_description(Some(
        "Git records your name and email on every save. Set these once, globally.",
    ));

    let (current_name, current_email) = git_identity();

    let name_row = adw::EntryRow::new();
    name_row.set_title("Name");
    name_row.set_text(&current_name);

    let email_row = adw::EntryRow::new();
    email_row.set_title("Email");
    email_row.set_text(&current_email);

    let status_lbl = Label::new(None);
    status_lbl.set_xalign(0.0);
    status_lbl.set_margin_top(4);
    if !current_name.is_empty() && !current_email.is_empty() {
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

    group
}

fn github_repo_group(work_dir: &Path) -> adw::PreferencesGroup {
    let work_dir = work_dir.to_path_buf();

    let group = adw::PreferencesGroup::new();
    group.set_title("GitHub Repository");
    group.set_description(Some(
        "Back up your work and collaborate by connecting to a GitHub repository.",
    ));

    let is_repo = git2::Repository::discover(&work_dir).is_ok();
    let remote_url = get_git_remote(&work_dir);

    // Row 1: repo status
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

    // Row 2: remote URL entry
    let remote_entry = adw::EntryRow::new();
    remote_entry.set_title("Remote URL (GitHub)");
    if let Some(ref url) = remote_url {
        remote_entry.set_text(url);
    } else {
        remote_entry.set_text("");
    }

    let status_lbl = Label::new(None);
    status_lbl.set_xalign(0.0);
    status_lbl.set_margin_top(4);
    match &remote_url {
        Some(url) => {
            status_lbl.set_label(&format!("✓ Remote: {url}"));
            status_lbl.add_css_class("success");
        }
        None => {
            status_lbl.set_label("No remote set. Paste the URL from GitHub, then click Apply.");
            status_lbl.add_css_class("dim-label");
        }
    }

    let apply_btn = Button::with_label("Apply");
    apply_btn.set_halign(Align::End);
    apply_btn.add_css_class("suggested-action");

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

    group.add(&remote_entry);

    let suffix_box = GtkBox::new(Orientation::Vertical, 6);
    suffix_box.set_margin_top(8);
    suffix_box.set_margin_bottom(4);
    suffix_box.append(&status_lbl);

    let btn_row = GtkBox::new(Orientation::Horizontal, 8);
    btn_row.set_halign(Align::End);
    let new_repo_link = LinkButton::with_label(
        "https://github.com/new",
        "Create repo on GitHub ↗",
    );
    new_repo_link.add_css_class("flat");
    btn_row.append(&new_repo_link);
    btn_row.append(&apply_btn);
    suffix_box.append(&btn_row);

    let wrapper = adw::ActionRow::new();
    wrapper.set_activatable(false);
    wrapper.add_suffix(&suffix_box);
    group.add(&wrapper);

    group
}

fn backup_remote_group(work_dir: &Path) -> adw::PreferencesGroup {
    let work_dir = work_dir.to_path_buf();

    let group = adw::PreferencesGroup::new();
    group.set_title("Backup Remote");
    group.set_description(Some(
        "Push to a second host on every sync for redundancy. \
         GitLab, Codeberg, or a self-hosted server all work.",
    ));

    let current_url = crate::git_sync::get_remote_url(&work_dir, "backup")
        .unwrap_or_default();

    let url_row = adw::EntryRow::new();
    url_row.set_title("Backup URL (optional)");
    url_row.set_text(&current_url);

    let status_lbl = Label::new(None);
    status_lbl.set_xalign(0.0);
    status_lbl.set_margin_top(4);
    if !current_url.is_empty() {
        status_lbl.set_label(&format!("✓ Backup remote: {current_url}"));
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
            let url = entry_c.text().trim().to_string();
            if url.is_empty() {
                lbl_c.set_label("No backup remote set.");
                return;
            }
            match crate::git_sync::add_backup_remote(&wdir, &url) {
                Ok(()) => {
                    lbl_c.set_label(&format!("✓ Backup remote saved: {url}"));
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

    let hint_row = adw::ActionRow::new();
    hint_row.set_activatable(false);
    let links_box = GtkBox::new(Orientation::Horizontal, 8);
    links_box.set_margin_top(4);
    links_box.set_margin_bottom(4);
    let gitlab_link = LinkButton::with_label("https://gitlab.com", "GitLab ↗");
    gitlab_link.add_css_class("flat");
    gitlab_link.add_css_class("caption");
    let codeberg_link = LinkButton::with_label("https://codeberg.org", "Codeberg ↗");
    codeberg_link.add_css_class("flat");
    codeberg_link.add_css_class("caption");
    links_box.append(&gitlab_link);
    links_box.append(&Separator::new(Orientation::Vertical));
    links_box.append(&codeberg_link);
    hint_row.add_suffix(&links_box);

    let suffix_box = GtkBox::new(Orientation::Vertical, 6);
    suffix_box.set_margin_top(8);
    suffix_box.set_margin_bottom(4);
    suffix_box.append(&status_lbl);
    suffix_box.append(&apply_btn);
    let wrapper = adw::ActionRow::new();
    wrapper.set_activatable(false);
    wrapper.add_suffix(&suffix_box);

    group.add(&hint_row);
    group.add(&wrapper);

    group
}

fn optional_tools_group() -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title("Optional Tools");
    group.set_description(Some(
        "These tools extend Zerkalo's capabilities. Install them via your package manager.",
    ));

    for (name, cmd, purpose, pkg_hint) in [
        (
            "tinymist",
            "tinymist",
            "Autocomplete & LSP hints in the editor",
            "curl -fsSL https://github.com/Myriad-Dreamin/tinymist/releases/latest/download/tinymist-installer.sh | sh",
        ),
        (
            "pandoc",
            "pandoc",
            "Export to DOCX and LaTeX import",
            "zypper in pandoc  (openSUSE)  /  apt install pandoc",
        ),
        (
            "hunspell",
            "hunspell",
            "Spellcheck in the editor",
            "zypper in hunspell  (openSUSE)  /  apt install hunspell",
        ),
    ] {
        group.add(&tool_row(name, cmd, purpose, pkg_hint));
    }

    group
}

fn tool_row(
    name: &str,
    cmd: &str,
    purpose: &str,
    pkg_hint: &str,
) -> adw::ActionRow {
    let ok = check_command(cmd);
    let row = adw::ActionRow::new();
    row.set_title(name);
    row.set_subtitle(purpose);

    if ok {
        let icon = Label::new(Some("✓"));
        icon.add_css_class("success");
        row.add_suffix(&icon);
    } else {
        // Revealer with install hint
        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.set_valign(Align::Center);

        let hint_box = GtkBox::new(Orientation::Vertical, 4);
        hint_box.set_margin_top(4);
        hint_box.set_margin_bottom(4);
        let hint_lbl = Label::new(Some(pkg_hint));
        hint_lbl.set_xalign(0.0);
        hint_lbl.set_selectable(true);
        hint_lbl.add_css_class("monospace");
        hint_lbl.add_css_class("caption");
        hint_box.append(&hint_lbl);

        let revealer = gtk4::Revealer::new();
        revealer.set_reveal_child(false);
        revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
        revealer.set_child(Some(&hint_box));

        let toggle_btn = Button::with_label("How to install");
        toggle_btn.add_css_class("flat");
        toggle_btn.add_css_class("caption");
        toggle_btn.set_valign(Align::Center);
        {
            let rev = revealer.clone();
            toggle_btn.connect_clicked(move |_| {
                rev.set_reveal_child(!rev.reveals_child());
            });
        }

        outer.append(&toggle_btn);
        outer.append(&revealer);

        let missing = Label::new(Some("✗"));
        missing.add_css_class("error");

        row.add_suffix(&missing);
        row.add_suffix(&outer);
    }

    row
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
    std::process::Command::new(cmd)
        .arg("--version")
        .output()
        .is_ok()
}

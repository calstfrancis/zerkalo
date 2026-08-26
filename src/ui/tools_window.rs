//! The Tools list — what Zerkalo ships with and what it can optionally use.
//!
//! This used to be the last step of setup, which put a table of `sudo`
//! commands in front of someone whose only goal was to start writing. Nothing
//! here is required any more — git, tinymist and pandoc are all bundled — so
//! it is a diagnostic you open when something looks wrong, not a gate.

use std::rc::Rc;

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, ScrolledWindow};
use libadwaita as adw;

pub struct ToolsWindow {
    window: adw::Window,
}

impl ToolsWindow {
    pub fn new(parent: &impl IsA<gtk4::Window>) -> Self {
        let window = adw::Window::builder()
            .title("Tools")
            .transient_for(parent)
            .modal(true)
            .default_width(560)
            .default_height(480)
            .build();

        let header = adw::HeaderBar::new();
        header.add_css_class("fond-chrome");

        let body = GtkBox::new(Orientation::Vertical, 20);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_top(16);
        body.set_margin_bottom(16);

        let (group, _ok, rechecks) = tools_group();
        body.append(&group);

        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(580);
        clamp.set_child(Some(&body));

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));

        // Re-check whenever the window regains focus, so installing something
        // in a terminal and coming back updates the list without a click.
        window.connect_is_active_notify(move |w| {
            if w.is_active() {
                for f in &rechecks {
                    f();
                }
            }
        });

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&scroll));
        window.set_content(Some(&toolbar_view));

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

pub fn tools_group() -> (adw::PreferencesGroup, bool, Vec<Rc<dyn Fn()>>) {
    let group = adw::PreferencesGroup::new();
    group.set_title("Tools");
    group.set_description(Some(
        "git and tinymist are bundled with Zerkalo — nothing to install there. \
         The rest, including pandoc, are optional.",
    ));

    let distro = detect_distro();
    let mut rechecks: Vec<Rc<dyn Fn()>> = Vec::new();
    let mut all_ok = true;

    // git is bundled in the flatpak; outside it, the system's own git is used.
    let git_kind = if crate::git_sync::bundled_git().is_some() {
        ToolKind::Bundled
    } else {
        ToolKind::Package {
            apt: "git",
            dnf: "git",
            pacman: "git",
            zypper: "git",
        }
    };
    let git_purpose = if crate::git_sync::bundled_git().is_some() {
        "Version history and sync — bundled"
    } else {
        "Version history and sync"
    };

    for (name, cmd, purpose, kind, required) in [
        ("git", "git", git_purpose, git_kind, true),
        ("tinymist", "tinymist", "Completions and diagnostics — bundled", ToolKind::Bundled, false),
        // Unlike git and tinymist, pandoc is not actually built into the
        // flatpak (nothing in packaging/*.yml fetches or installs it) — it
        // used to be listed as `ToolKind::Bundled` here, which always reports
        // `ok = true` regardless of whether the command is found, so this row
        // silently showed a green checkmark even when export_dialog.rs's own
        // independent pandoc check had just disabled every non-PDF format.
        ("pandoc", "pandoc", "LaTeX, HTML, EPUB and RTF import; DOCX, ODT, LaTeX and EPUB export", ToolKind::Package {
            apt: "pandoc", dnf: "pandoc", pacman: "pandoc", zypper: "pandoc",
        }, false),
        // Checked as dictionary files, not a `hunspell` command — see
        // check_command's "hunspell" branch. `cmd` stays "hunspell" only as
        // that branch's dispatch key and because installing the `hunspell`
        // package is still the right hint (it typically pulls in a base
        // dictionary as a recommended dependency).
        ("Spelling dictionary", "hunspell", "Spell checking — optional", ToolKind::Package {
            apt: "hunspell", dnf: "hunspell", pacman: "hunspell", zypper: "hunspell",
        }, false),
        (
            "Skrizhal",
            "",
            "Optional companion app for CV Mode — a structured database of jobs, degrees and awards you can reuse across résumés",
            ToolKind::Flatpak { app_id: "io.github.calstfrancis.Skrizhal" },
            false,
        ),
    ] {
        let (row, ok, recheck) = tool_row(name, cmd, purpose, &distro, kind, required);
        group.add(&row);
        if let Some(f) = recheck {
            rechecks.push(f);
        }
        if required {
            all_ok = all_ok && ok;
        }
    }

    (group, all_ok, rechecks)
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

    if id.contains("ubuntu")
        || id.contains("debian")
        || id.contains("mint")
        || id_like.contains("ubuntu")
        || id_like.contains("debian")
    {
        Distro::Debian
    } else if id.contains("fedora")
        || id.contains("rhel")
        || id.contains("centos")
        || id_like.contains("fedora")
        || id_like.contains("rhel")
    {
        Distro::Fedora
    } else if id.contains("arch")
        || id.contains("manjaro")
        || id.contains("endeavour")
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
    Package {
        apt: &'a str,
        dnf: &'a str,
        pacman: &'a str,
        zypper: &'a str,
    },
    Flatpak {
        app_id: &'a str,
    },
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
        return (row, true, None);
    }

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
    // Some hints are a single long line — without wrapping, this one label's
    // minimum width forces the whole window wider than intended.
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

pub fn check_command(cmd: &str) -> bool {
    if cmd == "git" {
        return crate::git_sync::git_available();
    }
    // Spell checking reads system dictionary files directly (spellcheck.rs)
    // rather than shelling out to a `hunspell` binary, so "installed" means
    // a usable .aff/.dic pair exists, not that a command is on PATH — there
    // usually isn't one to find any more.
    if cmd == "hunspell" {
        return !crate::spellcheck::SpellChecker::available_languages().is_empty();
    }
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

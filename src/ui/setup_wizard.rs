//! Setup: three screens, one decision each.
//!
//! The old version was a single scrolling page of five independent sections,
//! each with its own Apply button — seven separate actions, in an order
//! nothing announced, starting with a request for a git name and email. This
//! asks for as little as it can instead: signing in with GitHub supplies the
//! identity, the folder name supplies the repository name, and everything
//! else (creating the repository, initialising git, the first commit and
//! push) happens behind one button.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{sync_channel, Receiver, TryRecvError};
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, LinkButton, Orientation,
};
use libadwaita as adw;
use adw::prelude::*;

use super::github_signin;

pub struct SetupWizard {
    window: adw::Window,
}

/// What the user chose to do, resolved into the work the final screen runs.
#[derive(Clone)]
enum Plan {
    /// Create a repository on the signed-in GitHub account and push to it.
    Github { repo_name: String, private: bool },
    /// Push to a repository the user already has.
    ExistingRemote { url: String },
    /// Push to a folder — a synced drive, a USB stick — with no account.
    Folder { path: PathBuf },
}

/// One line of the progress list on the final screen.
enum Progress {
    Step(usize),
    Done(String),
    Failed { step: usize, message: String },
}

#[derive(Default)]
struct State {
    identity: Option<crate::github_auth::Identity>,
}

impl SetupWizard {
    pub fn new(parent: &impl IsA<gtk4::Window>, work_dir: &Path) -> Self {
        let work_dir = work_dir.to_path_buf();

        let window = adw::Window::builder()
            .title("Set Up Zerkalo")
            .transient_for(parent)
            .modal(true)
            // Tall enough that the connect screen's alternatives are all above
            // the fold — a fallback nobody scrolls to is a fallback nobody has.
            .default_width(560)
            .default_height(680)
            .build();

        let nav = adw::NavigationView::new();
        let state = Rc::new(RefCell::new(State::default()));

        nav.add(&welcome_page(&window, &nav));
        nav.add(&connect_page(&window, &nav, &state, &work_dir));
        nav.add(&folder_page(&window, &nav, &work_dir));
        nav.add(&existing_page(&window, &nav, &work_dir));
        nav.add(&confirm_page(&window, &nav, &state, &work_dir));

        window.set_content(Some(&nav));

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }

    /// Whether setup should open by itself on startup.
    ///
    /// Once it has been finished or declined it never reappears — being asked
    /// again on every launch reads as the app not having listened.
    pub fn should_show(work_dir: &Path) -> bool {
        if crate::config::shared().borrow().setup_done {
            return false;
        }
        !has_git_remote(work_dir)
    }

    fn mark_done() {
        let _ = crate::config::update(|c| c.setup_done = true);
    }
}

// ── Page scaffolding ─────────────────────────────────────────────────────────

/// Every screen is the same shape: a headline, a paragraph, content, and the
/// buttons pinned at the bottom — so moving between them doesn't move the
/// primary button around under the user's cursor.
fn page_shell(title: &str, tag: &str, heading: &str, blurb: &str) -> (adw::NavigationPage, GtkBox, GtkBox) {
    let header = adw::HeaderBar::new();
    header.add_css_class("fond-chrome");

    let body = GtkBox::new(Orientation::Vertical, 12);
    body.set_margin_start(24);
    body.set_margin_end(24);
    body.set_margin_top(24);
    body.set_margin_bottom(12);
    body.set_valign(Align::Start);

    let heading_lbl = Label::new(Some(heading));
    heading_lbl.set_xalign(0.0);
    heading_lbl.set_wrap(true);
    heading_lbl.add_css_class("title-2");
    body.append(&heading_lbl);

    if !blurb.is_empty() {
        let blurb_lbl = Label::new(Some(blurb));
        blurb_lbl.set_xalign(0.0);
        blurb_lbl.set_wrap(true);
        blurb_lbl.add_css_class("dim-label");
        blurb_lbl.set_margin_bottom(8);
        body.append(&blurb_lbl);
    }

    let content = GtkBox::new(Orientation::Vertical, 12);
    content.set_vexpand(true);
    body.append(&content);

    let buttons = GtkBox::new(Orientation::Horizontal, 8);
    buttons.set_halign(Align::End);
    buttons.set_margin_start(24);
    buttons.set_margin_end(24);
    buttons.set_margin_top(4);
    buttons.set_margin_bottom(20);

    let outer = GtkBox::new(Orientation::Vertical, 0);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(460);
    clamp.set_child(Some(&body));
    scroll.set_child(Some(&clamp));
    outer.append(&scroll);
    outer.append(&buttons);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.set_top_bar_style(adw::ToolbarStyle::Flat);
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&outer));

    let page = adw::NavigationPage::builder()
        .title(title)
        .tag(tag)
        .child(&toolbar_view)
        .build();

    (page, content, buttons)
}

fn primary(label: &str) -> Button {
    let btn = Button::with_label(label);
    btn.add_css_class("suggested-action");
    btn.add_css_class("pill");
    btn
}

fn quiet(label: &str) -> Button {
    let btn = Button::with_label(label);
    btn.add_css_class("flat");
    btn
}

// ── 1 · Welcome ──────────────────────────────────────────────────────────────

fn welcome_page(window: &adw::Window, nav: &adw::NavigationView) -> adw::NavigationPage {
    let (page, content, buttons) = page_shell(
        "Set Up Zerkalo",
        "welcome",
        "Keep your writing safe",
        "Zerkalo can save a copy of everything you write to a private place online, \
         and keep every earlier version of it. If your computer is lost or a document \
         goes wrong, nothing is gone.",
    );

    for (icon, text) in [
        ("document-save-symbolic", "Saved as you work, not just when you remember"),
        ("document-open-recent-symbolic", "Every past version kept, so you can go back"),
        ("channel-secure-symbolic", "Private by default — only you can see it"),
    ] {
        let row = GtkBox::new(Orientation::Horizontal, 12);
        let img = gtk4::Image::from_icon_name(icon);
        img.add_css_class("dim-label");
        row.append(&img);
        let lbl = Label::new(Some(text));
        lbl.set_xalign(0.0);
        lbl.set_wrap(true);
        row.append(&lbl);
        content.append(&row);
    }

    let not_now = quiet("Not now");
    {
        let window = window.clone();
        not_now.connect_clicked(move |_| {
            SetupWizard::mark_done();
            window.close();
        });
    }

    let start = primary("Set this up");
    {
        let nav = nav.clone();
        start.connect_clicked(move |_| nav.push_by_tag("connect"));
    }

    buttons.append(&not_now);
    buttons.append(&start);
    page
}

// ── 2 · Connect ──────────────────────────────────────────────────────────────

fn connect_page(
    window: &adw::Window,
    nav: &adw::NavigationView,
    state: &Rc<RefCell<State>>,
    work_dir: &Path,
) -> adw::NavigationPage {
    let (page, content, buttons) = page_shell(
        "Connect",
        "connect",
        "Connect a GitHub account",
        "GitHub stores the copy of your work. It's free, and it's where the versions live. \
         Signing in shows you a short code to type into your browser — Zerkalo never sees \
         your password.",
    );

    let signin = primary("Sign in with GitHub");
    signin.set_halign(Align::Center);
    signin.set_margin_top(8);
    {
        let window = window.clone();
        let nav = nav.clone();
        let state = state.clone();
        signin.connect_clicked(move |_| {
            let nav = nav.clone();
            let state = state.clone();
            github_signin::present(&window, move |username| {
                // The display name and commit address come from the account, so
                // the user is never asked to type either. Fetched in the
                // background; the confirm page fills it in when it lands.
                if let Some(token) = crate::secret_store::load_github_token() {
                    let (tx, rx) = sync_channel(1);
                    std::thread::spawn(move || {
                        let _ = tx.send(crate::github_auth::fetch_identity(&token));
                    });
                    let state = state.clone();
                    let login = username.clone();
                    glib::timeout_add_local(Duration::from_millis(150), move || {
                        match rx.try_recv() {
                            Ok(Ok(identity)) => {
                                state.borrow_mut().identity = Some(identity);
                                glib::ControlFlow::Break
                            }
                            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                                // Falling back to the login alone still produces
                                // a valid commit address.
                                state.borrow_mut().identity = Some(crate::github_auth::Identity {
                                    name: login.clone(),
                                    email: format!("{login}@users.noreply.github.com"),
                                    login: login.clone(),
                                });
                                glib::ControlFlow::Break
                            }
                            Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                        }
                    });
                }
                nav.push_by_tag("confirm");
            });
        });
    }
    content.append(&signin);

    let signup = LinkButton::with_label(
        "https://github.com/signup",
        "Don't have an account? Create one — it's free",
    );
    signup.add_css_class("flat");
    signup.set_halign(Align::Center);
    content.append(&signup);

    let sep = GtkBox::new(Orientation::Horizontal, 8);
    sep.set_margin_top(12);
    let other = Label::new(Some("Other ways to keep it safe"));
    other.add_css_class("dim-label");
    other.add_css_class("caption");
    other.set_xalign(0.0);
    sep.append(&other);
    content.append(&sep);

    let group = adw::PreferencesGroup::new();

    let folder_row = adw::ActionRow::new();
    folder_row.set_title("Back up to a folder or drive");
    folder_row.set_subtitle("A synced folder like Nextcloud or pCloud, or a USB drive — no account needed");
    folder_row.set_activatable(true);
    folder_row.add_prefix(&gtk4::Image::from_icon_name("folder-symbolic"));
    folder_row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
    {
        let nav = nav.clone();
        folder_row.connect_activated(move |_| nav.push_by_tag("folder"));
    }
    group.add(&folder_row);

    let existing_row = adw::ActionRow::new();
    existing_row.set_title("I already have an online copy");
    existing_row.set_subtitle("Paste its address");
    existing_row.set_activatable(true);
    existing_row.add_prefix(&gtk4::Image::from_icon_name("insert-link-symbolic"));
    existing_row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
    {
        let nav = nav.clone();
        existing_row.connect_activated(move |_| nav.push_by_tag("existing"));
    }
    group.add(&existing_row);

    content.append(&group);

    let _ = work_dir;

    let skip = quiet("Skip — don't back up my work");
    {
        let window = window.clone();
        skip.connect_clicked(move |_| {
            SetupWizard::mark_done();
            window.close();
        });
    }
    buttons.append(&skip);
    page
}

// ── 3a · Folder backup ───────────────────────────────────────────────────────

fn folder_page(window: &adw::Window, nav: &adw::NavigationView, work_dir: &Path) -> adw::NavigationPage {
    let work_dir = work_dir.to_path_buf();
    let (page, content, buttons) = page_shell(
        "Folder Backup",
        "folder",
        "Back up to a folder",
        "Pick somewhere that isn't this computer's own disk — a folder your cloud service \
         syncs, or a drive you plug in. Every version of your work is copied there each \
         time you sync.",
    );

    let chosen: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

    let group = adw::PreferencesGroup::new();
    let path_row = adw::ActionRow::new();
    path_row.set_title("Backup folder");
    path_row.set_subtitle("Nothing chosen yet");
    path_row.set_activatable(true);
    group.add(&path_row);
    content.append(&group);

    let finish = primary("Finish");
    finish.set_sensitive(false);

    {
        let window = window.clone();
        let chosen = chosen.clone();
        let path_row_c = path_row.clone();
        let finish_c = finish.clone();
        path_row.connect_activated(move |_| {
            let fd = gtk4::FileDialog::new();
            let chosen = chosen.clone();
            let path_row_c = path_row_c.clone();
            let finish_c = finish_c.clone();
            fd.select_folder(Some(&window), None::<&gtk4::gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        path_row_c.set_subtitle(&path.display().to_string());
                        *chosen.borrow_mut() = Some(path);
                        finish_c.set_sensitive(true);
                    }
                }
            });
        });
    }

    {
        let nav = nav.clone();
        let chosen = chosen.clone();
        let work_dir = work_dir.clone();
        finish.connect_clicked(move |_| {
            let Some(path) = chosen.borrow().clone() else { return };
            nav.push(&working_page(&nav, &work_dir, Plan::Folder { path }, None));
        });
    }

    buttons.append(&finish);
    page
}

// ── 3b · An existing repository ──────────────────────────────────────────────

fn existing_page(window: &adw::Window, nav: &adw::NavigationView, work_dir: &Path) -> adw::NavigationPage {
    let work_dir = work_dir.to_path_buf();
    let (page, content, buttons) = page_shell(
        "Existing Copy",
        "existing",
        "Use an online copy you already have",
        "Paste its address. If it's on GitHub and you sign in first, Zerkalo can \
         save to it without asking for a password each time.",
    );

    let group = adw::PreferencesGroup::new();
    let url_row = adw::EntryRow::new();
    url_row.set_title("Address");
    group.add(&url_row);
    content.append(&group);

    let error_lbl = Label::new(None);
    error_lbl.set_xalign(0.0);
    error_lbl.set_wrap(true);
    error_lbl.add_css_class("error");
    error_lbl.set_visible(false);
    content.append(&error_lbl);

    let finish = primary("Finish");
    {
        let nav = nav.clone();
        let url_row_c = url_row.clone();
        let error_lbl_c = error_lbl.clone();
        let work_dir = work_dir.clone();
        finish.connect_clicked(move |_| {
            let url = url_row_c.text().trim().to_string();
            if url.is_empty() {
                error_lbl_c.set_label("Paste its address first.");
                error_lbl_c.set_visible(true);
                return;
            }
            error_lbl_c.set_visible(false);
            nav.push(&working_page(&nav, &work_dir, Plan::ExistingRemote { url }, None));
        });
    }

    let _ = window;
    buttons.append(&finish);
    page
}

// ── 4 · Confirm ──────────────────────────────────────────────────────────────

fn confirm_page(
    window: &adw::Window,
    nav: &adw::NavigationView,
    state: &Rc<RefCell<State>>,
    work_dir: &Path,
) -> adw::NavigationPage {
    let work_dir = work_dir.to_path_buf();
    let (page, content, buttons) = page_shell(
        "Confirm",
        "confirm",
        "Where your work will be kept",
        "",
    );

    let group = adw::PreferencesGroup::new();

    let name_row = adw::EntryRow::new();
    name_row.set_title("What to call it on GitHub");
    name_row.set_text(&crate::github_auth::suggested_repo_name(&work_dir));
    group.add(&name_row);

    let private_row = adw::SwitchRow::new();
    private_row.set_title("Private");
    private_row.set_subtitle("Only you can see it");
    private_row.set_active(true);
    group.add(&private_row);

    content.append(&group);

    // Shown, not asked: the name and address that will be recorded against each
    // saved version, taken from the GitHub account.
    let identity_lbl = Label::new(None);
    identity_lbl.set_xalign(0.0);
    identity_lbl.set_wrap(true);
    identity_lbl.add_css_class("dim-label");
    identity_lbl.add_css_class("caption");
    content.append(&identity_lbl);

    {
        let state = state.clone();
        let identity_lbl = identity_lbl.clone();
        page.connect_shown(move |_| {
            let text = match state.borrow().identity.as_ref() {
                Some(id) => format!("Saved versions will be recorded as {} <{}>", id.name, id.email),
                None => String::new(),
            };
            identity_lbl.set_label(&text);
        });
    }

    let finish = primary("Finish");
    {
        let nav = nav.clone();
        let name_row_c = name_row.clone();
        let private_row_c = private_row.clone();
        let state = state.clone();
        let work_dir = work_dir.clone();
        finish.connect_clicked(move |_| {
            let repo_name = crate::github_auth::sanitize_repo_name(&name_row_c.text());
            let private = private_row_c.is_active();
            let identity = state.borrow().identity.clone();
            nav.push(&working_page(
                &nav,
                &work_dir,
                Plan::Github { repo_name, private },
                identity,
            ));
        });
    }

    let _ = window;
    buttons.append(&finish);
    page
}

// ── 5 · Doing the work ───────────────────────────────────────────────────────

fn step_labels(plan: &Plan) -> Vec<&'static str> {
    match plan {
        Plan::Github { .. } => vec![
            "Preparing your folder",
            "Creating the repository",
            "Linking it to this folder",
            "Uploading your work",
        ],
        Plan::ExistingRemote { .. } => vec![
            "Preparing your folder",
            "Linking it to this folder",
            "Uploading your work",
        ],
        Plan::Folder { .. } => vec![
            "Preparing your folder",
            "Setting up the backup location",
            "Copying your work across",
        ],
    }
}

fn working_page(
    nav: &adw::NavigationView,
    work_dir: &Path,
    plan: Plan,
    identity: Option<crate::github_auth::Identity>,
) -> adw::NavigationPage {
    let work_dir = work_dir.to_path_buf();
    let (page, content, buttons) = page_shell("Setting Up", "working", "Setting things up", "");

    let labels = step_labels(&plan);
    let mut rows: Vec<(adw::ActionRow, gtk4::Spinner, Label)> = Vec::new();
    let group = adw::PreferencesGroup::new();
    for label in &labels {
        let row = adw::ActionRow::new();
        row.set_title(label);
        let spinner = gtk4::Spinner::new();
        spinner.set_valign(Align::Center);
        let tick = Label::new(None);
        tick.set_valign(Align::Center);
        row.add_suffix(&spinner);
        row.add_suffix(&tick);
        group.add(&row);
        rows.push((row, spinner, tick));
    }
    content.append(&group);

    let error_lbl = Label::new(None);
    error_lbl.set_xalign(0.0);
    error_lbl.set_wrap(true);
    error_lbl.set_selectable(true);
    error_lbl.add_css_class("error");
    error_lbl.set_visible(false);
    error_lbl.set_margin_top(12);
    content.append(&error_lbl);

    let back_btn = quiet("Back");
    back_btn.set_visible(false);
    {
        let nav = nav.clone();
        back_btn.connect_clicked(move |_| {
            nav.pop();
        });
    }
    buttons.append(&back_btn);

    let rx = run_plan(work_dir, plan.clone(), identity);

    let nav_c = nav.clone();
    let rows = Rc::new(rows);
    {
        let rows = rows.clone();
        let error_lbl = error_lbl.clone();
        let back_btn = back_btn.clone();
        if let Some((_, spinner, _)) = rows.first() {
            spinner.set_spinning(true);
        }
        glib::timeout_add_local(Duration::from_millis(120), move || {
            loop {
                match rx.try_recv() {
                    Ok(Progress::Step(i)) => {
                        if i > 0 {
                            if let Some((_, spinner, tick)) = rows.get(i - 1) {
                                spinner.set_spinning(false);
                                spinner.set_visible(false);
                                tick.set_label("✓");
                                tick.add_css_class("success");
                            }
                        }
                        if let Some((_, spinner, _)) = rows.get(i) {
                            spinner.set_spinning(true);
                        }
                    }
                    Ok(Progress::Done(summary)) => {
                        for (_, spinner, tick) in rows.iter() {
                            spinner.set_spinning(false);
                            spinner.set_visible(false);
                            if tick.label().is_empty() {
                                tick.set_label("✓");
                                tick.add_css_class("success");
                            }
                        }
                        SetupWizard::mark_done();
                        nav_c.push(&done_page(&nav_c, &summary));
                        return glib::ControlFlow::Break;
                    }
                    Ok(Progress::Failed { step, message }) => {
                        for (_, spinner, _) in rows.iter() {
                            spinner.set_spinning(false);
                            spinner.set_visible(false);
                        }
                        if let Some((_, _, tick)) = rows.get(step) {
                            tick.set_label("✗");
                            tick.add_css_class("error");
                        }
                        error_lbl.set_label(&message);
                        error_lbl.set_visible(true);
                        back_btn.set_visible(true);
                        return glib::ControlFlow::Break;
                    }
                    Err(TryRecvError::Empty) => return glib::ControlFlow::Continue,
                    Err(TryRecvError::Disconnected) => return glib::ControlFlow::Break,
                }
            }
        });
    }

    page
}

fn done_page(nav: &adw::NavigationView, summary: &str) -> adw::NavigationPage {
    let (page, content, buttons) = page_shell("Done", "done", "You're set up", "");

    let icon = gtk4::Image::from_icon_name("emblem-ok-symbolic");
    icon.set_pixel_size(48);
    icon.add_css_class("success");
    icon.set_margin_top(8);
    content.append(&icon);

    let lbl = Label::new(Some(summary));
    lbl.set_xalign(0.0);
    lbl.set_wrap(true);
    lbl.set_selectable(true);
    content.append(&lbl);

    let hint = Label::new(Some(
        "From now on, press Ctrl+Shift+S whenever you want to save a version and send it up. \
         Zerkalo will tell you when it has.",
    ));
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    hint.add_css_class("dim-label");
    hint.set_margin_top(8);
    content.append(&hint);

    let start = primary("Start writing");
    {
        let nav = nav.clone();
        start.connect_clicked(move |btn| {
            let _ = nav;
            if let Some(root) = btn.root() {
                if let Ok(win) = root.downcast::<gtk4::Window>() {
                    win.close();
                }
            }
        });
    }
    buttons.append(&start);
    page
}

// ── The work itself ──────────────────────────────────────────────────────────

/// Runs the plan on a background thread, reporting which step it is on.
///
/// Everything here blocks — process spawns and network calls — so none of it
/// may touch the interface thread.
fn run_plan(
    work_dir: PathBuf,
    plan: Plan,
    identity: Option<crate::github_auth::Identity>,
) -> Receiver<Progress> {
    let (tx, rx) = sync_channel::<Progress>(8);
    std::thread::spawn(move || {
        let token = crate::secret_store::load_github_token();
        let mut step = 0usize;
        let _ = tx.send(Progress::Step(step));

        // 1 · The folder is a git repository, and knows who is writing.
        if let Err(e) = prepare_repo(&work_dir, identity.as_ref()) {
            let _ = tx.send(Progress::Failed { step, message: e });
            return;
        }

        let summary = match plan {
            Plan::Github { repo_name, private } => {
                let Some(token) = token.clone() else {
                    let _ = tx.send(Progress::Failed {
                        step,
                        message: "The GitHub sign-in didn't finish. Go back and sign in again.".into(),
                    });
                    return;
                };

                step += 1;
                let _ = tx.send(Progress::Step(step));
                let clone_url = match crate::github_auth::create_repo(&token, &repo_name, private) {
                    Ok(url) => url,
                    Err(e) => {
                        let _ = tx.send(Progress::Failed {
                            step,
                            message: describe_create_failure(&repo_name, &e),
                        });
                        return;
                    }
                };

                step += 1;
                let _ = tx.send(Progress::Step(step));
                if let Err(e) = set_git_remote(&work_dir, &clone_url) {
                    let _ = tx.send(Progress::Failed {
                        step,
                        message: format!("It was created on GitHub, but linking it to this folder failed:\n{e}"),
                    });
                    return;
                }

                format!("Your work is saved to {}", clone_url.trim_end_matches(".git"))
            }
            Plan::ExistingRemote { url } => {
                step += 1;
                let _ = tx.send(Progress::Step(step));
                if let Err(e) = set_git_remote(&work_dir, &url) {
                    let _ = tx.send(Progress::Failed {
                        step,
                        message: format!("Couldn't use that address:\n{e}"),
                    });
                    return;
                }
                format!("Your work is saved to {}", url.trim_end_matches(".git"))
            }
            Plan::Folder { path } => {
                step += 1;
                let _ = tx.send(Progress::Step(step));
                if let Err(e) = crate::git_sync::add_backup_remote(&work_dir, &path.display().to_string()) {
                    let _ = tx.send(Progress::Failed {
                        step,
                        message: format!("Couldn't set up that folder as a backup:\n{e}"),
                    });
                    return;
                }
                format!("Your work is backed up to {}", path.display())
            }
        };

        // Last step, whichever route: commit what's there and push it.
        step += 1;
        let _ = tx.send(Progress::Step(step));
        let result = crate::git_sync::sync(&work_dir, token.as_deref());
        if let Some(err) = result.error {
            let _ = tx.send(Progress::Failed {
                step,
                message: format!("Couldn't save the first version:\n{err}"),
            });
            return;
        }
        if !result.pushed && !result.push_errors.is_empty() {
            let _ = tx.send(Progress::Failed {
                step,
                message: format!(
                    "Everything is set up, but the first upload didn't go through:\n{}\n\n\
                     Your work is saved on this computer. Try Sync again from the main window.",
                    result.push_errors.join("\n")
                ),
            });
            return;
        }

        let _ = tx.send(Progress::Done(summary));
    });
    rx
}

/// Makes the work folder a git repository and gives git a name and email to
/// record, without asking for either.
fn prepare_repo(work_dir: &Path, identity: Option<&crate::github_auth::Identity>) -> Result<(), String> {
    std::fs::create_dir_all(work_dir)
        .map_err(|e| format!("Couldn't create the folder {}:\n{e}", work_dir.display()))?;

    if git2::Repository::discover(work_dir).is_err() {
        let repo = git2::Repository::init(work_dir)
            .map_err(|e| format!("Couldn't set up version history here:\n{}", e.message()))?;
        // GitHub's default branch is main; without this, git2 starts on
        // whatever init.defaultBranch says (often master) and the first push
        // creates a second, unrelated branch.
        let _ = repo.set_head("refs/heads/main");
    }

    let (name, email) = git_identity();
    if !name.is_empty() && !email.is_empty() {
        return Ok(());
    }

    let (name, email) = match identity {
        Some(id) => (id.name.clone(), id.email.clone()),
        None => system_identity(),
    };
    set_git_identity(&name, &email)
}

/// A name and address derived from the account on this computer, for the paths
/// that have no GitHub account to take one from. Git would otherwise refuse to
/// commit at all on a machine where it can't guess one.
fn system_identity() -> (String, String) {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "writer".to_string());
    let host = gtk4::glib::host_name().to_string();
    let host = if host.is_empty() { "localhost".to_string() } else { host };
    (user.clone(), format!("{user}@{host}"))
}

/// Turns GitHub's API errors into something that says what to do next.
fn describe_create_failure(name: &str, e: &crate::github_auth::GithubAuthError) -> String {
    let text = e.to_string();
    if text.contains("already exists") || text.contains("name already exists") {
        return format!(
            "You already have something called \"{name}\" on GitHub. Go back and pick a \
             different name, or use \"I already have an online copy\" to point Zerkalo at \
             the existing one."
        );
    }
    if text.contains("401") || text.contains("Bad credentials") {
        return "GitHub didn't accept the sign-in. Go back and sign in again.".to_string();
    }
    if text.contains("Network") {
        return "Couldn't reach GitHub. Check your internet connection and try again.".to_string();
    }
    format!("GitHub couldn't create it:\n{text}")
}

// ── Git helpers ───────────────────────────────────────────────────────────────

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

pub fn set_git_identity(name: &str, email: &str) -> Result<(), String> {
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
    let _ = repo.remote_delete("origin");
    repo.remote("origin", url).map_err(|e| e.message().to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_route_lists_the_steps_it_actually_runs() {
        // The progress list is what tells the user how far along they are, so
        // a route that skips repository creation must not display it.
        assert_eq!(step_labels(&Plan::Github { repo_name: "x".into(), private: true }).len(), 4);
        assert_eq!(step_labels(&Plan::ExistingRemote { url: "x".into() }).len(), 3);
        assert_eq!(step_labels(&Plan::Folder { path: PathBuf::from("/tmp") }).len(), 3);
    }

    #[test]
    fn a_system_identity_is_always_usable_as_a_commit_address() {
        let (name, email) = system_identity();
        assert!(!name.is_empty(), "git refuses to commit without a name");
        assert!(email.contains('@'), "git requires an address shaped like an email: {email}");
    }

    #[test]
    fn a_duplicate_repository_name_is_explained_rather_than_echoed() {
        let e = crate::github_auth::GithubAuthError::Api(
            "422: {\"message\":\"Repository creation failed.\",\"errors\":[{\"message\":\"name already exists on this account\"}]}".into(),
        );
        let msg = describe_create_failure("zerkalo-docs", &e);
        assert!(msg.contains("already have something called \"zerkalo-docs\""), "got: {msg}");
        assert!(!msg.contains("422"), "the raw status code is not useful here: {msg}");
    }

    #[test]
    fn an_expired_sign_in_points_back_at_signing_in() {
        let e = crate::github_auth::GithubAuthError::Api("401: Bad credentials".into());
        let msg = describe_create_failure("zerkalo-docs", &e);
        assert!(msg.contains("sign in again"), "got: {msg}");
    }

    #[test]
    fn preparing_a_folder_creates_the_repository_and_sets_the_branch_to_main() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("Zerkalo");
        let identity = crate::github_auth::Identity {
            login: "octocat".into(),
            name: "Octo Cat".into(),
            email: "1+octocat@users.noreply.github.com".into(),
        };
        prepare_repo(&work, Some(&identity)).unwrap();

        let repo = git2::Repository::discover(&work).expect("work folder is a repository");
        assert_eq!(
            repo.head().err().map(|_| "unborn"),
            Some("unborn"),
            "a fresh repository has no commits yet"
        );
        let head_ref = repo.find_reference("HEAD").unwrap();
        assert_eq!(
            head_ref.symbolic_target(),
            Some("refs/heads/main"),
            "GitHub's default branch is main; starting on master creates a second branch on first push"
        );
    }

    #[test]
    fn preparing_an_existing_repository_twice_is_harmless() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().to_path_buf();
        prepare_repo(&work, None).unwrap();
        prepare_repo(&work, None).expect("running setup again must not fail");
    }
}

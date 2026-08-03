//! Windows that only present information: document statistics and the
//! in-app changelog (rendered live from `CHANGELOG.md`).
//! Split out of `app_window.rs`.

use gtk4::prelude::*;
use gtk4::Orientation;
use libadwaita as adw;
use adw::prelude::*;
use glib::object::IsA;
use super::super::editor_pane::{strip_typst_markup, strip_zerkalo_blocks};

pub(super) fn show_doc_stats(
    parent: &impl IsA<gtk4::Window>,
    text: &str,
    session_start: u32,
    _project_root: Option<&std::path::Path>,
) {
    let content = strip_typst_markup(&strip_zerkalo_blocks(text));
    let words = content.split_whitespace().count();
    let chars = content.chars().filter(|c| !c.is_whitespace()).count();
    let chars_with_spaces = content.chars().count();
    let paragraphs = content.split("\n\n").filter(|s| !s.trim().is_empty()).count();
    let sentences = content
        .split(['.', '!', '?'])
        .filter(|s| !s.trim().is_empty())
        .count();
    let reading_mins = if words < 200 { "<1".to_string() } else { format!("{}", words / 200) };
    let session_delta = if words as u32 > session_start {
        format!("+{}", words as u32 - session_start)
    } else if (words as u32) < session_start {
        format!("{}", words as i64 - session_start as i64)
    } else {
        "±0".to_string()
    };

    let win = adw::Window::new();
    win.set_title(Some("Document Statistics"));
    win.set_default_width(360);
    win.set_resizable(false);
    win.set_transient_for(Some(parent));
    win.set_modal(false);

    let make_row = |title: &str, value: &str| {
        let row = adw::ActionRow::new();
        row.set_title(title);
        let lbl = gtk4::Label::new(Some(value));
        lbl.add_css_class("dim-label");
        lbl.set_valign(gtk4::Align::Center);
        row.add_suffix(&lbl);
        row
    };

    let group = adw::PreferencesGroup::new();
    group.set_margin_start(12);
    group.set_margin_end(12);
    group.set_margin_top(12);
    group.set_margin_bottom(12);
    group.add(&make_row("Words", &format!("{words}  ({session_delta} this session)")));
    group.add(&make_row("Characters", &format!("{chars}  ({chars_with_spaces} with spaces)")));
    group.add(&make_row("Paragraphs", &paragraphs.to_string()));
    group.add(&make_row("Sentences", &sentences.to_string()));
    group.add(&make_row("Reading time", &format!("{reading_mins} min")));

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Never);
    scroll.set_child(Some(&group));

    let header = adw::HeaderBar::new();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroll));
    win.set_content(Some(&toolbar));
    win.present();
}

pub(super) fn show_changelog(parent: &impl IsA<gtk4::Window>) {
    const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");
    const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

    let win = adw::Window::new();
    win.set_title(Some("Changelog — Zerkalo"));
    win.set_default_width(720);
    win.set_default_height(680);
    win.set_transient_for(Some(parent));
    win.set_modal(false);

    let header = adw::HeaderBar::new();
    let title_widget = adw::WindowTitle::new(
        "Changelog",
        &format!("You're on v{CURRENT_VERSION}"),
    );
    header.set_title_widget(Some(&title_widget));

    let body = gtk4::Box::new(Orientation::Vertical, 4);
    body.set_margin_start(24);
    body.set_margin_end(24);
    body.set_margin_top(16);
    body.set_margin_bottom(24);

    let mut first_heading = true;
    for line in CHANGELOG.lines() {
        let trimmed = line.trim();
        if let Some(inner) = trimmed.strip_prefix("## [") {
            // "## [0.16.1-dev4] — Skrizhal CV element integration"
            // Version and title are split onto their own rows — a version
            // tag is always short (never needs eliding), and the title can
            // then wrap freely instead of being cut off with "…".
            let (version, rest) = match inner.split_once(']') {
                Some((v, r)) => (v, r.trim()),
                None => (inner.trim_end_matches(']'), ""),
            };
            let title = rest.strip_prefix("— ").unwrap_or(rest);

            let heading_row = gtk4::Box::new(Orientation::Horizontal, 8);
            heading_row.set_margin_top(if first_heading { 0 } else { 22 });
            first_heading = false;

            let ver_lbl = gtk4::Label::new(Some(version));
            ver_lbl.add_css_class("monospace");
            ver_lbl.add_css_class("dim-label");
            ver_lbl.add_css_class("caption-heading");
            ver_lbl.set_xalign(0.0);
            heading_row.append(&ver_lbl);

            if version == CURRENT_VERSION {
                let badge = gtk4::Label::new(Some("· Current"));
                badge.add_css_class("caption-heading");
                badge.add_css_class("accent");
                heading_row.append(&badge);
            }
            body.append(&heading_row);

            if !title.is_empty() {
                let title_lbl = gtk4::Label::new(Some(title));
                title_lbl.add_css_class("title-3");
                title_lbl.set_xalign(0.0);
                title_lbl.set_wrap(true);
                title_lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
                title_lbl.set_margin_bottom(2);
                body.append(&title_lbl);
            }
        } else if let Some(text) = trimmed.strip_prefix("### ") {
            let lbl = gtk4::Label::new(Some(text));
            lbl.add_css_class("heading");
            lbl.set_xalign(0.0);
            lbl.set_margin_top(8);
            lbl.set_margin_start(4);
            lbl.set_margin_bottom(2);
            lbl.set_wrap(true);
            body.append(&lbl);
        } else if let Some(content) = trimmed.strip_prefix("- ") {
            body.append(&changelog_bullet(content));
        }
    }

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(700);
    clamp.set_child(Some(&body));
    scroll.set_child(Some(&clamp));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroll));
    win.set_content(Some(&toolbar));
    win.present();
}

fn changelog_bullet(text: &str) -> gtk4::Box {
    let row = gtk4::Box::new(Orientation::Horizontal, 8);
    row.set_margin_start(8);
    let dot = gtk4::Label::new(Some("•"));
    dot.set_valign(gtk4::Align::Start);
    dot.add_css_class("dim-label");
    dot.set_margin_top(1);

    let markup = md_inline_to_pango(text);
    let lbl = gtk4::Label::new(None);
    lbl.set_markup(&markup);
    lbl.set_xalign(0.0);
    lbl.set_wrap(true);
    lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    lbl.set_hexpand(true);
    lbl.set_halign(gtk4::Align::Fill);

    row.append(&dot);
    row.append(&lbl);
    row
}

fn md_inline_to_pango(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                out.push_str("<b>");
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'*') => { chars.next(); break; }
                        Some(ch) => inner.push(ch),
                        None => break,
                    }
                }
                out.push_str(&glib::markup_escape_text(&inner));
                out.push_str("</b>");
            }
            '`' => {
                out.push_str("<tt>");
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('`') => break,
                        Some(ch) => inner.push(ch),
                        None => break,
                    }
                }
                out.push_str(&glib::markup_escape_text(&inner));
                out.push_str("</tt>");
            }
            '[' => {
                // [text](url) → just text
                let mut link_text = String::new();
                loop {
                    match chars.next() {
                        Some(']') => break,
                        Some(ch) => link_text.push(ch),
                        None => break,
                    }
                }
                if chars.peek() == Some(&'(') {
                    // consume (url)
                    chars.next();
                    loop { match chars.next() { Some(')') | None => break, _ => {} } }
                }
                out.push_str(&glib::markup_escape_text(&link_text));
            }
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

//! Single home for Zerkalo's static, app-wide CSS. Previously this was split
//! across `app_window.rs`, `library_window.rs`, and `preview_pane.rs`, each
//! loading its own `CssProvider` — that made it easy for the same class
//! (e.g. hardcoded colors) to drift out of sync across files. Anything that
//! is a fixed class selector belongs here; per-widget/dynamic CSS (computed
//! colors for a specific category chip, live font-size providers, etc.)
//! still lives next to the code that computes it.

/// The suite's shared interface layer, vendored from fond-style. Loaded before
/// GLOBAL_CSS so app rules can still override it. Do not edit the copy in
/// `style/` — change it in fond-style and run its `sync.sh`, or the next sync
/// silently reverts you.
const FOND_CSS: &str = include_str!("../../style/fond.css");

const GLOBAL_CSS: &str = ".fond-accent-outline { color: #1F5E75; } \
    .fond-accent-citations { color: #8A6A24; } \
    .fond-accent-library { color: #1F5E75; } \
    .fond-accent-pinned { color: #8A6A24; } \
    /* A cue for a row that has no colour of its own — themed, so it stays \
       legible in both schemes, unlike a literal hex. */ \
    .fond-cue-neutral { background: alpha(@window_fg_color, 0.22); } \
    /* The library's multi-select. The list itself is SelectionMode::None \
       because selection is tracked by the window, so the :selected rule in \
       fond.css never applies — this says the same thing in a class. */ \
    .doc-selected { background: alpha(@window_fg_color, 0.09); } \
    .navigation-sidebar > row:hover:not(:selected) { \
        background-color: alpha(@accent_color, 0.08); \
    } \
    .navigation-sidebar > row:selected { \
        background-color: @accent_bg_color; \
        color: @accent_fg_color; \
    } \
    .linked > toggle:checked, \
    .linked > button:checked { \
        background-color: @accent_bg_color; \
        color: @accent_fg_color; \
    } \
    .paned > separator { \
        min-width: 5px; \
        min-height: 5px; \
        transition: background-color 150ms ease; \
    } \
    .paned > separator:hover { \
        background-color: alpha(@accent_color, 0.45); \
        -gtk-icon-source: -gtk-icontheme(\"col-resize-symbolic\"); \
    } \
    .zerkalo-sidebar { \
        transition: opacity 250ms; \
    } \
    /* Inline completion suggestion drawn after the cursor. Font properties are \
       inherited from the textview node, so it lines up with the real text. */ \
    .completion-ghost { \
        color: alpha(@window_fg_color, 0.42); \
    } \
    .zerkalo-sidebar entry, \
    .zerkalo-sidebar button, \
    .zerkalo-sidebar label { \
        min-width: 0; \
    } \
    window.zen-writing .zerkalo-sidebar { \
        opacity: 0.3; \
    } \
    window.zen-writing textview text { \
        padding-left: 40px; \
        padding-right: 40px; \
    } \
    window.high-contrast textview { \
        color: #ffffff; \
        background-color: #000000; \
    } \
    window.high-contrast textview text { \
        color: #ffffff; \
    } \
    textview.view { \
        caret-color: @accent_color; \
    } \
    textview text .current-line { \
        background-color: alpha(@accent_color, 0.10); \
    } \
    notebook tab button.circular { \
        min-width: 20px; \
        min-height: 20px; \
        padding: 2px; \
        transition: background-color 120ms ease; \
    } \
    notebook tab button.circular:hover { \
        background-color: alpha(@window_fg_color, 0.08); \
    } \
    notebook tab button.circular:active { \
        background-color: alpha(@window_fg_color, 0.16); \
    } \
    .modified-dot { \
        color: @accent_color; \
        font-size: 8px; \
    } \
    .statusbar-sep { \
        opacity: 0.25; \
    } \
    .status-toggle label { \
        opacity: 0.7; \
    } \
    .status-toggle:focus label, \
    .status-toggle:hover label { \
        opacity: 1.0; \
    } \
    .compile-progress { \
        min-height: 3px; \
        padding: 0; \
        border-radius: 0; \
    } \
    .table-grid-cell { \
        min-width: 0; \
        min-height: 0; \
        padding: 1px; \
        border-radius: 2px; \
        border: 1px solid alpha(@borders, 0.6); \
        background-color: alpha(@card_bg_color, 0.5); \
    } \
    .table-grid-cell:hover { \
        background-color: alpha(@accent_color, 0.15); \
        border-color: alpha(@accent_color, 0.4); \
    } \
    .table-grid-cell-selected { \
        background-color: alpha(@accent_color, 0.25); \
        border-color: @accent_color; \
    } \
    notebook stack { transition: opacity 120ms ease; } \
    notebook > stack { transition: all 150ms ease; } \
    revealer > * { transition: opacity 200ms ease; } \
    notebook > header > tabs > tab { \
        transition: background-color 120ms ease; \
    } \
    notebook > header > tabs > tab:not(:checked):hover { \
        background-color: alpha(@window_fg_color, 0.06); \
        transition: background-color 120ms ease; \
    } \
    notebook header.top { \
        box-shadow: inset -16px 0 12px -8px alpha(@window_bg_color, 0.7); \
    } \
    @keyframes pulse-opacity { \
        0%   { opacity: 1.0; } \
        50%  { opacity: 0.45; } \
        100% { opacity: 1.0; } \
    } \
    .compiling-pulse { \
        animation: pulse-opacity 1.2s ease-in-out infinite; \
    } \
    .compile-mode-manual { \
        color: @warning_color; \
    } \
    .compile-mode-auto { \
        color: @success_color; \
    } \
    .session-delta-positive { \
        color: @success_color; \
    } \
    .format-bar { \
        transition: opacity 120ms ease; \
    } \
    .breadcrumb-bar { \
        -gtk-icon-shadow: none; \
    } \
    .breadcrumb-scroll-fade { \
        box-shadow: inset 16px 0 12px -8px alpha(@window_bg_color, 0.7); \
    } \
    notebook > header > tabs > tab.reorderable-page:hover { \
        background-color: alpha(@accent_color, 0.08); \
    } \
    notebook > header > tabs > tab.dragged-tab { \
        opacity: 0.7; \
        background-color: alpha(@accent_color, 0.15); \
    } \
    @keyframes shake { \
        0%   { margin-left: 0px; } \
        20%  { margin-left: -6px; } \
        40%  { margin-left: 5px; } \
        60%  { margin-left: -4px; } \
        80%  { margin-left: 3px; } \
        100% { margin-left: 0px; } \
    } \
    .shake-banner { \
        animation: shake 0.5s ease-in-out; \
    } \
    .doc-title { \
        font-weight: 600; \
    } \
    .chip-active { \
        background: alpha(@accent_color, 0.25); \
    } \
    .zoom-osd { \
        background: alpha(@window_bg_color, 0.85); \
        border-radius: 6px; padding: 4px 10px; \
        font-size: 0.85em; font-weight: bold; \
        box-shadow: 0 1px 4px alpha(black, 0.3); \
        opacity: 1; transition: opacity 200ms ease; \
    } \
    .zoom-osd.osd-hidden { opacity: 0; }";

/// Loads all static, app-wide CSS once. Safe to call multiple times (GTK
/// dedupes identical providers by reference, and this is only invoked once
/// per process at startup in practice).
pub fn load_global_css() {
    let css = gtk4::CssProvider::new();
    css.load_from_data(&format!("{FOND_CSS}\n{GLOBAL_CSS}"));
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Draws icons from Adwaita whatever the desktop's icon theme is.
///
/// Symbolic icon *names* are shared between themes but the drawings are not.
/// Under KDE this resolves them from Breeze, where `document-save-symbolic` is
/// a floppy disk rather than a download arrow — fine icons, but a different
/// family from the one a libadwaita interface is drawn against, so the window
/// ends up mixing two icon languages. Only the icon theme is pinned; colour
/// scheme, accent and font still come from the system.
pub fn pin_icon_theme() {
    if let Some(settings) = gtk4::Settings::default() {
        settings.set_gtk_icon_theme_name(Some("Adwaita"));
    }
}

/// A section header in the suite's shared form: a coloured dot, a letterspaced
/// small-caps title, and a count set immediately after it. Used by the sidebar
/// panels so the outline and the citation list announce themselves the way a
/// section does everywhere else in the suite.
pub fn fond_section_header(title: &str, accent: &str) -> gtk4::Box {
    use gtk4::prelude::*;
    let bx = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    bx.add_css_class("fond-section");
    bx.set_margin_start(12);
    bx.set_margin_end(8);
    bx.set_margin_top(10);
    bx.set_margin_bottom(2);

    let dot = gtk4::Label::new(Some("\u{25cf}"));
    dot.add_css_class("fond-section-dot");
    dot.add_css_class(accent);
    dot.set_valign(gtk4::Align::Center);
    bx.append(&dot);

    let lbl = gtk4::Label::new(Some(title));
    lbl.add_css_class("fond-section-title");
    lbl.set_valign(gtk4::Align::Center);
    bx.append(&lbl);

    bx
}

/// A cue dot in the suite's form: a drawn 9x9 box, not a glyph. "●" renders at
/// roughly a third of its font size, so matching the design target by font-size
/// alone is hopeless, and a 50% radius does not round a box this small in GTK's
/// renderer — fond.css uses an absolute one.
///
/// Pass a hex colour for a cue that carries meaning (a category's colour), or
/// `None` for a row that has none, which draws a faint neutral instead of
/// leaving the titles unaligned.
pub fn fond_cue(color: Option<&str>) -> gtk4::Box {
    use gtk4::prelude::*;
    let cue = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    cue.add_css_class("fond-cue");
    cue.set_size_request(9, 9);
    cue.set_valign(gtk4::Align::Center);
    match color {
        Some(hex) => cue.add_css_class(&cue_class_for(hex)),
        None => cue.add_css_class("fond-cue-neutral"),
    }
    cue
}

/// A CSS class that paints a cue in `hex`, registered display-wide the first
/// time that colour is asked for.
///
/// The obvious way to colour one widget — its own `CssProvider` on its own
/// style context — has been deprecated since GTK 4.10, and a library of a few
/// hundred documents would build one provider per row besides. A class per
/// distinct colour is registered once and shared by every row using it.
fn cue_class_for(hex: &str) -> String {
    use std::cell::RefCell;
    use std::collections::HashSet;

    thread_local! {
        static REGISTERED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    }

    let slug: String = hex
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    let class = format!("fond-cue-{slug}");

    REGISTERED.with(|reg| {
        if !reg.borrow_mut().insert(class.clone()) {
            return;
        }
        let provider = gtk4::CssProvider::new();
        provider.load_from_data(&format!(".{class} {{ background: {hex}; }}"));
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });

    class
}

/// The count or summary that follows a section title.
pub fn fond_section_meta() -> gtk4::Label {
    use gtk4::prelude::*;
    let lbl = gtk4::Label::new(None);
    lbl.add_css_class("fond-section-meta");
    lbl.set_valign(gtk4::Align::Center);
    lbl
}

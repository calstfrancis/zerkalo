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
    .category-chip { \
        background: alpha(@accent_color, 0.15); \
        color: @accent_color; \
        border-radius: 4px; \
        padding: 1px 6px; \
        font-size: 0.8em; \
    } \
    .tag-chip { \
        background: alpha(@window_fg_color, 0.12); \
        border-radius: 4px; \
        padding: 1px 6px; \
        font-size: 0.75em; \
    } \
    .sidebar-header { \
        font-size: 0.75em; \
        font-weight: bold; \
        color: alpha(@window_fg_color, 0.55); \
        padding: 8px 12px 2px 12px; \
    } \
    .doc-title { \
        font-weight: 600; \
    } \
    .selected-doc { \
        background: alpha(@accent_color, 0.12); \
        border-left: 3px solid @accent_color; \
        padding-left: 5px; \
    } \
    .selected-doc:focus-visible { \
        outline: 2px solid @accent_color; \
        outline-offset: -2px; \
    } \
    .pinned-doc { \
        border-left: 2px solid @accent_color; \
        padding-left: 6px; \
    } \
    .compact-active { \
        font-weight: bold; \
    } \
    .count-badge { \
        min-width: 24px; \
        border-radius: 8px; \
        background: alpha(@window_fg_color, 0.08); \
        padding: 0 4px; \
        font-size: 0.8em; \
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

/// The count or summary that follows a section title.
pub fn fond_section_meta() -> gtk4::Label {
    use gtk4::prelude::*;
    let lbl = gtk4::Label::new(None);
    lbl.add_css_class("fond-section-meta");
    lbl.set_valign(gtk4::Align::Center);
    lbl
}

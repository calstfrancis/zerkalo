//! The status bar along the bottom: the name-as-label toggles, cursor and word-count readouts, version button and breadcrumb.

use super::*;

pub(super) struct StatusBar {
    pub(super) status_bar: GtkBox,
    pub(super) gost_label: Label,
    pub(super) gost_btn: Button,
    pub(super) autocorrect_label: Label,
    pub(super) autocorrect_btn: Button,
    pub(super) search_label: Label,
    pub(super) search_btn: Button,
    pub(super) focus_label: Label,
    pub(super) focus_toggle_btn: Button,
    pub(super) autosave_label: Label,
    pub(super) autosave_toggle_btn: Button,
    pub(super) format_bar_label: Label,
    pub(super) format_bar_toggle_btn: Button,
    pub(super) undo_btn: Button,
    pub(super) redo_btn: Button,
    pub(super) cursor_label: Label,
    pub(super) safe_label: Label,
    pub(super) lsp_status_label: Label,
    pub(super) diag_label: Label,
    pub(super) diag_btn: Button,
    pub(super) on_diag_click: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    pub(super) simple_mode_label: Label,
    pub(super) simple_mode_btn: Button,
    pub(super) section_wc_label: Label,
    pub(super) word_count_label: Label,
    pub(super) wc_btn: Button,
    pub(super) session_delta_label: Label,
    pub(super) goal_fraction: Rc<Cell<f64>>,
    pub(super) goal_celebrating: Rc<Cell<bool>>,
    pub(super) goal_ring: DrawingArea,
    pub(super) version_btn: Button,
    pub(super) breadcrumb_label: Label,
    pub(super) breadcrumb_bar: GtkBox,
}

pub(super) fn build_status_bar() -> StatusBar {
    let status_bar = GtkBox::new(Orientation::Horizontal, 0);
    status_bar.set_hexpand(true);
    status_bar.add_css_class("fond-chrome");
    status_bar.add_css_class("fond-statusbar");
    status_bar.add_css_class("fond-edge-top");

    // Lives in the hamburger menu — a whole-UI font switch is a setting you
    // change once, not something to keep a status-bar chip for.
    let gost_label = Label::new(Some("GOST Type B font"));
    gost_label.set_use_markup(true);
    gost_label.set_halign(gtk4::Align::Start);
    gost_label.set_hexpand(true);

    // Same row padding as make_menu_item() in app_window, so it lines up
    // with the menu items either side of it.
    let gost_row = GtkBox::new(Orientation::Horizontal, 0);
    gost_row.set_margin_start(4);
    gost_row.set_margin_end(6);
    gost_row.append(&gost_label);

    let gost_btn = Button::new();
    gost_btn.set_child(Some(&gost_row));
    gost_btn.add_css_class("flat");
    gost_btn.set_tooltip_text(Some("Toggle GOST type B engineering font for the whole UI"));

    // Matches gost_label above and make_menu_item()'s rows: this button is
    // packed into the hamburger, not the status bar, so it drops the
    // dim/caption status-toggle styling that made it read as a stray.
    let autocorrect_label = Label::new(Some("Autocorrect"));
    autocorrect_label.set_use_markup(true);
    autocorrect_label.set_halign(gtk4::Align::Start);
    autocorrect_label.set_hexpand(true);

    let autocorrect_row = GtkBox::new(Orientation::Horizontal, 0);
    autocorrect_row.set_margin_start(4);
    autocorrect_row.set_margin_end(6);
    autocorrect_row.append(&autocorrect_label);

    let autocorrect_btn = Button::new();
    autocorrect_btn.set_child(Some(&autocorrect_row));
    autocorrect_btn.add_css_class("flat");
    autocorrect_btn.set_tooltip_text(Some("Toggle autocorrect (fixes spelling as you type)"));
    autocorrect_btn.update_property(&[gtk4::accessible::Property::Label("Toggle autocorrect")]);

    let search_label = Label::new(Some("search"));
    search_label.add_css_class("dim-label");
    search_label.add_css_class("caption");
    search_label.set_use_markup(true);
    search_label.set_margin_top(3);
    search_label.set_margin_bottom(3);
    let search_btn = Button::new();
    search_btn.set_child(Some(&search_label));
    search_btn.add_css_class("flat");
    search_btn.add_css_class("status-toggle");
    search_btn.set_tooltip_text(Some("Find & Replace (Ctrl+F)"));
    search_btn.set_margin_start(4);
    search_btn.set_margin_end(4);
    let focus_label = Label::new(Some("focus"));
    focus_label.add_css_class("dim-label");
    focus_label.add_css_class("caption");
    focus_label.set_use_markup(true);
    focus_label.set_margin_top(3);
    focus_label.set_margin_bottom(3);

    let focus_toggle_btn = Button::new();
    focus_toggle_btn.set_child(Some(&focus_label));
    focus_toggle_btn.add_css_class("flat");
    focus_toggle_btn.add_css_class("status-toggle");
    focus_toggle_btn.set_tooltip_text(Some("Focus mode — hide sidebar and preview"));
    focus_toggle_btn.set_margin_end(4);
    focus_toggle_btn.update_property(&[gtk4::accessible::Property::Label("Toggle focus mode")]);

    let autosave_label = Label::new(Some("autosave"));
    autosave_label.add_css_class("dim-label");
    autosave_label.add_css_class("caption");
    autosave_label.set_use_markup(true);
    autosave_label.set_margin_top(3);
    autosave_label.set_margin_bottom(3);
    let autosave_toggle_btn = Button::new();
    autosave_toggle_btn.set_child(Some(&autosave_label));
    autosave_toggle_btn.add_css_class("flat");
    autosave_toggle_btn.add_css_class("status-toggle");
    autosave_toggle_btn.set_tooltip_text(Some(
        "Autosave — save the document a few seconds after you stop typing, and \
         whenever you switch away, compile, export or quit",
    ));
    autosave_toggle_btn.set_margin_end(4);
    autosave_toggle_btn.update_property(&[gtk4::accessible::Property::Label("Toggle autosave")]);

    let format_bar_label = Label::new(Some("format bar"));
    format_bar_label.add_css_class("dim-label");
    format_bar_label.add_css_class("caption");
    format_bar_label.set_use_markup(true);
    format_bar_label.set_margin_top(3);
    format_bar_label.set_margin_bottom(3);
    set_toggle_label(&format_bar_label, "format bar", true);

    let format_bar_toggle_btn = Button::new();
    format_bar_toggle_btn.set_child(Some(&format_bar_label));
    format_bar_toggle_btn.add_css_class("flat");
    format_bar_toggle_btn.add_css_class("status-toggle");
    format_bar_toggle_btn.set_tooltip_text(Some("Toggle the formatting toolbar"));
    format_bar_toggle_btn.set_margin_end(4);
    format_bar_toggle_btn
        .update_property(&[gtk4::accessible::Property::Label("Toggle format bar")]);

    let sb_sep1 = gtk4::Separator::new(Orientation::Vertical);
    sb_sep1.add_css_class("statusbar-sep");
    sb_sep1.set_margin_start(6);
    sb_sep1.set_margin_end(6);
    sb_sep1.set_margin_top(6);
    sb_sep1.set_margin_bottom(6);

    let undo_btn = Button::from_icon_name("edit-undo-symbolic");
    undo_btn.add_css_class("flat");
    undo_btn.set_tooltip_text(Some("Undo (Ctrl+Z)"));
    undo_btn.set_sensitive(false);
    undo_btn.update_property(&[gtk4::accessible::Property::Label("Undo")]);

    let redo_btn = Button::from_icon_name("edit-redo-symbolic");
    redo_btn.add_css_class("flat");
    redo_btn.set_tooltip_text(Some("Redo (Ctrl+Shift+Z)"));
    redo_btn.set_sensitive(false);
    redo_btn.update_property(&[gtk4::accessible::Property::Label("Redo")]);

    let cursor_label = Label::new(Some("L1:C1"));
    cursor_label.add_css_class("dim-label");
    cursor_label.add_css_class("caption");
    cursor_label.set_margin_start(12);
    cursor_label.set_margin_top(3);
    cursor_label.set_margin_bottom(3);
    cursor_label.set_tooltip_text(Some("Line 1, Column 1"));

    // The one steady, quiet line: "Saved · backed up 12 min ago".
    let safe_label = Label::new(None);
    safe_label.add_css_class("dim-label");
    safe_label.add_css_class("caption");
    safe_label.set_margin_start(8);
    safe_label.set_margin_top(3);
    safe_label.set_margin_bottom(3);
    safe_label.set_halign(gtk4::Align::Start);
    let lsp_status_label = Label::new(None);
    lsp_status_label.add_css_class("dim-label");
    lsp_status_label.add_css_class("caption");
    lsp_status_label.set_use_markup(true);
    lsp_status_label.set_margin_start(8);
    // First in the bar, so it takes its natural width before anything else
    // is measured — no width needs reserving. The ceiling and ellipsis are
    // there for the rare very long LSP description.
    lsp_status_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    lsp_status_label.set_max_width_chars(86);
    lsp_status_label.set_margin_top(3);
    lsp_status_label.set_margin_bottom(3);

    let diag_label = Label::new(None);
    diag_label.add_css_class("dim-label");
    diag_label.add_css_class("caption");
    let diag_btn = Button::new();
    diag_btn.set_child(Some(&diag_label));
    diag_btn.add_css_class("flat");
    diag_btn.add_css_class("status-toggle");
    diag_btn.set_margin_start(8);
    diag_btn.set_tooltip_text(Some("Show what Zerkalo found"));
    diag_btn.set_visible(false);
    let on_diag_click: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    {
        let cb = on_diag_click.clone();
        diag_btn.connect_clicked(move |_| {
            if let Some(f) = cb.borrow().as_ref() {
                f();
            }
        });
    }

    let left_spacer = GtkBox::new(Orientation::Horizontal, 0);
    left_spacer.set_hexpand(true);

    let simple_mode_label = Label::new(None);
    simple_mode_label.add_css_class("caption");
    simple_mode_label.set_use_markup(true);
    simple_mode_label.set_margin_top(3);
    simple_mode_label.set_margin_bottom(3);

    let simple_mode_btn = Button::new();
    simple_mode_btn.set_child(Some(&simple_mode_label));
    simple_mode_btn.add_css_class("flat");
    simple_mode_btn.add_css_class("status-toggle");
    simple_mode_btn.set_tooltip_text(Some(
        "Show Template: reveals the Typst front-matter above the document body.\nEdit it via the Update Template button.",
    ));
    simple_mode_btn.update_property(&[gtk4::accessible::Property::Label(
        "Toggle template visibility",
    )]);

    let sep1 = gtk4::Separator::new(Orientation::Vertical);
    sep1.add_css_class("statusbar-sep");
    sep1.set_margin_start(6);
    sep1.set_margin_end(6);
    sep1.set_margin_top(6);
    sep1.set_margin_bottom(6);

    let section_wc_label = Label::new(None);
    section_wc_label.add_css_class("dim-label");
    section_wc_label.add_css_class("caption");
    section_wc_label.set_margin_start(8);
    section_wc_label.set_margin_end(4);
    section_wc_label.set_margin_top(3);
    section_wc_label.set_margin_bottom(3);

    let word_count_label = Label::new(Some(""));
    word_count_label.add_css_class("dim-label");
    word_count_label.add_css_class("caption");
    word_count_label.set_xalign(1.0);

    let wc_btn = Button::new();
    wc_btn.set_child(Some(&word_count_label));
    wc_btn.add_css_class("flat");
    wc_btn.set_margin_end(4);
    wc_btn.set_margin_top(1);
    wc_btn.set_margin_bottom(1);
    wc_btn.set_tooltip_text(Some("Document statistics"));

    let sep2 = gtk4::Separator::new(Orientation::Vertical);
    sep2.add_css_class("statusbar-sep");
    sep2.set_margin_start(6);
    sep2.set_margin_end(6);
    sep2.set_margin_top(6);
    sep2.set_margin_bottom(6);

    let session_delta_label = Label::new(None);
    session_delta_label.add_css_class("dim-label");
    session_delta_label.add_css_class("caption");
    session_delta_label.set_margin_end(8);
    session_delta_label.set_margin_top(3);
    session_delta_label.set_margin_bottom(3);
    session_delta_label.set_visible(false);

    let goal_fraction: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
    let goal_celebrating: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    let goal_ring = DrawingArea::new();
    goal_ring.set_visible(false);
    goal_ring.set_valign(gtk4::Align::Center);
    goal_ring.set_size_request(22, 22);
    goal_ring.set_margin_end(6);
    goal_ring.set_tooltip_text(Some("Word count progress toward goal"));
    goal_ring.add_css_class("goal-ring");
    {
        let frac_rc = goal_fraction.clone();
        let cel_rc = goal_celebrating.clone();
        let ring_widget = goal_ring.clone();
        goal_ring.set_draw_func(move |_da, cr, w, h| {
            let cx = w as f64 / 2.0;
            let cy = h as f64 / 2.0;
            let radius = (w.min(h) as f64 / 2.0) - 2.0;
            let celebrating = cel_rc.get();

            // Query theme colors on every draw so a theme/accent switch is
            // reflected immediately, matching the pattern in apply_comment_highlights.
            #[allow(deprecated)]
            let ctx = ring_widget.style_context();
            #[allow(deprecated)]
            let track = ctx
                .lookup_color("window_fg_color")
                .unwrap_or(gtk4::gdk::RGBA::new(0.5, 0.5, 0.5, 1.0));
            #[allow(deprecated)]
            let accent = ctx
                .lookup_color("accent_color")
                .unwrap_or(gtk4::gdk::RGBA::new(0.2, 0.4, 0.9, 1.0));
            #[allow(deprecated)]
            let success = ctx
                .lookup_color("success_color")
                .unwrap_or(gtk4::gdk::RGBA::new(0.2, 0.8, 0.2, 1.0));

            cr.set_line_width(if celebrating { 3.5 } else { 2.5 });
            cr.set_source_rgba(
                track.red() as f64,
                track.green() as f64,
                track.blue() as f64,
                0.2,
            );
            cr.arc(cx, cy, radius, 0.0, 2.0 * std::f64::consts::PI);
            let _ = cr.stroke();
            let frac = frac_rc.get();
            if frac > 0.0 {
                let end_angle = -std::f64::consts::FRAC_PI_2 + frac * 2.0 * std::f64::consts::PI;
                let progress = if frac >= 1.0 || celebrating {
                    &success
                } else {
                    &accent
                };
                if celebrating {
                    cr.set_line_width(3.5);
                }
                cr.set_source_rgba(
                    progress.red() as f64,
                    progress.green() as f64,
                    progress.blue() as f64,
                    0.9,
                );
                cr.arc(cx, cy, radius, -std::f64::consts::FRAC_PI_2, end_angle);
                let _ = cr.stroke();
            }
        });
    }

    let version_btn = Button::with_label(concat!("v", env!("CARGO_PKG_VERSION")));
    version_btn.add_css_class("flat");
    version_btn.add_css_class("dim-label");
    version_btn.add_css_class("caption");
    version_btn.set_margin_end(4);
    version_btn.set_tooltip_text(Some("View changelog"));

    // ── Status bar assembly ───────────────────────────────────────────────
    //
    // The completion hint leads, alone, with every standing control packed
    // to the far right behind an expanding spacer. The hint is the only
    // thing here that changes with what you're doing rather than how the
    // app is set up, and it needs room for a name, a description and its
    // keys — so it gets the whole left half of the window and the settings
    // queue up out of its way.
    status_bar.append(&safe_label);
    status_bar.append(&lsp_status_label);
    status_bar.append(&left_spacer);
    status_bar.append(&autosave_toggle_btn);
    status_bar.append(&format_bar_toggle_btn);
    status_bar.append(&search_btn);
    status_bar.append(&sb_sep1);
    status_bar.append(&cursor_label);
    status_bar.append(&diag_btn);
    status_bar.append(&sep1);
    status_bar.append(&wc_btn);
    status_bar.append(&sep2);
    status_bar.append(&session_delta_label);
    status_bar.append(&goal_ring);
    status_bar.append(&version_btn);

    let breadcrumb_label = Label::new(Some(""));
    breadcrumb_label.add_css_class("dim-label");
    breadcrumb_label.add_css_class("caption");
    breadcrumb_label.set_margin_start(12);
    breadcrumb_label.set_margin_top(3);
    breadcrumb_label.set_margin_bottom(3);
    breadcrumb_label.set_hexpand(true);
    breadcrumb_label.set_xalign(0.0);
    breadcrumb_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    let breadcrumb_bar = GtkBox::new(Orientation::Horizontal, 0);
    breadcrumb_bar.add_css_class("breadcrumb-bar");
    // Undo/redo at top-left of the editor panel
    breadcrumb_bar.append(&undo_btn);
    breadcrumb_bar.append(&redo_btn);
    let sep = Separator::new(Orientation::Vertical);
    sep.set_margin_top(6);
    sep.set_margin_bottom(6);
    sep.set_margin_start(2);
    sep.set_margin_end(2);
    breadcrumb_bar.append(&sep);
    breadcrumb_bar.append(&breadcrumb_label);
    breadcrumb_bar.append(&section_wc_label);

    StatusBar {
        status_bar,
        gost_label,
        gost_btn,
        autocorrect_label,
        autocorrect_btn,
        search_label,
        search_btn,
        focus_label,
        focus_toggle_btn,
        autosave_label,
        autosave_toggle_btn,
        format_bar_label,
        format_bar_toggle_btn,
        undo_btn,
        redo_btn,
        cursor_label,
        safe_label,
        lsp_status_label,
        diag_label,
        diag_btn,
        on_diag_click,
        simple_mode_label,
        simple_mode_btn,
        section_wc_label,
        word_count_label,
        wc_btn,
        session_delta_label,
        goal_fraction,
        goal_celebrating,
        goal_ring,
        version_btn,
        breadcrumb_label,
        breadcrumb_bar,
    }
}

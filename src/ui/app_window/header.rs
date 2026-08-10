//! Header bar construction: the title/open dropdown, the primary buttons, and
//! the hamburger popover. Split out of `AppWindow::new`.

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Entry, Label, MenuButton, Orientation, Popover,
    ScrolledWindow, Separator, ToggleButton,
};
use libadwaita as adw;

use super::{HamburgerItems, build_hamburger_menu_items};

/// The hamburger popover's rows, kept together so the menu-wiring helpers can
/// take one value instead of 22 parameters.
pub(super) struct Menus {
    pub(super) menu_about_item: Button,
    pub(super) menu_whats_new_item: Button,
    pub(super) menu_backup_remote_item: Button,
    pub(super) menu_docs_item: Button,
    pub(super) menu_export_item: Button,
    pub(super) menu_export_web_item: Button,
    pub(super) menu_fonts_item: Button,
    pub(super) menu_help_item: Button,
    pub(super) menu_shortcuts_item: Button,
    pub(super) menu_import_item: Button,
    pub(super) menu_import_pdf_item: Button,
    pub(super) menu_new_item: Button,
    pub(super) menu_new_template_item: Button,
    pub(super) menu_open_item: Button,
    pub(super) menu_print_item: Button,
    pub(super) menu_reapply_template_item: Button,
    pub(super) menu_repair_markers_item: Button,
    pub(super) menu_save_as_item: Button,
    pub(super) menu_save_item: Button,
    pub(super) menu_settings_item: Button,
    pub(super) menu_setup_item: Button,
    pub(super) menu_snapshots_item: Button,
    pub(super) menu_tools_item: Button,
    pub(super) menu_writing_stats_item: Button,
}

/// Every widget the header bar owns, handed back for the wiring that happens
/// later in `AppWindow::new`. Construction only — nothing here is connected to
/// a handler yet.
pub(super) struct HeaderWidgets {
    pub(super) menus: Menus,
    pub(super) compile_btn: Button,
    pub(super) compile_mode_slot: GtkBox,
    pub(super) draft_toggle: ToggleButton,
    pub(super) file_title_widget: adw::WindowTitle,
    pub(super) gost_menu_slot: GtkBox,
    pub(super) header: adw::HeaderBar,
    pub(super) library_btn: Button,
    pub(super) menu_popover: Popover,
    pub(super) open_list_box: GtkBox,
    pub(super) open_search: Entry,
    pub(super) preview_label: Label,
    pub(super) print_header_btn: Button,
    pub(super) recent_popover: Popover,
    pub(super) recompile_header_btn: Button,
    pub(super) save_btn: Button,
    pub(super) sidebar_btn: Button,
    pub(super) style_box: GtkBox,
    pub(super) style_btn: Button,
    pub(super) style_popover: Popover,
    pub(super) sync_btn: Button,
    pub(super) title_extras: GtkBox,
}

/// Builds the header bar, the hamburger popover and the open dropdown.
pub(super) fn build_header() -> HeaderWidgets {
    // ── Header bar ──────────────────────────────────────────────────────

    let header = adw::HeaderBar::new();
    header.add_css_class("fond-chrome");

    // Start: sidebar toggle + insert panel toggle (flat, left side)
    let sidebar_btn = Button::from_icon_name("sidebar-show-symbolic");
    sidebar_btn.set_tooltip_text(Some("Toggle sidebar"));
    sidebar_btn.add_css_class("flat");
    sidebar_btn.update_property(&[gtk4::accessible::Property::Label("Toggle sidebar")]);
    header.pack_start(&sidebar_btn);

    let library_btn = Button::with_label("Library");
    library_btn.add_css_class("flat");
    library_btn.set_tooltip_text(Some("Open document library (Ctrl+L)"));
    header.pack_start(&library_btn);

    // Style switcher dropdown — placed in header start, beside the title
    let style_names = crate::styles::STYLES.iter().map(|(n, _, _, _, _)| *n).collect::<Vec<_>>();
    let style_box = GtkBox::new(Orientation::Vertical, 0);
    style_box.set_margin_top(4);
    style_box.set_margin_bottom(4);
    let style_popover = Popover::new();
    style_popover.set_child(Some(&style_box));
    let style_btn = Button::with_label("Style");
    style_btn.add_css_class("flat");
    style_btn.add_css_class("caption");
    style_btn.set_tooltip_text(Some("Apply a formatting style to the document"));
    {
        let sp = style_popover.clone();
        let sb = style_btn.clone();
        style_btn.connect_clicked(move |_| {
            sp.set_parent(&sb);
            if sp.is_visible() { sp.popdown(); } else { sp.popup(); }
        });
    }
    for name in &style_names {
        let row = Button::new();
        row.set_label(name);
        row.set_halign(Align::Start);
        row.add_css_class("flat");
        row.set_size_request(160, -1);
        style_box.append(&row);
    }
    // Wire style buttons after editor_pane is available (done below)


    // ── Compilation profile toggle (status bar) ──────────────────────────
    let draft_label = gtk4::Label::new(Some("Final"));
    draft_label.add_css_class("caption");
    let draft_toggle = ToggleButton::new();
    draft_toggle.set_child(Some(&draft_label));
    draft_toggle.add_css_class("flat");
    draft_toggle.set_tooltip_text(Some("Toggle Draft (fast preview) / Final (full quality)"));

    // ── Primary header buttons (packed together at end of section) ────────
    let preview_label = Label::new(Some("Preview"));
    preview_label.set_use_markup(true);
    let compile_btn = Button::new();
    compile_btn.set_child(Some(&preview_label));
    compile_btn.set_tooltip_text(Some("Toggle Preview (Ctrl+Shift+P)"));
    compile_btn.add_css_class("flat");

    let recompile_header_btn = Button::from_icon_name("view-refresh-symbolic");
    recompile_header_btn.set_tooltip_text(Some("Compile now (Ctrl+Shift+P)"));
    recompile_header_btn.add_css_class("flat");
    recompile_header_btn.update_property(&[gtk4::accessible::Property::Label("Compile now")]);

    let sync_btn = Button::from_icon_name("vcs-push-symbolic");
    sync_btn.set_tooltip_text(Some("Commit & Push to Git (Ctrl+Shift+G)"));
    sync_btn.add_css_class("flat");
    sync_btn.update_property(&[gtk4::accessible::Property::Label("Commit and push to Git")]);

    let save_btn = Button::from_icon_name("document-save-symbolic");
    save_btn.set_tooltip_text(Some("Save (Ctrl+S)"));
    save_btn.add_css_class("flat");
    save_btn.update_property(&[gtk4::accessible::Property::Label("Save the current document")]);

    // Connected further down, alongside the hamburger's Print item — the
    // panes it needs don't exist yet at this point.
    let print_header_btn = Button::from_icon_name("printer-symbolic");
    print_header_btn.set_tooltip_text(Some("Print (Ctrl+P)"));
    print_header_btn.add_css_class("flat");
    print_header_btn.update_property(&[gtk4::accessible::Property::Label("Print the document")]);

    // ── Hamburger menu items (using make_menu_item for left+shortcut layout) ──
    let HamburgerItems {
        menu_about_item,
        menu_backup_remote_item,
        menu_docs_item,
        menu_export_item,
        menu_export_web_item,
        menu_fonts_item,
        menu_help_item,
        menu_shortcuts_item,
        menu_whats_new_item,
        menu_import_item,
        menu_import_pdf_item,
        menu_new_item,
        menu_new_template_item,
        menu_open_item,
        menu_print_item,
        menu_reapply_template_item,
        menu_repair_markers_item,
        menu_save_as_item,
        menu_save_item,
        menu_settings_item,
        menu_setup_item,
        menu_snapshots_item,
        menu_tools_item,
        menu_writing_stats_item,
    } = build_hamburger_menu_items();

    // ── Popover layout ────────────────────────────────────────────────────
    let menu_popover_box = GtkBox::new(Orientation::Vertical, 0);
    menu_popover_box.set_margin_top(4);
    menu_popover_box.set_margin_bottom(4);
    menu_popover_box.set_width_request(260);

    // Get a document in: creating, opening, and bringing one in from another
    // format all answer the same question, so Import sits here rather than
    // down among the export actions where it used to be.
    menu_popover_box.append(&menu_new_template_item);
    menu_popover_box.append(&menu_new_item);
    menu_popover_box.append(&Separator::new(Orientation::Horizontal));
    menu_popover_box.append(&menu_open_item);
    menu_popover_box.append(&menu_docs_item);
    menu_popover_box.append(&menu_import_item);
    menu_popover_box.append(&Separator::new(Orientation::Horizontal));
    // Current document
    menu_popover_box.append(&menu_reapply_template_item);
    menu_popover_box.append(&menu_repair_markers_item);
    menu_popover_box.append(&Separator::new(Orientation::Horizontal));
    // Save / version
    menu_popover_box.append(&menu_save_item);
    menu_popover_box.append(&menu_save_as_item);
    menu_popover_box.append(&menu_snapshots_item);
    menu_popover_box.append(&Separator::new(Orientation::Horizontal));
    // Get a document out
    menu_popover_box.append(&menu_export_item);
    menu_popover_box.append(&menu_export_web_item);
    menu_popover_box.append(&menu_print_item);
    menu_popover_box.append(&Separator::new(Orientation::Horizontal));
    // Writing/session info — a report, not a setting, so it no longer sits in
    // the app-settings block below.
    menu_popover_box.append(&menu_writing_stats_item);
    menu_popover_box.append(&Separator::new(Orientation::Horizontal));
    // App settings. The two toggles filled in from editor_pane are fenced off
    // by separators so their bold-when-on styling reads as a group rather than
    // as odd rows among the dialog-opening ones.
    // Two different font surfaces used to sit here reading as competitors, so
    // each now says which fonts it is about.
    menu_settings_item.set_tooltip_text(Some(
        "App preferences — including the font the editor text is displayed in",
    ));
    menu_fonts_item.set_tooltip_text(Some(
        "Fonts used in the compiled document, not in the editor",
    ));
    menu_popover_box.append(&menu_settings_item);
    menu_popover_box.append(&menu_fonts_item);
    menu_popover_box.append(&Separator::new(Orientation::Horizontal));
    // Filled once editor_pane exists — it owns the buttons and their state.
    let gost_menu_slot = GtkBox::new(Orientation::Vertical, 0);
    menu_popover_box.append(&gost_menu_slot);
    menu_popover_box.append(&Separator::new(Orientation::Horizontal));
    menu_popover_box.append(&menu_setup_item);
    menu_popover_box.append(&menu_backup_remote_item);
    menu_popover_box.append(&menu_tools_item);
    menu_popover_box.append(&Separator::new(Orientation::Horizontal));
    menu_popover_box.append(&menu_help_item);
    menu_popover_box.append(&menu_shortcuts_item);
    menu_popover_box.append(&menu_whats_new_item);
    menu_popover_box.append(&menu_about_item);

    let menu_popover = Popover::new();
    menu_popover.set_child(Some(&menu_popover_box));
    let menu_btn = MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.add_css_class("flat");
    menu_btn.set_popover(Some(&menu_popover));

    // Header end section layout (left → right):
    //   sync | save | todo | print | ⟳ compile now | compile mode | Preview | ≡
    // In GTK4 pack_end the last-packed widget is leftmost in the end section.
    // `compile_mode_slot` is packed empty here and filled further down, once
    // the config-backed compile-mode button exists — packing it late would
    // otherwise land it at the far left of the section, away from the
    // compile buttons it belongs with.
    let compile_mode_slot = GtkBox::new(Orientation::Horizontal, 0);
    header.pack_end(&menu_btn);
    header.pack_end(&compile_btn);
    header.pack_end(&compile_mode_slot);
    header.pack_end(&recompile_header_btn);
    header.pack_end(&print_header_btn);
    header.pack_end(&save_btn);
    header.pack_end(&sync_btn);

    // ── Setzer-style open dropdown ───────────────────────────────────────
    let open_search = Entry::new();
    open_search.set_placeholder_text(Some("Search documents…"));
    open_search.set_hexpand(true);
    open_search.set_margin_start(8);
    open_search.set_margin_end(8);
    open_search.set_margin_top(8);
    open_search.set_margin_bottom(4);

    let open_list_box = GtkBox::new(Orientation::Vertical, 0);

    let open_scroll = ScrolledWindow::new();
    open_scroll.set_child(Some(&open_list_box));
    open_scroll.set_min_content_height(80);
    open_scroll.set_max_content_height(360);
    open_scroll.set_propagate_natural_height(true);
    open_scroll.set_margin_start(4);
    open_scroll.set_margin_end(4);
    open_scroll.set_margin_bottom(4);

    let open_popover_box = GtkBox::new(Orientation::Vertical, 0);
    open_popover_box.set_width_request(280);
    open_popover_box.append(&open_search);
    open_popover_box.append(&open_scroll);

    let recent_popover = Popover::new();
    recent_popover.set_child(Some(&open_popover_box));

    let file_title_widget = adw::WindowTitle::new("untitled", "");

    let file_selector = MenuButton::new();
    file_selector.add_css_class("flat");
    file_selector.set_child(Some(&file_title_widget));
    file_selector.set_popover(Some(&recent_popover));

    // Root-file controls sit immediately right of the document title, where
    // they read as being about *this* document. Filled in further down.
    let title_extras = GtkBox::new(Orientation::Horizontal, 4);
    let title_box = GtkBox::new(Orientation::Horizontal, 6);
    title_box.append(&file_selector);
    title_box.append(&title_extras);
    header.set_title_widget(Some(&title_box));


    HeaderWidgets {
        menus: Menus {
            menu_about_item,
            menu_backup_remote_item,
            menu_docs_item,
            menu_export_item,
            menu_export_web_item,
            menu_fonts_item,
            menu_help_item,
            menu_shortcuts_item,
            menu_whats_new_item,
            menu_import_item,
            menu_import_pdf_item,
            menu_new_item,
            menu_new_template_item,
            menu_open_item,
            menu_print_item,
            menu_reapply_template_item,
            menu_repair_markers_item,
            menu_save_as_item,
            menu_save_item,
            menu_settings_item,
            menu_setup_item,
            menu_snapshots_item,
            menu_tools_item,
            menu_writing_stats_item,
        },
        compile_btn,
        compile_mode_slot,
        draft_toggle,
        file_title_widget,
        gost_menu_slot,
        header,
        library_btn,
        menu_popover,
        open_list_box,
        open_search,
        preview_label,
        print_header_btn,
        recent_popover,
        recompile_header_btn,
        save_btn,
        sidebar_btn,
        style_box,
        style_btn,
        style_popover,
        sync_btn,
        title_extras,
    }
}

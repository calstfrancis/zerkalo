//! The formatting toolbar: bold/italic, headings, page break, table picker, figure, CV style switcher, font and size menus and the overflow menu.

use super::*;

pub(super) struct FormatBar {
    pub(super) bold_btn: Button,
    pub(super) italic_btn: Button,
    pub(super) h1_btn: Button,
    pub(super) h2_btn: Button,
    pub(super) h3_btn: Button,
    pub(super) pb_btn: Button,
    pub(super) hr_btn: Button,
    pub(super) line_numbers_btn: ToggleButton,
    pub(super) table_popover: Popover,
    pub(super) selected_rows: Rc<std::cell::Cell<i32>>,
    pub(super) selected_cols: Rc<std::cell::Cell<i32>>,
    pub(super) grid_btns: Vec<Vec<Button>>,
    pub(super) table_rows_entry: Entry,
    pub(super) table_cols_entry: Entry,
    pub(super) table_custom_insert_btn: Button,
    pub(super) figure_btn: Button,
    pub(super) cv_style_label: Label,
    pub(super) cv_style_popover: Popover,
    pub(super) cv_style_popover_box: GtkBox,
    pub(super) cv_format_section: GtkBox,
    pub(super) font_popover: Popover,
    pub(super) font_buttons: Vec<(String, Button)>,
    pub(super) font_bar_label: Label,
    pub(super) size_popover: Popover,
    pub(super) size_buttons: Vec<(String, Button)>,
    pub(super) size_bar_label: Label,
    pub(super) format_bar_container: GtkBox,
}

pub(super) fn build_format_bar() -> FormatBar {
    // ── Formatting toolbar ────────────────────────────────────────────────
    let format_bar = GtkBox::new(Orientation::Horizontal, 0);
    format_bar.add_css_class("format-bar");
    format_bar.set_hexpand(true);
    format_bar.set_margin_start(4);
    format_bar.set_margin_end(4);
    format_bar.set_margin_top(1);
    format_bar.set_margin_bottom(1);

    let bold_btn = Button::from_icon_name("format-text-bold-symbolic");
    bold_btn.add_css_class("flat");
    bold_btn.set_tooltip_text(Some("Bold — wraps selection in *…*  (Ctrl+B)"));
    bold_btn.update_property(&[gtk4::accessible::Property::Label("Bold")]);

    let italic_btn = Button::from_icon_name("format-text-italic-symbolic");
    italic_btn.add_css_class("flat");
    italic_btn.set_tooltip_text(Some("Italic — wraps selection in _…_  (Ctrl+I)"));
    italic_btn.update_property(&[gtk4::accessible::Property::Label("Italic")]);

    format_bar.append(&bold_btn);
    format_bar.append(&italic_btn);

    let fb_sep1 = Separator::new(Orientation::Vertical);
    fb_sep1.set_margin_top(6);
    fb_sep1.set_margin_bottom(6);
    fb_sep1.set_margin_start(4);
    fb_sep1.set_margin_end(4);
    format_bar.append(&fb_sep1);

    let h1_btn = Button::with_label("H1");
    h1_btn.add_css_class("flat");
    h1_btn.add_css_class("caption");
    h1_btn.set_tooltip_text(Some("Heading 1  (= Heading text)"));
    h1_btn.update_property(&[gtk4::accessible::Property::Label("Heading 1")]);
    let h2_btn = Button::with_label("H2");
    h2_btn.add_css_class("flat");
    h2_btn.add_css_class("caption");
    h2_btn.set_tooltip_text(Some("Heading 2  (== Heading text)"));
    h2_btn.update_property(&[gtk4::accessible::Property::Label("Heading 2")]);
    let h3_btn = Button::with_label("H3");
    h3_btn.add_css_class("flat");
    h3_btn.add_css_class("caption");
    h3_btn.set_tooltip_text(Some("Heading 3  (=== Heading text)"));
    h3_btn.update_property(&[gtk4::accessible::Property::Label("Heading 3")]);

    format_bar.append(&h1_btn);
    format_bar.append(&h2_btn);
    format_bar.append(&h3_btn);

    let fb_sep2 = Separator::new(Orientation::Vertical);
    fb_sep2.set_margin_top(6);
    fb_sep2.set_margin_bottom(6);
    fb_sep2.set_margin_start(4);
    fb_sep2.set_margin_end(4);
    format_bar.append(&fb_sep2);

    let pb_btn = Button::with_label("¶");
    pb_btn.add_css_class("flat");
    pb_btn.add_css_class("caption");
    pb_btn.set_tooltip_text(Some("Insert page break  (#pagebreak())"));
    pb_btn.update_property(&[gtk4::accessible::Property::Label("Insert page break")]);
    format_bar.append(&pb_btn);

    let hr_btn = Button::with_label("―");
    hr_btn.add_css_class("flat");
    hr_btn.add_css_class("caption");
    hr_btn.set_tooltip_text(Some("Insert horizontal rule  (#line(length: 100%))"));
    hr_btn.update_property(&[gtk4::accessible::Property::Label("Insert horizontal rule")]);
    format_bar.append(&hr_btn);

    let fb_sep3 = Separator::new(Orientation::Vertical);
    fb_sep3.set_margin_top(6);
    fb_sep3.set_margin_bottom(6);
    fb_sep3.set_margin_start(4);
    fb_sep3.set_margin_end(4);
    format_bar.append(&fb_sep3);

    let line_numbers_btn = ToggleButton::with_label("#");
    line_numbers_btn.add_css_class("flat");
    line_numbers_btn.add_css_class("caption");
    line_numbers_btn.set_tooltip_text(Some("Toggle line numbers"));
    line_numbers_btn.update_property(&[gtk4::accessible::Property::Label("Toggle line numbers")]);
    format_bar.append(&line_numbers_btn);

    let fb_sep3b = Separator::new(Orientation::Vertical);
    fb_sep3b.set_margin_top(6);
    fb_sep3b.set_margin_bottom(6);
    fb_sep3b.set_margin_start(4);
    fb_sep3b.set_margin_end(4);
    format_bar.append(&fb_sep3b);

    // ── Insert table (grid picker) ──────────────────────────────────────
    let table_popover = Popover::new();
    let table_grid_box = GtkBox::new(Orientation::Vertical, 2);
    table_grid_box.set_margin_top(6);
    table_grid_box.set_margin_bottom(6);
    table_grid_box.set_margin_start(6);
    table_grid_box.set_margin_end(6);
    let table_size_lbl = Label::new(Some("Insert table"));
    table_size_lbl.add_css_class("caption");
    table_size_lbl.add_css_class("dim-label");
    table_grid_box.append(&table_size_lbl);

    // 8×8 grid of cells; hover to highlight, click to insert
    const GRID_MAX: usize = 8;
    let selected_rows: Rc<std::cell::Cell<i32>> = Rc::new(std::cell::Cell::new(0));
    let selected_cols: Rc<std::cell::Cell<i32>> = Rc::new(std::cell::Cell::new(0));
    let mut grid_btns: Vec<Vec<Button>> = Vec::new();
    for r in 0..GRID_MAX {
        let row_box = GtkBox::new(Orientation::Horizontal, 1);
        let mut row_btns: Vec<Button> = Vec::new();
        for c in 0..GRID_MAX {
            let cell = Button::new();
            cell.set_size_request(22, 20);
            cell.add_css_class("table-grid-cell");
            cell.update_property(&[gtk4::accessible::Property::Label(&format!(
                "{}×{} table",
                r + 1,
                c + 1
            ))]);
            row_btns.push(cell.clone());
            row_box.append(&cell);
        }
        grid_btns.push(row_btns);
        table_grid_box.append(&row_box);
    }
    // Wrap grid_btns in Rc so hover handlers can update all cells
    let grid_rc: Rc<Vec<Vec<Button>>> = Rc::new(grid_btns.to_vec());
    // Wire hover handlers (separate pass so all cells are available)
    for (r, row) in grid_btns.iter().enumerate().take(GRID_MAX) {
        for (c, cell) in row.iter().enumerate().take(GRID_MAX) {
            let cell = cell.clone();
            let sr = selected_rows.clone();
            let sc = selected_cols.clone();
            let lbl = table_size_lbl.clone();
            let gc = grid_rc.clone();
            let mc = EventControllerMotion::new();
            mc.connect_enter(move |_, _, _| {
                sr.set(r as i32 + 1);
                sc.set(c as i32 + 1);
                lbl.set_text(&format!("{}×{} table", r + 1, c + 1));
                for (ri, row) in gc.iter().enumerate() {
                    for (ci, btn) in row.iter().enumerate() {
                        if ri <= r && ci <= c {
                            btn.add_css_class("table-grid-cell-selected");
                        } else {
                            btn.remove_css_class("table-grid-cell-selected");
                        }
                    }
                }
            });
            cell.add_controller(mc);
        }
    }
    // Clear highlights when pointer leaves the grid
    {
        let gc = grid_rc.clone();
        let lbl = table_size_lbl.clone();
        let mc_leave = EventControllerMotion::new();
        mc_leave.connect_leave(move |_| {
            lbl.set_text("Insert table");
            for row in gc.iter() {
                for btn in row.iter() {
                    btn.remove_css_class("table-grid-cell-selected");
                }
            }
        });
        table_grid_box.add_controller(mc_leave);
    }
    // Custom rows × cols entry below the grid
    let custom_sep = Separator::new(Orientation::Horizontal);
    custom_sep.set_margin_top(4);
    custom_sep.set_margin_bottom(2);
    table_grid_box.append(&custom_sep);

    let custom_row_box = GtkBox::new(Orientation::Horizontal, 4);
    custom_row_box.set_margin_top(2);

    let table_rows_entry = Entry::new();
    table_rows_entry.set_placeholder_text(Some("Rows"));
    table_rows_entry.set_input_purpose(gtk4::InputPurpose::Digits);
    table_rows_entry.set_width_chars(4);
    table_rows_entry.set_max_length(2);

    let table_x_lbl = Label::new(Some("×"));
    table_x_lbl.add_css_class("dim-label");

    let table_cols_entry = Entry::new();
    table_cols_entry.set_placeholder_text(Some("Cols"));
    table_cols_entry.set_input_purpose(gtk4::InputPurpose::Digits);
    table_cols_entry.set_width_chars(4);
    table_cols_entry.set_max_length(2);

    let table_custom_insert_btn = Button::with_label("Insert");
    table_custom_insert_btn.add_css_class("suggested-action");

    custom_row_box.append(&table_rows_entry);
    custom_row_box.append(&table_x_lbl);
    custom_row_box.append(&table_cols_entry);
    custom_row_box.append(&table_custom_insert_btn);
    table_grid_box.append(&custom_row_box);
    table_popover.set_child(Some(&table_grid_box));

    let table_btn = Button::new();
    table_btn.set_icon_name("x-office-spreadsheet-symbolic");
    table_btn.add_css_class("flat");
    table_btn.set_tooltip_text(Some("Insert table"));
    table_btn.update_property(&[gtk4::accessible::Property::Label("Insert table")]);
    table_popover.set_autohide(true);
    {
        let tp = table_popover.clone();
        let tb = table_btn.clone();
        table_btn.connect_clicked(move |_| {
            tp.set_parent(&tb);
            if tp.is_visible() {
                tp.popdown();
            } else {
                tp.popup();
                tp.grab_focus();
            }
        });
    }
    format_bar.append(&table_btn);

    // ── Insert figure (file dialog) ──────────────────────────────────────
    let figure_btn = Button::new();
    figure_btn.set_icon_name("insert-image-symbolic");
    figure_btn.add_css_class("flat");
    figure_btn.set_tooltip_text(Some("Insert figure / image"));
    figure_btn.update_property(&[gtk4::accessible::Property::Label("Insert figure or image")]);
    format_bar.append(&figure_btn);

    // ── CV style switcher (shown only when editing a CV) ─────────────────
    let cv_sep = Separator::new(Orientation::Vertical);
    cv_sep.set_margin_top(6);
    cv_sep.set_margin_bottom(6);
    cv_sep.set_margin_start(4);
    cv_sep.set_margin_end(4);

    let cv_style_label = Label::new(Some("Modern"));
    cv_style_label.add_css_class("dim-label");
    cv_style_label.add_css_class("caption");

    let cv_style_popover = Popover::new();
    let cv_style_popover_box = GtkBox::new(Orientation::Vertical, 2);
    cv_style_popover_box.set_margin_top(4);
    cv_style_popover_box.set_margin_bottom(4);
    cv_style_popover_box.set_margin_start(4);
    cv_style_popover_box.set_margin_end(4);
    // Descriptions mirror the CV presets in the "New from Template" gallery
    // (see TEMPLATE_PRESETS in template_dialog.rs) so switching style here
    // carries the same explanation as picking it there.
    const CV_STYLE_DESCRIPTIONS: &[(&str, &str)] = &[
        ("Modern", "Clean résumé with colour accents, compact margins"),
        ("Academic", "Traditional academic CV with ruled section headers"),
        ("Classic", "Minimal timeless résumé, clean lines, no colour"),
        ("Two-Column", "Profile summary above a sidebar (Education, Skills & Awards) beside a main Experience column"),
    ];
    for (label, desc) in CV_STYLE_DESCRIPTIONS {
        let row = Button::new();
        row.add_css_class("flat");
        row.set_halign(gtk4::Align::Start);
        row.set_size_request(240, -1);
        let row_box = GtkBox::new(Orientation::Vertical, 1);
        row_box.set_margin_top(3);
        row_box.set_margin_bottom(3);
        let name_lbl = Label::new(Some(label));
        name_lbl.set_halign(gtk4::Align::Start);
        let desc_lbl = Label::new(Some(desc));
        desc_lbl.add_css_class("dim-label");
        desc_lbl.add_css_class("caption");
        desc_lbl.set_halign(gtk4::Align::Start);
        desc_lbl.set_wrap(true);
        desc_lbl.set_max_width_chars(30);
        row_box.append(&name_lbl);
        row_box.append(&desc_lbl);
        row.set_child(Some(&row_box));
        cv_style_popover_box.append(&row);
    }
    cv_style_popover.set_child(Some(&cv_style_popover_box));
    cv_style_popover.set_autohide(true);

    let cv_style_btn = Button::new();
    cv_style_btn.set_child(Some(&cv_style_label));
    cv_style_btn.add_css_class("flat");
    cv_style_btn.set_tooltip_text(Some("Switch CV visual style"));
    cv_style_btn.set_margin_start(4);
    {
        let sp = cv_style_popover.clone();
        let sb = cv_style_btn.clone();
        cv_style_btn.connect_clicked(move |_| {
            sp.set_parent(&sb);
            if sp.is_visible() {
                sp.popdown();
            } else {
                sp.popup();
                sp.grab_focus();
            }
        });
    }

    let cv_format_section = GtkBox::new(Orientation::Horizontal, 0);
    cv_format_section.append(&cv_sep);
    cv_format_section.append(&cv_style_btn);
    cv_format_section.set_visible(false);
    format_bar.append(&cv_format_section);

    // ── Spacer ────────────────────────────────────────────────────────────
    let fb_spacer = GtkBox::new(Orientation::Horizontal, 0);
    fb_spacer.set_hexpand(true);
    format_bar.append(&fb_spacer);

    // ── Font dropdown (right-aligned) ────────────────────────────────────
    let enabled_fonts = FontManager::enabled_fonts();
    let font_popover = Popover::new();
    let font_popover_box = GtkBox::new(Orientation::Vertical, 2);
    font_popover_box.set_margin_top(4);
    font_popover_box.set_margin_bottom(4);
    font_popover_box.set_margin_start(4);
    font_popover_box.set_margin_end(4);
    let mut font_buttons: Vec<(String, Button)> = Vec::new();
    for font_name in &enabled_fonts {
        let row = Button::with_label(font_name);
        row.add_css_class("flat");
        row.set_halign(gtk4::Align::Start);
        row.set_size_request(260, -1);
        font_popover_box.append(&row);
        font_buttons.push((font_name.clone(), row));
    }
    let font_scroll = ScrolledWindow::new();
    font_scroll.set_child(Some(&font_popover_box));
    font_scroll.set_min_content_width(260);
    font_scroll.set_max_content_height(320);
    font_scroll.set_propagate_natural_height(true);
    font_scroll.set_propagate_natural_width(true);
    font_popover.set_child(Some(&font_scroll));
    font_popover.set_autohide(true);

    let font_bar_label = Label::new(Some("font"));
    font_bar_label.add_css_class("dim-label");
    font_bar_label.add_css_class("caption");
    let font_bar_btn = Button::new();
    font_bar_btn.set_child(Some(&font_bar_label));
    font_bar_btn.add_css_class("flat");
    font_bar_btn.set_tooltip_text(Some("Document body font"));
    font_bar_btn.set_margin_start(4);
    {
        let fp = font_popover.clone();
        let fb = font_bar_btn.clone();
        font_bar_btn.connect_clicked(move |_| {
            fp.set_parent(&fb);
            if fp.is_visible() {
                fp.popdown();
            } else {
                fp.popup();
                fp.grab_focus();
            }
        });
    }
    format_bar.append(&font_bar_btn);

    // ── Font size dropdown (right-aligned) ────────────────────────────────
    const DOC_SIZES: &[&str] = &[
        "10pt", "11pt", "12pt", "14pt", "16pt", "18pt", "20pt", "24pt",
    ];
    let size_popover = Popover::new();
    let size_popover_box = GtkBox::new(Orientation::Vertical, 2);
    size_popover_box.set_margin_top(4);
    size_popover_box.set_margin_bottom(4);
    size_popover_box.set_margin_start(4);
    size_popover_box.set_margin_end(4);
    let mut size_buttons: Vec<(String, Button)> = Vec::new();
    for size_name in DOC_SIZES {
        let row = Button::with_label(size_name);
        row.add_css_class("flat");
        row.add_css_class("caption");
        row.set_halign(gtk4::Align::Start);
        row.set_size_request(80, -1);
        size_popover_box.append(&row);
        size_buttons.push((size_name.to_string(), row));
    }
    size_popover.set_child(Some(&size_popover_box));
    size_popover.set_autohide(true);

    let size_bar_label = Label::new(Some("size"));
    size_bar_label.add_css_class("dim-label");
    size_bar_label.add_css_class("caption");
    let size_bar_btn = Button::new();
    size_bar_btn.set_child(Some(&size_bar_label));
    size_bar_btn.add_css_class("flat");
    size_bar_btn.set_tooltip_text(Some("Document font size"));
    size_bar_btn.set_margin_start(2);
    {
        let sp = size_popover.clone();
        let sb = size_bar_btn.clone();
        size_bar_btn.connect_clicked(move |_| {
            sp.set_parent(&sb);
            if sp.is_visible() {
                sp.popdown();
            } else {
                sp.popup();
                sp.grab_focus();
            }
        });
    }
    format_bar.append(&size_bar_btn);

    // ── Overflow menu ─────────────────────────────────────────────────────
    // The bar above can't shrink below the combined minimum width of every
    // button it holds, which used to force the editor pane to overflow
    // underneath the sidebar on narrow windows/splits. An AdwBreakpointBin
    // (which — unlike a plain GtkBox — is allowed to be allocated smaller
    // than its child's minimum size once it has breakpoints) wraps the bar
    // and, as space runs low, moves lower-priority controls out of the bar
    // and into this popover behind a trailing "more" button instead.
    let overflow_box = GtkBox::new(Orientation::Vertical, 2);
    overflow_box.set_margin_top(4);
    overflow_box.set_margin_bottom(4);
    overflow_box.set_margin_start(4);
    overflow_box.set_margin_end(4);

    let overflow_popover = Popover::new();
    overflow_popover.set_child(Some(&overflow_box));
    overflow_popover.set_autohide(true);

    let overflow_btn = Button::from_icon_name("pan-down-symbolic");
    overflow_btn.add_css_class("flat");
    overflow_btn.set_tooltip_text(Some("More formatting options"));
    overflow_btn.set_visible(false);
    overflow_btn.update_property(&[gtk4::accessible::Property::Label("More formatting options")]);
    {
        let op = overflow_popover.clone();
        let ob = overflow_btn.clone();
        overflow_btn.connect_clicked(move |_| {
            op.set_parent(&ob);
            if op.is_visible() {
                op.popdown();
            } else {
                op.popup();
                op.grab_focus();
            }
        });
    }
    format_bar.append(&overflow_btn);

    // A plain GTK Button grabs keyboard focus on click by default, which
    // moved it off the document — clicking Bold left the cursor blinking
    // in the format bar, not the text, so the next keystroke went
    // nowhere until the user clicked back into the document. Formatting
    // actions like these are meant to apply to a selection and hand
    // control straight back, not become the new focus target — so every
    // widget in the bar is marked non-focusable. This naturally stops at
    // popover boundaries (a Popover attaches via set_parent, not as a
    // child in the anchor's own child chain), so the table/figure/font
    // pickers' own keyboard navigation inside their popovers is
    // unaffected.
    fn disable_focus_recursive(widget: &impl IsA<gtk4::Widget>) {
        let widget = widget.as_ref();
        widget.set_can_focus(false);
        let mut child = widget.first_child();
        while let Some(c) = child {
            disable_focus_recursive(&c);
            child = c.next_sibling();
        }
    }
    disable_focus_recursive(&format_bar);

    struct OverflowGroup {
        lead_separator: Option<gtk4::Widget>,
        controls: Vec<gtk4::Widget>,
        zone_b: bool,
    }

    // Collapse priority, least-important-first (mirrors visual order:
    // zone B — font/size — collapses right-to-left, then zone A —
    // headings/pagebreak/line-numbers/table/figure/CV style — also
    // collapses right-to-left).
    let overflow_groups: Rc<Vec<OverflowGroup>> = Rc::new(vec![
        OverflowGroup {
            lead_separator: None,
            controls: vec![size_bar_btn.clone().upcast()],
            zone_b: true,
        },
        OverflowGroup {
            lead_separator: None,
            controls: vec![font_bar_btn.clone().upcast()],
            zone_b: true,
        },
        OverflowGroup {
            lead_separator: None,
            controls: vec![cv_format_section.clone().upcast()],
            zone_b: false,
        },
        OverflowGroup {
            lead_separator: None,
            controls: vec![figure_btn.clone().upcast()],
            zone_b: false,
        },
        OverflowGroup {
            lead_separator: Some(fb_sep3b.clone().upcast()),
            controls: vec![table_btn.clone().upcast()],
            zone_b: false,
        },
        OverflowGroup {
            lead_separator: Some(fb_sep3.clone().upcast()),
            controls: vec![line_numbers_btn.clone().upcast()],
            zone_b: false,
        },
        OverflowGroup {
            lead_separator: Some(fb_sep2.clone().upcast()),
            controls: vec![pb_btn.clone().upcast(), hr_btn.clone().upcast()],
            zone_b: false,
        },
        OverflowGroup {
            lead_separator: Some(fb_sep1.clone().upcast()),
            controls: vec![
                h1_btn.clone().upcast(),
                h2_btn.clone().upcast(),
                h3_btn.clone().upcast(),
            ],
            zone_b: false,
        },
    ]);

    // Rolling "insert after" anchors: the current rightmost visible widget
    // in each zone. Restoring a group pushes its last widget as the new
    // anchor; collapsing a group pops back to the previous one. So while
    // the bar is fully expanded each stack must already hold the whole
    // chain, bottom-to-top: the zone's fixed base widget, then the last
    // control of every group in reverse collapse order. Seeding only the
    // base left the stack empty after the first collapse, and restoring
    // then unwrapped a None and aborted the app.
    let zone_a_anchor_stack: Rc<RefCell<Vec<gtk4::Widget>>> = Rc::new(RefCell::new(vec![
        italic_btn.clone().upcast(),
        h3_btn.clone().upcast(),
        hr_btn.clone().upcast(),
        line_numbers_btn.clone().upcast(),
        table_btn.clone().upcast(),
        figure_btn.clone().upcast(),
        cv_format_section.clone().upcast(),
    ]));
    let zone_b_anchor_stack: Rc<RefCell<Vec<gtk4::Widget>>> = Rc::new(RefCell::new(vec![
        fb_spacer.clone().upcast(),
        font_bar_btn.clone().upcast(),
        size_bar_btn.clone().upcast(),
    ]));
    let zone_a_base: gtk4::Widget = italic_btn.clone().upcast();
    let zone_b_base: gtk4::Widget = fb_spacer.clone().upcast();
    let overflow_stage: Rc<Cell<usize>> = Rc::new(Cell::new(0));

    let set_overflow_stage = {
        let overflow_groups = overflow_groups.clone();
        let zone_a_anchor_stack = zone_a_anchor_stack.clone();
        let zone_b_anchor_stack = zone_b_anchor_stack.clone();
        let zone_a_base = zone_a_base.clone();
        let zone_b_base = zone_b_base.clone();
        let overflow_stage = overflow_stage.clone();
        let overflow_box = overflow_box.clone();
        let overflow_btn = overflow_btn.clone();
        let format_bar = format_bar.clone();
        move |target: usize| {
            let target = target.min(overflow_groups.len());
            let mut stage = overflow_stage.get();
            while stage < target {
                let group = &overflow_groups[stage];
                for control in &group.controls {
                    control.unparent();
                    overflow_box.append(control);
                }
                if let Some(sep) = &group.lead_separator {
                    sep.unparent();
                }
                let stack = if group.zone_b {
                    &zone_b_anchor_stack
                } else {
                    &zone_a_anchor_stack
                };
                stack.borrow_mut().pop();
                stage += 1;
            }
            while stage > target {
                stage -= 1;
                let group = &overflow_groups[stage];
                let stack = if group.zone_b {
                    &zone_b_anchor_stack
                } else {
                    &zone_a_anchor_stack
                };
                let base = if group.zone_b {
                    &zone_b_base
                } else {
                    &zone_a_base
                };
                // Never unwrap here: an unbalanced stack must degrade to a
                // slightly odd button order, not abort the process (a panic
                // in a GTK callback can't unwind and takes the app down).
                let mut anchor = stack
                    .borrow()
                    .last()
                    .cloned()
                    .unwrap_or_else(|| base.clone());
                if let Some(sep) = &group.lead_separator {
                    format_bar.insert_child_after(sep, Some(&anchor));
                    anchor = sep.clone();
                }
                for control in &group.controls {
                    control.unparent();
                    format_bar.insert_child_after(control, Some(&anchor));
                    anchor = control.clone();
                }
                stack.borrow_mut().push(anchor);
            }
            overflow_stage.set(stage);
            overflow_btn.set_visible(stage > 0);
        }
    };

    let format_bar_bin = adw::BreakpointBin::new();
    format_bar_bin.set_width_request(190);
    format_bar_bin.set_height_request(38);
    format_bar_bin.set_hexpand(true);
    format_bar_bin.set_child(Some(&format_bar));

    // Thresholds are generous on purpose (better to collapse a control
    // slightly before it's strictly necessary than to risk the bar
    // overflowing its own bin). Added widest-to-narrowest: AdwBreakpointBin
    // picks "the last added breakpoint whose condition matches", so at any
    // given width the narrowest still-matching one — i.e. the deepest
    // applicable collapse stage — always wins.
    const OVERFLOW_THRESHOLDS: &[f64] = &[760.0, 700.0, 650.0, 600.0, 550.0, 480.0, 420.0, 360.0];
    for (i, px) in OVERFLOW_THRESHOLDS.iter().enumerate() {
        let condition = adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            *px,
            adw::LengthUnit::Px,
        );
        let bp = adw::Breakpoint::new(condition);
        {
            let set_stage = set_overflow_stage.clone();
            bp.connect_apply(move |_| set_stage(i + 1));
        }
        {
            let set_stage = set_overflow_stage.clone();
            bp.connect_unapply(move |_| set_stage(i));
        }
        format_bar_bin.add_breakpoint(bp);
    }

    let format_bar_container = GtkBox::new(Orientation::Vertical, 0);
    format_bar_container.append(&format_bar_bin);
    format_bar_container.append(&Separator::new(Orientation::Horizontal));
    FormatBar {
        bold_btn,
        italic_btn,
        h1_btn,
        h2_btn,
        h3_btn,
        pb_btn,
        hr_btn,
        line_numbers_btn,
        table_popover,
        selected_rows,
        selected_cols,
        grid_btns,
        table_rows_entry,
        table_cols_entry,
        table_custom_insert_btn,
        figure_btn,
        cv_style_label,
        cv_style_popover,
        cv_style_popover_box,
        cv_format_section,
        font_popover,
        font_buttons,
        font_bar_label,
        size_popover,
        size_buttons,
        size_bar_label,
        format_bar_container,
    }
}

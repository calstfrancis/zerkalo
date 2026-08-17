//! The print sheet: what Zerkalo asks before handing off to the system print
//! dialog.
//!
//! It deliberately does *not* duplicate the system dialog. Printer, copies and
//! duplex belong to the desktop; what lives here is what only Zerkalo knows —
//! the document's own page numbering, its real paper size, and how pages should
//! be arranged onto sheets. Everything chosen here is pre-applied to the
//! portal's dialog, which still opens for the printer itself.
//!
//! It also stands in for the old "Preparing to print…" toast: it opens
//! immediately with a spinner and fills in when the compile lands, so a long
//! document shows progress and can be abandoned.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, DrawingArea, Label, Orientation, Window};
use libadwaita as adw;

use crate::config::{Config, DuplexPref, PrintPrefs};
use crate::print_layout::Imposition;
use crate::ui::print::{self, Prepared, PrintJob, PrintRequest, PrintStatus};

/// Longest edge of the thumbnail, in pixels. Big enough to tell a booklet's
/// page order at a glance, small enough to re-render on every option change
/// without the dialog feeling sticky.
const THUMB_MAX_PX: f64 = 260.0;

/// Quick starting points offered above the individual controls.
///
/// Not user-editable presets: these are the three jobs that actually recur —
/// a proof, a finished copy, and a folded booklet — and each just sets the
/// controls below, which stay free to adjust afterwards.
const PRESETS: &[(&str, Imposition, DuplexPref, bool)] = &[
    ("Proof — two pages a sheet, grayscale", Imposition::TwoUp, DuplexPref::LongEdge, false),
    ("Final — one page a sheet, two-sided", Imposition::Off, DuplexPref::LongEdge, true),
    ("Booklet — fold and staple", Imposition::Booklet, DuplexPref::ShortEdge, true),
];

pub struct PrintSheet;

impl PrintSheet {
    /// Open the sheet for `request`, compiling (or reusing) in the background.
    ///
    /// `on_save_prefs` persists the settings the user ends up printing with;
    /// `on_error` receives compile failures so they can reach the error panel,
    /// which knows how to parse Typst diagnostics.
    pub fn open(
        parent: &adw::ApplicationWindow,
        request: PrintRequest,
        config: &Config,
        on_save_prefs: impl Fn(PrintPrefs) + 'static,
        on_error: impl Fn(String) + 'static,
        on_status: impl Fn(PrintStatus) + 'static,
    ) {
        let window = adw::Window::new();
        window.set_title(Some("Print"));
        window.set_default_width(460);
        window.set_transient_for(Some(parent));
        window.set_modal(true);
        window.set_resizable(false);

        let content = GtkBox::new(Orientation::Vertical, 0);
        let header = adw::HeaderBar::new();
        header.add_css_class("fond-chrome");

        // ── Document summary ─────────────────────────────────────────────────
        let doc_label = Label::new(Some(&request.job_name));
        doc_label.add_css_class("heading");
        doc_label.set_xalign(0.0);
        doc_label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);

        let detail_label = Label::new(Some("Preparing…"));
        detail_label.add_css_class("caption");
        detail_label.add_css_class("dim-label");
        detail_label.set_xalign(0.0);
        detail_label.set_wrap(true);

        let summary = GtkBox::new(Orientation::Vertical, 2);
        summary.set_margin_start(16);
        summary.set_margin_end(16);
        summary.set_margin_top(12);
        summary.append(&doc_label);
        summary.append(&detail_label);
        content.append(&summary);

        // ── Preview ──────────────────────────────────────────────────────────
        let spinner = gtk4::Spinner::new();
        spinner.set_size_request(32, 32);
        spinner.set_halign(Align::Center);
        spinner.set_valign(Align::Center);
        spinner.start();

        let thumb = DrawingArea::new();
        thumb.set_content_width(THUMB_MAX_PX as i32);
        thumb.set_content_height(THUMB_MAX_PX as i32);
        thumb.set_halign(Align::Center);
        thumb.set_visible(false);

        let preview_area = GtkBox::new(Orientation::Vertical, 0);
        preview_area.set_size_request(-1, THUMB_MAX_PX as i32 + 16);
        preview_area.set_valign(Align::Center);
        preview_area.set_margin_top(12);
        preview_area.set_margin_bottom(4);
        preview_area.append(&spinner);
        preview_area.append(&thumb);
        content.append(&preview_area);

        let sheet_label = Label::new(None);
        sheet_label.add_css_class("caption");
        sheet_label.add_css_class("dim-label");
        sheet_label.set_margin_bottom(8);
        content.append(&sheet_label);

        // ── Options ──────────────────────────────────────────────────────────
        let group = adw::PreferencesGroup::new();
        group.set_margin_start(16);
        group.set_margin_end(16);
        group.set_margin_bottom(8);

        let preset_row = adw::ComboRow::new();
        preset_row.set_title("Start from");
        let preset_names: Vec<&str> =
            std::iter::once("Custom").chain(PRESETS.iter().map(|(n, ..)| *n)).collect();
        preset_row.set_model(Some(&gtk4::StringList::new(&preset_names)));
        group.add(&preset_row);

        let range_row = adw::EntryRow::new();
        range_row.set_title("Pages");
        range_row.set_show_apply_button(false);
        group.add(&range_row);

        let layout_row = adw::ComboRow::new();
        layout_row.set_title("Layout");
        let layout_names: Vec<&str> = Imposition::ALL.iter().map(|i| i.label()).collect();
        layout_row.set_model(Some(&gtk4::StringList::new(&layout_names)));
        let stored = Imposition::from_config_key(&config.print.imposition);
        layout_row.set_selected(
            Imposition::ALL.iter().position(|i| *i == stored).unwrap_or(0) as u32
        );
        group.add(&layout_row);

        let copies_row = adw::SpinRow::with_range(1.0, 99.0, 1.0);
        copies_row.set_title("Copies");
        copies_row.set_value(config.print.copies.max(1) as f64);
        group.add(&copies_row);

        let duplex_row = adw::ComboRow::new();
        duplex_row.set_title("Two-sided");
        duplex_row.set_model(Some(&gtk4::StringList::new(&[
            "Printer default",
            "One-sided",
            "Flip on long edge",
            "Flip on short edge",
        ])));
        duplex_row.set_selected(duplex_index(config.print.duplex));
        group.add(&duplex_row);

        let color_row = adw::SwitchRow::new();
        color_row.set_title("Colour");
        color_row.set_active(config.print.color);
        group.add(&color_row);

        content.append(&group);

        // ── Buttons ──────────────────────────────────────────────────────────
        let cancel_btn = Button::with_label("Cancel");
        let print_btn = Button::with_label("Print…");
        print_btn.add_css_class("suggested-action");
        print_btn.set_sensitive(false);

        let btn_row = GtkBox::new(Orientation::Horizontal, 8);
        btn_row.set_halign(Align::End);
        btn_row.set_margin_start(16);
        btn_row.set_margin_end(16);
        btn_row.set_margin_bottom(16);
        btn_row.append(&cancel_btn);
        btn_row.append(&print_btn);
        content.append(&btn_row);

        let toolbar = adw::ToolbarView::new();
        toolbar.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));
        window.set_content(Some(&toolbar));

        // ── State ────────────────────────────────────────────────────────────
        let prepared: Rc<RefCell<Option<Rc<Prepared>>>> = Rc::new(RefCell::new(None));
        // Physical page indices the current range resolves to. Recomputed
        // whenever the range or the document changes; the thumbnail and the
        // print button both read it rather than re-parsing.
        let selection: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));

        // Re-derives everything downstream of the options: the resolved page
        // selection, the sheet description, and the thumbnail. One function so
        // the three can't drift apart as controls are added.
        let refresh: Rc<dyn Fn()> = {
            let prepared = prepared.clone();
            let selection = selection.clone();
            let range_row = range_row.clone();
            let layout_row = layout_row.clone();
            let detail_label = detail_label.clone();
            let sheet_label = sheet_label.clone();
            let thumb = thumb.clone();
            let print_btn = print_btn.clone();
            Rc::new(move || {
                let Some(doc) = prepared.borrow().clone() else { return };
                let imposition = Imposition::ALL[layout_row.selected() as usize];

                match doc.numbering.resolve(&range_row.text()) {
                    Ok(pages) => {
                        range_row.remove_css_class("error");
                        let sheets = imposition.arrange(&pages).len();
                        sheet_label.set_text(&describe_sheets(pages.len(), sheets, imposition));
                        *selection.borrow_mut() = pages;
                        print_btn.set_sensitive(true);
                        detail_label.remove_css_class("error");
                    }
                    Err(msg) => {
                        range_row.add_css_class("error");
                        sheet_label.set_text(&msg);
                        selection.borrow_mut().clear();
                        print_btn.set_sensitive(false);
                    }
                }
                thumb.queue_draw();
            })
        };

        // ── Thumbnail drawing ────────────────────────────────────────────────
        {
            let prepared = prepared.clone();
            let selection = selection.clone();
            let layout_row = layout_row.clone();
            thumb.set_draw_func(move |area, cr, w, h| {
                let Some(doc) = prepared.borrow().clone() else { return };
                let pages = selection.borrow();
                if pages.is_empty() {
                    return;
                }
                let imposition = Imposition::ALL[layout_row.selected() as usize];
                let sides = imposition.arrange(&pages);
                let Some(first) = sides.first() else { return };
                draw_sheet_preview(area, cr, w, h, &doc, first, imposition);
            });
        }

        // ── Wiring ───────────────────────────────────────────────────────────
        {
            let refresh = refresh.clone();
            range_row.connect_changed(move |_| refresh());
        }
        {
            let refresh = refresh.clone();
            layout_row.connect_selected_notify(move |_| refresh());
        }

        // Presets set the controls and then step out of the way — selecting one
        // and adjusting a single row afterwards must not silently re-apply it.
        {
            let layout_row = layout_row.clone();
            let duplex_row = duplex_row.clone();
            let color_row = color_row.clone();
            preset_row.connect_selected_notify(move |row| {
                let index = row.selected();
                if index == 0 {
                    return;
                }
                let Some((_, imposition, duplex, color)) = PRESETS.get(index as usize - 1) else {
                    return;
                };
                layout_row.set_selected(
                    Imposition::ALL.iter().position(|i| i == imposition).unwrap_or(0) as u32,
                );
                duplex_row.set_selected(duplex_index(*duplex));
                color_row.set_active(*color);
            });
        }

        // ── Preparation ──────────────────────────────────────────────────────
        let preparation = Rc::new(RefCell::new(None));
        {
            let prepared = prepared.clone();
            let refresh = refresh.clone();
            let detail_label = detail_label.clone();
            let spinner = spinner.clone();
            let thumb = thumb.clone();
            let window = window.clone();
            let handle = print::prepare(&request, move |result| match result {
                Ok(doc) => {
                    spinner.stop();
                    spinner.set_visible(false);
                    thumb.set_visible(true);
                    detail_label.set_text(&describe_document(&doc));
                    if !doc.paper.uniform {
                        // Only the first page's size reaches the printer, so
                        // say so rather than cropping the rest in silence.
                        detail_label.set_text(&format!(
                            "{}\nThis document mixes page sizes — the printer will use the \
                             first page's size for all of them.",
                            describe_document(&doc)
                        ));
                    }
                    *prepared.borrow_mut() = Some(doc);
                    refresh();
                }
                Err(msg) => {
                    // The error panel parses Typst diagnostics far better than
                    // a label can, so hand the failure over and get out of the
                    // way rather than showing a wall of text in a dialog.
                    window.close();
                    on_error(msg);
                }
            });
            *preparation.borrow_mut() = Some(handle);
        }

        {
            let window = window.clone();
            let preparation = preparation.clone();
            cancel_btn.connect_clicked(move |_| {
                if let Some(handle) = preparation.borrow().as_ref() {
                    handle.cancel();
                }
                window.close();
            });
        }

        {
            let window = window.clone();
            let prepared = prepared.clone();
            let selection = selection.clone();
            let parent = parent.clone();
            let job_name = request.job_name.clone();
            let layout_row = layout_row.clone();
            let copies_row = copies_row.clone();
            let duplex_row = duplex_row.clone();
            let color_row = color_row.clone();
            let collate = config.print.collate;
            let on_status = Rc::new(on_status);
            let on_save_prefs = Rc::new(on_save_prefs);
            print_btn.connect_clicked(move |_| {
                let Some(doc) = prepared.borrow().clone() else { return };
                let pages = selection.borrow().clone();
                if pages.is_empty() {
                    return;
                }
                let imposition = Imposition::ALL[layout_row.selected() as usize];
                let prefs = PrintPrefs {
                    imposition: imposition.config_key().to_string(),
                    copies: copies_row.value() as u32,
                    duplex: duplex_from_index(duplex_row.selected()),
                    color: color_row.is_active(),
                    collate,
                };
                on_save_prefs(prefs.clone());

                let job = PrintJob { job_name: job_name.clone(), pages, imposition, prefs };
                let on_status = on_status.clone();
                print::send_to_printer(parent.clone().upcast_ref::<Window>(), &doc, job, move |s| {
                    on_status(s)
                });
                window.close();
            });
        }

        window.present();
    }
}

fn duplex_index(pref: DuplexPref) -> u32 {
    match pref {
        DuplexPref::Printer => 0,
        DuplexPref::OneSided => 1,
        DuplexPref::LongEdge => 2,
        DuplexPref::ShortEdge => 3,
    }
}

fn duplex_from_index(index: u32) -> DuplexPref {
    match index {
        1 => DuplexPref::OneSided,
        2 => DuplexPref::LongEdge,
        3 => DuplexPref::ShortEdge,
        _ => DuplexPref::Printer,
    }
}

fn describe_document(doc: &Prepared) -> String {
    let pages = doc.numbering.len();
    let plural = if pages == 1 { "page" } else { "pages" };
    let mut text = format!("{pages} {plural} · {}", doc.paper.describe());
    if !doc.numbering.matches_physical_order() {
        // Worth saying only when it's true: the range field means something
        // different in a document whose printed numbers restart.
        text.push_str(" · pages are numbered by the document, not by position");
    }
    text
}

fn describe_sheets(pages: usize, sides: usize, imposition: Imposition) -> String {
    let page_word = if pages == 1 { "page" } else { "pages" };
    match imposition {
        Imposition::Off => {
            let sheet_word = if sides == 1 { "sheet" } else { "sheets" };
            format!("{pages} {page_word} on {sides} {sheet_word}")
        }
        Imposition::Booklet => {
            let leaves = sides / 2;
            let leaf_word = if leaves == 1 { "folded sheet" } else { "folded sheets" };
            format!("{pages} {page_word} on {leaves} {leaf_word}, printed both sides")
        }
        _ => format!("{pages} {page_word} on {sides} sheet sides"),
    }
}

/// Draw the first sheet as it will be imposed.
///
/// Renders the real pages rather than placeholder boxes — for a booklet the
/// whole point is seeing that the first sheet carries the last page next to the
/// first, which a diagram of empty rectangles wouldn't confirm.
fn draw_sheet_preview(
    area: &DrawingArea,
    cr: &gtk4::cairo::Context,
    width: i32,
    height: i32,
    doc: &Prepared,
    side: &[Option<usize>],
    imposition: Imposition,
) {
    let (sheet_w_pt, sheet_h_pt) = if imposition.rotates_sheet() {
        (doc.paper.height_pt, doc.paper.width_pt)
    } else {
        (doc.paper.width_pt, doc.paper.height_pt)
    };
    if sheet_w_pt <= 0.0 || sheet_h_pt <= 0.0 {
        return;
    }

    let scale = (width as f64 / sheet_w_pt).min(height as f64 / sheet_h_pt);
    let sheet_w = sheet_w_pt * scale;
    let sheet_h = sheet_h_pt * scale;
    let origin_x = (width as f64 - sheet_w) / 2.0;
    let origin_y = (height as f64 - sheet_h) / 2.0;

    // Colours are re-queried at draw time so the thumbnail follows a theme or
    // accent change without the dialog being reopened.
    let (fg_r, fg_g, fg_b) = crate::ui::theme::rgb(area, "window_fg_color").unwrap_or((0.2, 0.2, 0.2));

    cr.save().ok();
    cr.rectangle(origin_x, origin_y, sheet_w, sheet_h);
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.fill_preserve().ok();
    cr.set_source_rgba(fg_r, fg_g, fg_b, 0.35);
    cr.set_line_width(1.0);
    cr.stroke().ok();
    cr.restore().ok();

    let (cols, rows) = imposition.grid();
    let slot_w = sheet_w / cols as f64;
    let slot_h = sheet_h / rows as f64;

    for (slot, entry) in side.iter().enumerate() {
        let col = slot % cols;
        let row = slot / cols;
        let slot_x = origin_x + col as f64 * slot_w;
        let slot_y = origin_y + row as f64 * slot_h;

        let Some(index) = entry else {
            continue;
        };
        let Some(page) = doc.doc.pages.get(*index) else { continue };

        let page_size = page.frame.size();
        let (pw, ph) = (page_size.x.to_pt(), page_size.y.to_pt());
        if pw <= 0.0 || ph <= 0.0 {
            continue;
        }
        let fit = (slot_w / (pw * scale)).min(slot_h / (ph * scale)) * scale;
        let draw_w = pw * fit;
        let draw_h = ph * fit;
        let x = slot_x + (slot_w - draw_w) / 2.0;
        let y = slot_y + (slot_h - draw_h) / 2.0;

        // Rendered at the size it is shown at, not the page's own resolution —
        // this runs on every option change and a full-resolution render would
        // make the dialog stutter.
        let rendered = crate::compiler::render_page_rgba(page, fit as f32);
        if rendered.width == 0 || rendered.height == 0 {
            continue;
        }
        let pixbuf = gtk4::gdk_pixbuf::Pixbuf::from_bytes(
            &glib::Bytes::from_owned(rendered.rgba),
            gtk4::gdk_pixbuf::Colorspace::Rgb,
            true,
            8,
            rendered.width as i32,
            rendered.height as i32,
            (rendered.width * 4) as i32,
        );
        cr.save().ok();
        cr.rectangle(x, y, draw_w, draw_h);
        cr.clip();
        cr.set_source_pixbuf(&pixbuf, x, y);
        cr.paint().ok();
        cr.restore().ok();
    }

    // The fold, drawn last so it sits over the pages.
    if imposition == Imposition::Booklet {
        cr.save().ok();
        cr.set_source_rgba(fg_r, fg_g, fg_b, 0.4);
        cr.set_line_width(1.0);
        cr.set_dash(&[3.0, 3.0], 0.0);
        cr.move_to(origin_x + sheet_w / 2.0, origin_y);
        cr.line_to(origin_x + sheet_w / 2.0, origin_y + sheet_h);
        cr.stroke().ok();
        cr.restore().ok();
    }
}

/// Assemble the print request for the document currently on screen.
///
/// Shared by every entry point so none of them can assemble its own inputs and
/// drift — printing once omitted the CV sys inputs that way and produced
/// nothing at all for a CV document.
pub fn request_for(
    preview: &crate::ui::preview_pane::PreviewPane,
) -> Option<PrintRequest> {
    let (root, overrides, sys_inputs) = preview.compile_inputs()?;
    let job_name = root
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();
    Some(PrintRequest { root, overrides, sys_inputs, job_name })
}

/// Path of the document a request refers to, for callers that want to log it.
#[allow(dead_code)]
pub fn request_root(request: &PrintRequest) -> PathBuf {
    request.root.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplex_indices_round_trip() {
        for pref in [
            DuplexPref::Printer,
            DuplexPref::OneSided,
            DuplexPref::LongEdge,
            DuplexPref::ShortEdge,
        ] {
            assert_eq!(duplex_from_index(duplex_index(pref)), pref);
        }
    }

    #[test]
    fn an_out_of_range_duplex_index_falls_back_to_the_printer_default() {
        assert_eq!(duplex_from_index(99), DuplexPref::Printer);
    }

    #[test]
    fn sheet_descriptions_count_booklet_leaves_not_sides() {
        // A booklet's sides are printed two to a sheet, so reporting sides
        // would tell the user to load twice the paper they need.
        assert_eq!(
            describe_sheets(8, 4, Imposition::Booklet),
            "8 pages on 2 folded sheets, printed both sides"
        );
        assert_eq!(
            describe_sheets(4, 2, Imposition::Booklet),
            "4 pages on 1 folded sheet, printed both sides"
        );
    }

    #[test]
    fn sheet_descriptions_singularise() {
        assert_eq!(describe_sheets(1, 1, Imposition::Off), "1 page on 1 sheet");
    }

    #[test]
    fn every_preset_names_a_layout_the_dialog_offers() {
        // The preset applies by index into Imposition::ALL; a layout missing
        // from it would silently select the wrong row.
        for (name, imposition, ..) in PRESETS {
            assert!(
                Imposition::ALL.contains(imposition),
                "preset “{name}” names a layout the dialog doesn't list"
            );
        }
    }
}

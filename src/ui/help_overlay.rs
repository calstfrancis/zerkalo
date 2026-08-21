//! "What is this?" — labelled bubbles drawn over the running window.
//!
//! Deliberately not a dimmed tutorial: the program stays fully visible
//! underneath, because the point is to explain the thing you are looking at
//! while you look at it. Each bubble is tied to a real widget and follows it,
//! so nothing goes stale when panels are hidden or the window is resized —
//! anything not on screen simply isn't labelled.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, DrawingArea, Fixed, Label, Orientation};

/// Bubble width. Wide enough for a sentence at a comfortable measure, narrow
/// enough that several fit beside each other on a laptop screen.
const BUBBLE_W: i32 = 232;
/// Gap between a bubble and the thing it points at.
const GAP: f64 = 14.0;
/// Space kept clear at the window edges.
const MARGIN: f64 = 8.0;

struct Target {
    widget: gtk4::Widget,
    bubble: GtkBox,
    /// Where the bubble ended up, in layer coordinates — the draw function
    /// needs it to run the connector to the right edge.
    placed: Cell<Option<(f64, f64, f64, f64)>>,
    /// The widget's own rectangle, or None when it isn't on screen.
    anchor: Cell<Option<(f64, f64, f64, f64)>>,
}

pub struct HelpOverlay {
    /// Wraps the whole window; hand this to `window.set_content`.
    root: gtk4::Overlay,
    /// The layer bubbles and lines are drawn on. Hidden until asked for.
    coach: gtk4::Overlay,
    fixed: Fixed,
    area: DrawingArea,
    targets: Rc<RefCell<Vec<Rc<Target>>>>,
    hint: GtkBox,
}

impl HelpOverlay {
    pub fn new(content: &impl IsA<gtk4::Widget>) -> Rc<Self> {
        let root = gtk4::Overlay::new();
        root.set_child(Some(content));

        let area = DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        // Lines and outlines only; every click belongs to the layer above.
        area.set_can_target(false);

        let fixed = Fixed::new();
        fixed.set_hexpand(true);
        fixed.set_vexpand(true);

        let coach = gtk4::Overlay::new();
        coach.set_child(Some(&area));
        coach.add_overlay(&fixed);
        coach.set_visible(false);

        let hint = hint_pill();
        fixed.put(&hint, 0.0, 0.0);

        root.add_overlay(&coach);

        let this = Rc::new(Self {
            root,
            coach,
            fixed,
            area,
            targets: Rc::new(RefCell::new(Vec::new())),
            hint,
        });

        {
            let this_c = this.clone();
            this.area.set_draw_func(move |area, cr, w, h| {
                this_c.draw(area, cr, w as f64, h as f64);
            });
        }

        // Any click dismisses — the overlay is something you glance at, not
        // something to navigate.
        {
            let this_c = this.clone();
            let click = gtk4::GestureClick::new();
            click.set_button(0);
            click.connect_pressed(move |_, _, _, _| this_c.hide());
            this.coach.add_controller(click);
        }

        // Re-place everything when the window changes shape, so the bubbles
        // keep pointing at what they describe.
        {
            let this_c = this.clone();
            this.coach.connect_map(move |_| this_c.relayout());
        }
        {
            // The drawing area is the one that reports size changes; the Fixed
            // beside it in the same overlay always gets the same allocation.
            let this_c = this.clone();
            this.area.connect_resize(move |_, _, _| this_c.relayout());
        }

        this
    }

    /// The widget to install as the window's content.
    pub fn widget(&self) -> &gtk4::Overlay {
        &self.root
    }

    /// Labels `widget` with a short name and a sentence about what it does.
    pub fn annotate(&self, widget: &impl IsA<gtk4::Widget>, title: &str, body: &str) {
        let bubble = bubble_widget(title, body);
        bubble.set_size_request(BUBBLE_W, -1);
        bubble.set_visible(false);
        self.fixed.put(&bubble, 0.0, 0.0);
        self.targets.borrow_mut().push(Rc::new(Target {
            widget: widget.clone().upcast(),
            bubble,
            placed: Cell::new(None),
            anchor: Cell::new(None),
        }));
    }

    pub fn is_shown(&self) -> bool {
        self.coach.is_visible()
    }

    pub fn show(self: &Rc<Self>) {
        self.coach.set_visible(true);
        // Laying out here would measure a layer GTK hasn't allocated yet —
        // every bubble would be placed against a zero-sized window and stay
        // invisible. The idle callback runs after the layout pass, when the
        // widgets it has to point at actually have positions.
        let this = self.clone();
        glib::idle_add_local_once(move || this.relayout());
    }

    pub fn hide(&self) {
        self.coach.set_visible(false);
    }

    pub fn toggle(self: &Rc<Self>) {
        if self.is_shown() {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Works out where every bubble goes, then asks for a redraw.
    fn relayout(&self) {
        // Geometry comes from the drawing area, not the Fixed beside it: an
        // Overlay gives a Fixed no allocation of its own (it measures zero,
        // since its children are positioned rather than packed), so asking the
        // Fixed how big it is answers 0x0 and every bubble lands off-window.
        // The size request keeps the Fixed's own coordinate space aligned with
        // the area it is layered over.
        let width = self.area.width() as f64;
        let height = self.area.height() as f64;
        if width <= 1.0 || height <= 1.0 {
            return;
        }
        if self.fixed.width_request() != self.area.width()
            || self.fixed.height_request() != self.area.height()
        {
            self.fixed
                .set_size_request(self.area.width(), self.area.height());
        }

        // The hint sits at the bottom centre, out of the way of the header
        // where most of the controls — and so most of the bubbles — are.
        let (_, hint_w, _, _) = self.hint.measure(Orientation::Horizontal, -1);
        let (_, hint_h, _, _) = self.hint.measure(Orientation::Vertical, hint_w);
        self.fixed.move_(
            &self.hint,
            ((width - hint_w as f64) / 2.0).max(MARGIN),
            height - hint_h as f64 - MARGIN * 2.0,
        );

        // Every bubble is made visible before any of them is measured: GTK
        // reports a hidden widget's size as zero, so measuring first produced
        // zero-height rectangles, no two of which ever appeared to overlap —
        // and a dozen bubbles piled up on the same spot. Ones with nothing to
        // point at are hidden again as they're found.
        for target in self.targets.borrow().iter() {
            target.bubble.set_visible(true);
        }

        let mut placed: Vec<(f64, f64, f64, f64)> = vec![(
            (width - hint_w as f64) / 2.0,
            height - hint_h as f64 - MARGIN * 2.0,
            hint_w as f64,
            hint_h as f64,
        )];

        for target in self.targets.borrow().iter() {
            let bounds = if target.widget.is_visible() && target.widget.is_mapped() {
                target.widget.compute_bounds(&self.area)
            } else {
                None
            };
            let Some(rect) = bounds else {
                target.anchor.set(None);
                target.placed.set(None);
                target.bubble.set_visible(false);
                continue;
            };
            let (rx, ry, rw, rh) = (
                rect.x() as f64,
                rect.y() as f64,
                rect.width() as f64,
                rect.height() as f64,
            );
            // A widget scrolled out of view still has bounds — they just fall
            // outside the window. Labelling those would point at nothing.
            if rw < 1.0 || rh < 1.0 || rx > width || ry > height || rx + rw < 0.0 || ry + rh < 0.0 {
                target.anchor.set(None);
                target.placed.set(None);
                target.bubble.set_visible(false);
                continue;
            }
            target.anchor.set(Some((rx, ry, rw, rh)));

            let (_, bh, _, _) = target.bubble.measure(Orientation::Vertical, BUBBLE_W);
            let bw = BUBBLE_W as f64;
            let bh = bh as f64;

            let spot = place_bubble((rx, ry, rw, rh), bw, bh, width, height, &placed);
            placed.push((spot.0, spot.1, bw, bh));
            target.placed.set(Some((spot.0, spot.1, bw, bh)));
            target.bubble.set_visible(true);
            self.fixed.move_(&target.bubble, spot.0, spot.1);
        }

        self.area.queue_draw();
    }

    fn draw(&self, area: &DrawingArea, cr: &gtk4::cairo::Context, _w: f64, _h: f64) {
        let accent = super::theme::rgb(area, "accent_color").unwrap_or((0.21, 0.52, 0.89));

        for target in self.targets.borrow().iter() {
            let (Some(anchor), Some(bubble)) = (target.anchor.get(), target.placed.get()) else {
                continue;
            };

            // Small controls get a light wash so the eye finds them; whole
            // panels get an outline only. Tinting those washed the entire
            // window blue and buried the thing the overlay exists to explain.
            rounded_rect(cr, anchor.0, anchor.1, anchor.2, anchor.3, 6.0);
            if anchor.2 * anchor.3 < 30_000.0 {
                cr.set_source_rgba(accent.0, accent.1, accent.2, 0.14);
                let _ = cr.fill_preserve();
            }
            cr.set_source_rgba(accent.0, accent.1, accent.2, 0.9);
            cr.set_line_width(2.0);
            let _ = cr.stroke();

            let (from, to) = connector(bubble, anchor);
            cr.set_source_rgba(accent.0, accent.1, accent.2, 0.75);
            cr.set_line_width(2.0);
            cr.move_to(from.0, from.1);
            cr.line_to(to.0, to.1);
            let _ = cr.stroke();

            cr.set_source_rgba(accent.0, accent.1, accent.2, 0.95);
            cr.arc(to.0, to.1, 3.0, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
        }
    }
}

/// Chooses where a bubble goes for a target rectangle: beside it if there's
/// room, otherwise above or below, then nudged clear of bubbles already
/// placed. Returns the bubble's top-left corner.
fn place_bubble(
    anchor: (f64, f64, f64, f64),
    bw: f64,
    bh: f64,
    width: f64,
    height: f64,
    placed: &[(f64, f64, f64, f64)],
) -> (f64, f64) {
    let (ax, ay, aw, ah) = anchor;
    let centre_y = (ay + ah / 2.0 - bh / 2.0).clamp(MARGIN, (height - bh - MARGIN).max(MARGIN));
    let centre_x = (ax + aw / 2.0 - bw / 2.0).clamp(MARGIN, (width - bw - MARGIN).max(MARGIN));

    let candidates = [
        (ax + aw + GAP, centre_y), // right
        (ax - bw - GAP, centre_y), // left
        (centre_x, ay + ah + GAP), // below
        (centre_x, ay - bh - GAP), // above
    ];

    for (cx, cy) in candidates {
        if cx < MARGIN || cy < MARGIN || cx + bw > width - MARGIN || cy + bh > height - MARGIN {
            continue;
        }
        if placed.iter().any(|p| overlaps(*p, (cx, cy, bw, bh))) {
            continue;
        }
        return (cx, cy);
    }

    // Nothing fitted beside the target — which is the normal case for a row of
    // header buttons, where a dozen bubbles compete for the same strip of
    // window. Sweep the whole layer and take the free spot nearest the target:
    // a bubble further away joined by a longer line still reads, where two
    // bubbles on top of each other read as neither.
    let step = 12.0;
    let (acx, acy) = (ax + aw / 2.0, ay + ah / 2.0);
    let mut best: Option<(f64, f64, f64)> = None;
    let mut y = MARGIN;
    while y + bh <= height - MARGIN {
        let mut x = MARGIN;
        while x + bw <= width - MARGIN {
            if !placed.iter().any(|p| overlaps(*p, (x, y, bw, bh))) {
                let d = (x + bw / 2.0 - acx).powi(2) + (y + bh / 2.0 - acy).powi(2);
                if best.is_none_or(|b| d < b.2) {
                    best = Some((x, y, d));
                }
            }
            x += step;
        }
        y += step;
    }
    if let Some((x, y, _)) = best {
        return (x, y);
    }
    // The window is genuinely full. Better a legible bubble in a known place
    // than one pushed off the edge.
    (centre_x, centre_y)
}

fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// The two ends of the line joining a bubble to its target: from the bubble
/// edge facing the target, to the nearest point on the target's edge.
fn connector(
    bubble: (f64, f64, f64, f64),
    anchor: (f64, f64, f64, f64),
) -> ((f64, f64), (f64, f64)) {
    let (bx, by, bw, bh) = bubble;
    let (ax, ay, aw, ah) = anchor;
    let (bcx, bcy) = (bx + bw / 2.0, by + bh / 2.0);
    let (acx, acy) = (ax + aw / 2.0, ay + ah / 2.0);

    // Which side of the bubble the target lies on. Both ends of the line are
    // derived from this one decision: deciding them separately let the line
    // leave the bubble's top edge and then aim at the target's side, crossing
    // the control it was meant to point at.
    enum Side {
        Right,
        Left,
        Below,
        Above,
    }
    let side = if acx > bx + bw {
        Side::Right
    } else if acx < bx {
        Side::Left
    } else if acy > by + bh {
        Side::Below
    } else {
        Side::Above
    };

    match side {
        Side::Right => ((bx + bw, bcy), (ax, acy.clamp(ay, ay + ah))),
        Side::Left => ((bx, bcy), (ax + aw, acy.clamp(ay, ay + ah))),
        Side::Below => ((bcx, by + bh), (acx.clamp(ax, ax + aw), ay)),
        Side::Above => ((bcx, by), (acx.clamp(ax, ax + aw), ay + ah)),
    }
}

fn rounded_rect(cr: &gtk4::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(
        x + r,
        y + h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    cr.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        1.5 * std::f64::consts::PI,
    );
    cr.close_path();
}

fn bubble_widget(title: &str, body: &str) -> GtkBox {
    let b = GtkBox::new(Orientation::Vertical, 2);
    b.add_css_class("help-bubble");
    b.set_halign(Align::Start);
    b.set_valign(Align::Start);

    let title_lbl = Label::new(Some(title));
    title_lbl.set_xalign(0.0);
    title_lbl.set_wrap(true);
    title_lbl.add_css_class("help-bubble-title");
    b.append(&title_lbl);

    let body_lbl = Label::new(Some(body));
    body_lbl.set_xalign(0.0);
    body_lbl.set_wrap(true);
    body_lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    body_lbl.add_css_class("help-bubble-body");
    b.append(&body_lbl);

    b
}

fn hint_pill() -> GtkBox {
    let b = GtkBox::new(Orientation::Horizontal, 0);
    b.add_css_class("help-hint");
    b.set_halign(Align::Start);
    b.set_valign(Align::Start);
    let lbl = Label::new(Some(
        "What things do — press F1, Esc, or click anywhere to close",
    ));
    b.append(&lbl);
    b
}

/// The widgets the window labels. Passed as one struct rather than fifteen
/// arguments so adding a label is a one-line change at both ends.
pub struct AnnotationTargets<'a> {
    pub sidebar_btn: &'a gtk4::Button,
    pub file_title_widget: &'a libadwaita::WindowTitle,
    pub style_btn: &'a gtk4::Button,
    pub sync_btn: &'a gtk4::Button,
    pub library_btn: &'a gtk4::Button,
    pub preview_label: &'a Label,
    pub menu_btn: &'a gtk4::MenuButton,
    pub compile_btn: &'a gtk4::Button,
    pub compile_mode_slot: &'a GtkBox,
    pub outline: &'a GtkBox,
    pub citations: &'a GtkBox,
    pub editor: &'a GtkBox,
    pub preview: &'a GtkBox,
    pub status_bar: &'a GtkBox,
}

/// Written for someone who has never used a Typst editor: what the thing is
/// for, in one sentence, without naming a concept the sentence doesn't
/// explain. Order matters — bubbles are placed in sequence, so the ones that
/// matter most claim the best space.
pub fn annotate_window(overlay: &HelpOverlay, t: &AnnotationTargets) {
    overlay.annotate(
        t.editor,
        "Your document",
        "Type here. The text is plain, and the formatting comes from the buttons above and the settings in Template.",
    );
    overlay.annotate(
        t.preview,
        "The finished page",
        "How your document will look when printed or shared. It re-draws itself as you type.",
    );
    overlay.annotate(
        t.outline,
        "Outline",
        "Every heading in the document. Click one to jump straight to it.",
    );
    overlay.annotate(
        t.citations,
        "Citations",
        "Sources from your bibliography file. Double-click one to cite it where the cursor is.",
    );
    overlay.annotate(
        t.file_title_widget,
        "The open document",
        "The name of what you're editing. Click it to open something else or start a new one.",
    );
    overlay.annotate(
        t.style_btn,
        "Template",
        "Title, author, margins, fonts and citation style — everything about how the document is set out.",
    );
    overlay.annotate(
        t.sync_btn,
        "Save & Back Up",
        "Saves everything and sends it to your backup, so it survives losing this computer. Ctrl+Shift+S — plain Ctrl+S saves to disk without backing up.",
    );
    overlay.annotate(
        t.compile_btn,
        "Re-draw the page",
        "Builds the preview again. Normally it happens on its own; this is for when you want it now.",
    );
    overlay.annotate(
        t.compile_mode_slot,
        "When to re-draw",
        "Whether the page rebuilds as you type, only when you save, or only when you ask.",
    );
    overlay.annotate(
        t.preview_label,
        "Preview",
        "Hides or shows the finished page, giving the whole window to your writing.",
    );
    overlay.annotate(
        t.library_btn,
        "Library",
        "Every document Zerkalo knows about, with the newest first. Ctrl+L.",
    );
    overlay.annotate(
        t.sidebar_btn,
        "Side panels",
        "Hides or shows the outline and citations panels.",
    );
    overlay.annotate(
        t.menu_btn,
        "Menu",
        "Everything else: new documents, import, export, print, settings and help.",
    );
    overlay.annotate(
        t.status_bar,
        "Status bar",
        "Where you are in the document, how many words you've written, and switches for the modes below the writing area.",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bubble_goes_beside_its_target_when_there_is_room() {
        let placed = place_bubble((300.0, 300.0, 40.0, 20.0), 200.0, 60.0, 1000.0, 800.0, &[]);
        assert_eq!(placed.0, 300.0 + 40.0 + GAP, "should sit to the right");
    }

    #[test]
    fn a_bubble_never_leaves_the_window() {
        // A control hard against the right edge: the bubble must come inside,
        // not hang off where it can't be read.
        let (x, y) = place_bubble((960.0, 10.0, 30.0, 20.0), 200.0, 60.0, 1000.0, 800.0, &[]);
        assert!(x >= MARGIN, "x={x} is off the left edge");
        assert!(
            x + 200.0 <= 1000.0 - MARGIN + 0.01,
            "x={x} runs off the right edge"
        );
        assert!(y >= MARGIN, "y={y} is above the top edge");
        assert!(
            y + 60.0 <= 800.0 - MARGIN + 0.01,
            "y={y} runs off the bottom"
        );
    }

    #[test]
    fn two_targets_in_the_same_place_do_not_stack_their_bubbles() {
        let first = place_bubble((300.0, 300.0, 40.0, 20.0), 200.0, 60.0, 1000.0, 800.0, &[]);
        let occupied = [(first.0, first.1, 200.0, 60.0)];
        let second = place_bubble(
            (300.0, 300.0, 40.0, 20.0),
            200.0,
            60.0,
            1000.0,
            800.0,
            &occupied,
        );
        assert!(
            !overlaps(
                (first.0, first.1, 200.0, 60.0),
                (second.0, second.1, 200.0, 60.0)
            ),
            "bubbles overlap: {first:?} and {second:?}"
        );
    }

    #[test]
    fn the_connector_leaves_the_bubble_on_the_side_facing_its_target() {
        // Bubble to the left of the target: the line must start on the
        // bubble's right edge, or it crosses the bubble to get out.
        let (from, to) = connector((100.0, 100.0, 200.0, 60.0), (400.0, 110.0, 40.0, 20.0));
        assert_eq!(from.0, 300.0, "should leave from the right edge");
        assert_eq!(to.0, 400.0, "should stop at the target's near edge");
    }

    #[test]
    fn the_connector_stops_at_the_targets_edge_from_below() {
        let (from, to) = connector((100.0, 300.0, 200.0, 60.0), (150.0, 100.0, 40.0, 20.0));
        assert_eq!(from.1, 300.0, "should leave from the bubble's top edge");
        assert_eq!(to.1, 120.0, "should stop at the target's bottom edge");
    }

    #[test]
    fn overlap_detection_ignores_rectangles_that_merely_touch() {
        assert!(!overlaps((0.0, 0.0, 10.0, 10.0), (10.0, 0.0, 10.0, 10.0)));
        assert!(overlaps((0.0, 0.0, 10.0, 10.0), (9.0, 0.0, 10.0, 10.0)));
    }
}

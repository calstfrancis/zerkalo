//! A short, step-by-step guided walkthrough for brand-new users.
//!
//! Same bubble-and-connector visual language as `help_overlay.rs`'s F1 "what
//! things do" overlay — reuses its placement, connector, and outline-drawing
//! helpers directly rather than re-deriving them — but shows one step at a
//! time with Next/Back/Skip controls built into the bubble itself, instead of
//! labelling every control at once. F1 is "look anything up when you want
//! to"; the tour is "here's the handful of things worth knowing first,"
//! shown once automatically and replayable from Help & About afterward.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, DrawingArea, Fixed, Label, Orientation};

use super::help_overlay::{
    connector, draw_connector, place_bubble, rounded_rect, AnnotationTargets,
};

const BUBBLE_W: i32 = 260;
const MARGIN: f64 = 8.0;

#[derive(Clone)]
pub struct TourStep {
    widget: gtk4::Widget,
    title: &'static str,
    body: &'static str,
}

/// Builds one step. A plain function rather than a `TourStep::new` — the call
/// site (`tour_steps` in `app_window/mod.rs`) reads as a flat list this way.
pub fn step(widget: &impl IsA<gtk4::Widget>, title: &'static str, body: &'static str) -> TourStep {
    TourStep {
        widget: widget.clone().upcast(),
        title,
        body,
    }
}

pub struct Tour {
    /// Wraps the whole window; hand this to `window.set_content`.
    root: gtk4::Overlay,
    /// The layer the bubble and its connector are drawn on. Hidden until the
    /// tour starts.
    coach: gtk4::Overlay,
    fixed: Fixed,
    area: DrawingArea,
    bubble: GtkBox,
    title_lbl: Label,
    body_lbl: Label,
    step_lbl: Label,
    back_btn: Button,
    next_btn: Button,
    steps: RefCell<Vec<TourStep>>,
    index: Cell<usize>,
    anchor: Cell<Option<(f64, f64, f64, f64)>>,
    placed: Cell<Option<(f64, f64, f64, f64)>>,
    on_finished: RefCell<Option<Box<dyn Fn()>>>,
    /// Same reentrancy guard as `HelpOverlay::relayout` — moving the bubble
    /// can synchronously trigger another resize callback before this pass
    /// finishes. See its doc comment for the full explanation.
    laying_out: Cell<bool>,
    relayout_pending: Cell<bool>,
}

impl Tour {
    pub fn new(content: &impl IsA<gtk4::Widget>) -> Rc<Self> {
        let root = gtk4::Overlay::new();
        root.set_child(Some(content));

        let area = DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        // Outline and connector only; every click belongs to the bubble
        // above (its buttons) or dismisses nothing on its own — unlike the
        // F1 overlay, an accidental background click shouldn't cancel a tour
        // someone is mid-way through reading.
        area.set_can_target(false);

        let fixed = Fixed::new();
        fixed.set_hexpand(true);
        fixed.set_vexpand(true);

        let coach = gtk4::Overlay::new();
        coach.set_child(Some(&area));
        coach.add_overlay(&fixed);
        coach.set_visible(false);

        let (bubble, title_lbl, body_lbl, step_lbl, back_btn, next_btn, skip_btn) = build_bubble();
        bubble.set_size_request(BUBBLE_W, -1);
        fixed.put(&bubble, 0.0, 0.0);

        root.add_overlay(&coach);

        let this = Rc::new(Self {
            root,
            coach,
            fixed,
            area,
            bubble,
            title_lbl,
            body_lbl,
            step_lbl,
            back_btn,
            next_btn,
            steps: RefCell::new(Vec::new()),
            index: Cell::new(0),
            anchor: Cell::new(None),
            placed: Cell::new(None),
            on_finished: RefCell::new(None),
            laying_out: Cell::new(false),
            relayout_pending: Cell::new(false),
        });

        {
            let this_c = this.clone();
            this.area.set_draw_func(move |area, cr, w, h| {
                this_c.draw(area, cr, w as f64, h as f64);
            });
        }
        {
            let this_c = this.clone();
            this.next_btn.connect_clicked(move |_| this_c.advance());
        }
        {
            let this_c = this.clone();
            this.back_btn.connect_clicked(move |_| this_c.retreat());
        }
        {
            let this_c = this.clone();
            skip_btn.connect_clicked(move |_| this_c.finish());
        }
        {
            let this_c = this.clone();
            this.area.connect_resize(move |_, _, _| this_c.relayout());
        }

        this
    }

    /// The widget to install as the window's content.
    pub fn widget(&self) -> &gtk4::Overlay {
        &self.root
    }

    pub fn is_shown(&self) -> bool {
        self.coach.is_visible()
    }

    /// Called once the tour ends, whether by finishing the last step or
    /// being skipped — both count as "seen," so the caller can record that.
    pub fn set_on_finished(&self, f: impl Fn() + 'static) {
        *self.on_finished.borrow_mut() = Some(Box::new(f));
    }

    /// Starts from the first step. Safe to call again later (e.g. from a
    /// "Take the Tour" menu row) — it replaces whatever steps were running.
    pub fn start(self: &Rc<Self>, steps: Vec<TourStep>) {
        if steps.is_empty() {
            return;
        }
        *self.steps.borrow_mut() = steps;
        self.index.set(0);
        self.coach.set_visible(true);
        // Laying out here would measure a layer GTK hasn't allocated yet —
        // the bubble would be placed against a zero-sized window and stay
        // invisible. The idle callback runs after the layout pass.
        let this = self.clone();
        glib::idle_add_local_once(move || this.show_step());
    }

    /// Ends the tour, however it ended — Skip, Done on the last step, or the
    /// window closing mid-tour.
    pub fn finish(&self) {
        if !self.is_shown() {
            return;
        }
        self.coach.set_visible(false);
        self.steps.borrow_mut().clear();
        if let Some(f) = self.on_finished.borrow().as_ref() {
            f();
        }
    }

    fn advance(&self) {
        let last = self.steps.borrow().len().saturating_sub(1);
        if self.index.get() >= last {
            self.finish();
        } else {
            self.index.set(self.index.get() + 1);
            self.show_step();
        }
    }

    fn retreat(&self) {
        if self.index.get() > 0 {
            self.index.set(self.index.get() - 1);
            self.show_step();
        }
    }

    fn show_step(&self) {
        let (title, body, step_text, has_back, is_last) = {
            let steps = self.steps.borrow();
            let Some(cur) = steps.get(self.index.get()) else {
                return;
            };
            (
                cur.title,
                cur.body,
                format!("{} of {}", self.index.get() + 1, steps.len()),
                self.index.get() > 0,
                self.index.get() + 1 == steps.len(),
            )
        };
        self.title_lbl.set_text(title);
        self.body_lbl.set_text(body);
        self.step_lbl.set_text(&step_text);
        self.back_btn.set_sensitive(has_back);
        self.next_btn
            .set_label(if is_last { "Done" } else { "Next" });
        self.relayout();
    }

    /// Works out where the bubble goes, then asks for a redraw. Only ever one
    /// bubble on screen, so — unlike the F1 overlay's `relayout`, which has to
    /// dodge every other bubble already placed — there's nothing to avoid but
    /// the window edges.
    fn relayout(&self) {
        if self.laying_out.get() {
            self.relayout_pending.set(true);
            return;
        }
        self.laying_out.set(true);
        self.relayout_inner();
        self.laying_out.set(false);
        if self.relayout_pending.take() {
            self.relayout();
        }
    }

    fn relayout_inner(&self) {
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

        let steps = self.steps.borrow();
        let Some(cur) = steps.get(self.index.get()) else {
            self.anchor.set(None);
            self.placed.set(None);
            return;
        };

        let bounds = if cur.widget.is_visible() && cur.widget.is_mapped() {
            cur.widget.compute_bounds(&self.area)
        } else {
            None
        };
        drop(steps);

        let Some(rect) = bounds else {
            // Nothing to point at right now — the panel this step describes
            // is hidden (e.g. the sidebar toggled off). Centre the bubble
            // rather than leaving it stuck at a stale spot or drawing a
            // connector to nowhere; the copy still reads on its own.
            self.anchor.set(None);
            let (_, bh, _, _) = self.bubble.measure(Orientation::Vertical, BUBBLE_W);
            let x = ((width - BUBBLE_W as f64) / 2.0).max(MARGIN);
            let y = ((height - bh as f64) / 2.0).max(MARGIN);
            self.placed.set(Some((x, y, BUBBLE_W as f64, bh as f64)));
            self.fixed.move_(&self.bubble, x, y);
            self.area.queue_draw();
            return;
        };
        let (rx, ry, rw, rh) = (
            rect.x() as f64,
            rect.y() as f64,
            rect.width() as f64,
            rect.height() as f64,
        );
        self.anchor.set(Some((rx, ry, rw, rh)));

        let (_, bh, _, _) = self.bubble.measure(Orientation::Vertical, BUBBLE_W);
        let bw = BUBBLE_W as f64;
        let bh = bh as f64;
        // The tour only ever shows one bubble against an empty obstacle list,
        // so `None` here means the target's own anchor is too large to fit a
        // bubble beside anywhere — a full-bleed step like "Your document"
        // pointing at the whole editor pane. Center it the same way the
        // "target not on screen" branch above already does; there's no other
        // bubble here for it to collide with, just its own oversized target.
        let (x, y) = place_bubble((rx, ry, rw, rh), bw, bh, width, height, &[]).unwrap_or((
            ((width - bw) / 2.0).max(MARGIN),
            ((height - bh) / 2.0).max(MARGIN),
        ));
        self.placed.set(Some((x, y, bw, bh)));
        self.fixed.move_(&self.bubble, x, y);
        self.area.queue_draw();
    }

    fn draw(&self, area: &DrawingArea, cr: &gtk4::cairo::Context, _w: f64, _h: f64) {
        let (Some(anchor), Some(bubble)) = (self.anchor.get(), self.placed.get()) else {
            return;
        };
        let accent = super::theme::rgb(area, "accent_color").unwrap_or((0.21, 0.52, 0.89));

        rounded_rect(cr, anchor.0, anchor.1, anchor.2, anchor.3, 6.0);
        if anchor.2 * anchor.3 < 30_000.0 {
            cr.set_source_rgba(accent.0, accent.1, accent.2, 0.14);
            let _ = cr.fill_preserve();
        }
        cr.set_source_rgba(accent.0, accent.1, accent.2, 0.9);
        cr.set_line_width(2.0);
        let _ = cr.stroke();

        let (from, to, is_horizontal) = connector(bubble, anchor);
        draw_connector(cr, from, to, is_horizontal, accent);
    }
}

/// The main-window walkthrough, shown once on first run and replayable from
/// Help & About → Take the Tour. Reuses the same widget references the F1
/// overlay annotates (`AnnotationTargets`) but as a short, ordered sequence
/// with its own "first five minutes" copy rather than a full reference —
/// the template step in particular spells out the one thing that actually
/// trips people up: applying a template never throws away what you've
/// already written.
pub fn tour_steps(t: &AnnotationTargets) -> Vec<TourStep> {
    vec![
        step(
            t.editor,
            "Start writing here",
            "This is a plain-text editor — type prose with light markup like *bold* or # a heading. Nothing here is locked to a template yet.",
        ),
        step(
            t.preview,
            "Your typeset page",
            "Every change redraws here automatically. This is what the finished PDF actually looks like — headings, spacing, citations, all real.",
        ),
        step(
            t.style_btn,
            "Set up title, author, and style",
            "Template controls everything about how the document is laid out: title page, font, margins, citation style. If you've already been writing, applying a template only replaces this setup — your own text is always kept as the body, never discarded.",
        ),
        step(
            t.library_btn,
            "All your documents",
            "Ctrl+L. Every document you make is kept here, newest first, with search — this is how you get back to something without hunting through folders.",
        ),
        step(
            t.compile_mode_slot,
            "Auto or manual",
            "Auto redraws the preview as you type; Manual waits for you to ask (Save, this button, or Ctrl+Shift+P). Manual is the default — click to switch.",
        ),
        step(
            t.sync_btn,
            "Back up your work",
            "Saves everything and sends it to your configured backup, so a lost laptop doesn't mean a lost document. Set it up once from the menu below.",
        ),
        step(
            t.menu_btn,
            "That's the essentials",
            "Everything else — import, export, print, settings — lives in this menu. Forgot what a button does later? Press F1 anytime to label the whole window, or come back to Help & About → Take the Tour.",
        ),
    ]
}

fn build_bubble() -> (GtkBox, Label, Label, Label, Button, Button, Button) {
    let b = GtkBox::new(Orientation::Vertical, 6);
    b.add_css_class("help-bubble");
    b.set_halign(Align::Start);
    b.set_valign(Align::Start);

    let title_lbl = Label::new(None);
    title_lbl.set_xalign(0.0);
    title_lbl.set_wrap(true);
    title_lbl.add_css_class("help-bubble-title");
    b.append(&title_lbl);

    let body_lbl = Label::new(None);
    body_lbl.set_xalign(0.0);
    body_lbl.set_wrap(true);
    body_lbl.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    body_lbl.add_css_class("help-bubble-body");
    b.append(&body_lbl);

    let controls = GtkBox::new(Orientation::Horizontal, 6);
    controls.set_margin_top(4);

    let step_lbl = Label::new(None);
    step_lbl.add_css_class("dim-label");
    step_lbl.add_css_class("caption");
    step_lbl.set_hexpand(true);
    step_lbl.set_valign(Align::Center);
    step_lbl.set_xalign(0.0);
    controls.append(&step_lbl);

    let skip_btn = Button::with_label("Skip");
    skip_btn.add_css_class("flat");
    controls.append(&skip_btn);

    let back_btn = Button::with_label("Back");
    back_btn.add_css_class("flat");
    controls.append(&back_btn);

    let next_btn = Button::with_label("Next");
    next_btn.add_css_class("suggested-action");
    controls.append(&next_btn);

    b.append(&controls);

    (
        b, title_lbl, body_lbl, step_lbl, back_btn, next_btn, skip_btn,
    )
}

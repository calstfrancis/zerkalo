use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use gtk4::gdk::prelude::GdkCairoContextExt;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, DrawingArea, EventControllerKey, GestureClick, Label,
    Orientation, Overlay, ScrolledWindow, Spinner, Stack,
};
use std::time::Instant;

// ── Result sent from compile thread ──────────────────────────────────────────

/// `(error, warnings)` — the error is `None` on success, and warnings are the
/// empty string when the compile was clean. Both use the `error: …`/`warning: …`
/// text format `parse_typst_errors` reads.
type CompileDoneFn = dyn Fn(Option<String>, String);

enum CompileResult {
    /// Pages, warnings (empty when clean), elapsed.
    Success(
        Vec<crate::compiler::RenderedPage>,
        String,
        std::time::Duration,
    ),
    Error(String, std::time::Duration),
}

// ── Widget ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PreviewPane {
    root_widget: GtkBox,
    stack: Stack,
    img_scroll: ScrolledWindow,
    drawing_area: DrawingArea,
    spinner: Spinner,
    cancel_btn: Button,
    error_label: Label,
    output_dir: Rc<PathBuf>,
    extra_args: Rc<Vec<String>>,
    root_file: Rc<RefCell<Option<PathBuf>>>,
    zoom: Rc<RefCell<f64>>,
    auto_fit: Rc<RefCell<bool>>,
    on_compile_done: Rc<RefCell<Option<Box<CompileDoneFn>>>>,
    on_compile_cancelled: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_compile_time: Rc<RefCell<Option<Box<dyn Fn(u64, Option<usize>)>>>>,
    on_compile_start: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    spin_lbl: Label,
    compile_start_instant: Rc<RefCell<Option<Instant>>>,
    on_zoom_changed: Rc<RefCell<Option<Box<dyn Fn(f64)>>>>,
    on_page_changed: Rc<RefCell<Option<Box<dyn Fn(usize, usize)>>>>,
    on_click_jump: Rc<RefCell<Option<Box<dyn Fn(usize, f64)>>>>,
    on_word_click_jump: Rc<RefCell<Option<Box<dyn Fn(usize, f64, f64)>>>>,
    page_pixbufs: Rc<RefCell<Vec<Pixbuf>>>,
    watch_active: Rc<RefCell<bool>>,
    compile_gen: Rc<RefCell<u64>>,
    /// A Typst compile is running. Typst offers no way to abort one, so rather
    /// than letting each edit spawn another thread that races the last — several
    /// full compiles competing with the UI for cores — a request arriving mid
    /// compile just sets `compile_pending` and is run when the current one lands.
    compile_in_flight: Rc<Cell<bool>>,
    compile_pending: Rc<Cell<bool>>,
    buffer_snapshot: Rc<RefCell<HashMap<PathBuf, String>>>,
    /// CV mode's Skrizhal `cv-elements.yaml` path, if any — re-read fresh on
    /// every compile (see `set_cv_elements_path`) rather than cached, so
    /// edits made in Skrizhal while Zerkalo is open show up without a
    /// restart. `None` means not in CV mode for this document.
    cv_elements_path: Rc<RefCell<Option<PathBuf>>>,
    /// The configured bibliography path (`Config::bib_path`, project override
    /// included) — most often irrelevant to the compile sandbox, but when it
    /// points outside the project (a Kartoteka vault living elsewhere, most
    /// commonly) the compiler needs it to widen the World's root so the file
    /// is actually reachable. See `compiler::ZerkaloWorld::new`'s `extra_root`.
    bib_path: Rc<RefCell<Option<PathBuf>>>,
    draft_mode: Rc<RefCell<bool>>,
    first_load: Rc<RefCell<bool>>,
    zoom_osd: Label,
    osd_timer: Rc<RefCell<Option<glib::SourceId>>>,
    osd_fade_timer: Rc<RefCell<Option<glib::SourceId>>>,
}

impl PreviewPane {
    pub fn new(
        root_file: Option<PathBuf>,
        output_dir: Option<PathBuf>,
        extra_args: Vec<String>,
    ) -> Self {
        let root_widget = GtkBox::new(Orientation::Vertical, 0);
        root_widget.set_hexpand(true);
        root_widget.set_vexpand(true);

        let stack = Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);

        // ── empty page ────────────────────────────────────────────────────────
        let empty_lbl = Label::new(Some("No preview\nCtrl+Shift+P to compile"));
        empty_lbl.add_css_class("dim-label");
        empty_lbl.set_justify(gtk4::Justification::Center);
        stack.add_named(&empty_lbl, Some("empty"));

        // ── compiling page ────────────────────────────────────────────────────
        let spin_box = GtkBox::new(Orientation::Vertical, 12);
        spin_box.set_halign(Align::Center);
        spin_box.set_valign(Align::Center);
        let spinner = Spinner::new();
        spinner.set_size_request(48, 48);
        let spin_lbl = Label::new(Some("Compiling\u{2026}"));
        spin_lbl.add_css_class("dim-label");
        let spin_lbl_store = spin_lbl.clone();
        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        cancel_btn.set_visible(false);
        spin_box.append(&spinner);
        spin_box.append(&spin_lbl);
        spin_box.append(&cancel_btn);
        stack.add_named(&spin_box, Some("compiling"));

        // ── ready page: DrawingArea inside ScrolledWindow ─────────────────────
        let img_scroll = ScrolledWindow::new();
        img_scroll.add_css_class("fond-ground");
        img_scroll.set_hexpand(true);
        img_scroll.set_vexpand(true);

        let drawing_area = DrawingArea::new();
        drawing_area.set_halign(Align::Center);
        drawing_area.set_valign(Align::Start);
        img_scroll.set_child(Some(&drawing_area));
        stack.add_named(&img_scroll, Some("ready"));

        // ── error page ────────────────────────────────────────────────────────
        let err_scroll = ScrolledWindow::new();
        err_scroll.set_hexpand(true);
        err_scroll.set_vexpand(true);
        let error_label = Label::new(None);
        error_label.set_wrap(true);
        error_label.set_selectable(true);
        error_label.set_halign(Align::Start);
        error_label.set_valign(Align::Start);
        error_label.set_margin_top(12);
        error_label.set_margin_start(12);
        error_label.set_margin_end(12);
        error_label.add_css_class("error");
        err_scroll.set_child(Some(&error_label));
        stack.add_named(&err_scroll, Some("error"));

        stack.set_visible_child_name("empty");

        let zoom_osd = Label::new(None);
        zoom_osd.add_css_class("zoom-osd");
        zoom_osd.set_halign(gtk4::Align::End);
        zoom_osd.set_valign(gtk4::Align::End);
        zoom_osd.set_margin_end(12);
        zoom_osd.set_margin_bottom(12);
        zoom_osd.set_visible(false);
        zoom_osd.set_can_target(false);

        let preview_overlay = Overlay::new();
        preview_overlay.set_child(Some(&stack));
        preview_overlay.add_overlay(&zoom_osd);
        preview_overlay.set_hexpand(true);
        preview_overlay.set_vexpand(true);
        root_widget.append(&preview_overlay);

        // The zoom%/page# controls live in app_window.rs's preview_toolbar,
        // wired via on_zoom_changed/on_page_changed — this pane only owns the
        // floating auto-hide zoom_osd, to avoid two toolbars stacking here.

        // .zoom-osd CSS lives in ui::styles::load_global_css(), loaded once at app startup.

        let page_pixbufs: Rc<RefCell<Vec<Pixbuf>>> = Rc::new(RefCell::new(Vec::new()));

        // Wire up draw function
        let pixbufs_draw = page_pixbufs.clone();
        let zoom_draw: Rc<RefCell<f64>> = Rc::new(RefCell::new(1.0));
        let zoom_draw2 = zoom_draw.clone();

        drawing_area.set_draw_func(move |_area, ctx, w, _h| {
            let z = *zoom_draw.borrow();
            let pbs = pixbufs_draw.borrow();
            const PAGE_GAP: f64 = 20.0;

            // The ground the pages sit on, resolved per draw so a light/dark
            // switch is picked up immediately rather than after a restart.
            let (gr, gg, gb) = crate::ui::theme::preview_ground();
            ctx.set_source_rgb(gr, gg, gb);
            ctx.paint().ok();

            // Only paint pages that intersect the damaged region. Every page used
            // to be painted on every frame — five fills and a scaled blit each —
            // so scrolling a long document cost time proportional to its length
            // no matter how little of it was on screen.
            let (_, clip_top, _, clip_bottom) =
                ctx.clip_extents()
                    .unwrap_or((f64::MIN, f64::MIN, f64::MAX, f64::MAX));

            let mut y = 0.0f64;
            for pb in pbs.iter() {
                let pw = pb.width() as f64 * z;
                let ph = pb.height() as f64 * z;
                if y > clip_bottom {
                    break;
                }
                // The drop shadow extends a few px past the page bottom, so keep
                // drawing a page whose body has only just scrolled out of view.
                if y + ph + PAGE_GAP < clip_top {
                    y += ph + PAGE_GAP;
                    continue;
                }
                // Soft drop shadow (stacked translucent rects, darkest innermost).
                // Use pw (page width) for shadows so they don't bleed outside the
                // page when the viewport is wider than the rendered content.
                ctx.set_source_rgba(0.0, 0.0, 0.0, 0.14);
                ctx.rectangle(2.0, y + 3.0, pw, ph);
                ctx.fill().ok();
                ctx.set_source_rgba(0.0, 0.0, 0.0, 0.07);
                ctx.rectangle(3.5, y + 5.0, pw + 1.0, ph + 1.0);
                ctx.fill().ok();
                ctx.set_source_rgba(0.0, 0.0, 0.0, 0.03);
                ctx.rectangle(5.0, y + 7.0, pw + 2.0, ph + 2.0);
                ctx.fill().ok();
                // White page background fills the full viewport width so the
                // gray canvas is visible only in the gutter around pages.
                ctx.set_source_rgb(1.0, 1.0, 1.0);
                ctx.rectangle(0.0, y, pw.max(w as f64), ph);
                ctx.fill().ok();
                // Page content
                ctx.save().ok();
                ctx.scale(z, z);
                ctx.set_source_pixbuf(pb, 0.0, y / z);
                ctx.paint().ok();
                ctx.restore().ok();
                y += ph + PAGE_GAP;
            }
        });

        let on_click_jump: Rc<RefCell<Option<Box<dyn Fn(usize, f64)>>>> =
            Rc::new(RefCell::new(None));
        let on_word_click_jump: Rc<RefCell<Option<Box<dyn Fn(usize, f64, f64)>>>> =
            Rc::new(RefCell::new(None));

        // Ctrl+Click → jump to the nearby line; Double-click → jump to the exact word
        {
            let on_click_jump_c = on_click_jump.clone();
            let on_word_click_jump_c = on_word_click_jump.clone();
            let page_pixbufs_c = page_pixbufs.clone();
            let zoom_c = zoom_draw2.clone();
            let scroll_c = img_scroll.clone();
            let gesture = GestureClick::new();
            gesture.set_button(1);
            gesture.connect_pressed(move |g, n_press, x, y| {
                let state = g.current_event_state();
                let is_double = n_press == 2;
                let is_ctrl_click = state.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                if !is_double && !is_ctrl_click {
                    return;
                }
                let zoom = *zoom_c.borrow();
                let adj_val = scroll_c.vadjustment().value();
                let doc_y = y + adj_val;
                let pbs = page_pixbufs_c.borrow();
                let mut cum_y = 0.0f64;
                let mut clicked_page = pbs.len().saturating_sub(1);
                let mut clicked_rel_x = 0.5f64;
                let mut clicked_rel_y = 1.0f64;
                for (i, pb) in pbs.iter().enumerate() {
                    let raw_w = pb.width() as f64 * zoom;
                    let raw_h = pb.height() as f64 * zoom;
                    let page_h = raw_h + 20.0;
                    if doc_y < cum_y + page_h {
                        clicked_page = i;
                        clicked_rel_x = if raw_w > 0.0 {
                            (x / raw_w).clamp(0.0, 1.0)
                        } else {
                            0.5
                        };
                        clicked_rel_y = if raw_h > 0.0 {
                            ((doc_y - cum_y) / raw_h).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        break;
                    }
                    cum_y += page_h;
                }
                drop(pbs);
                if is_double {
                    if let Some(f) = on_word_click_jump_c.borrow().as_ref() {
                        f(clicked_page, clicked_rel_x, clicked_rel_y);
                    }
                } else if let Some(f) = on_click_jump_c.borrow().as_ref() {
                    f(clicked_page, clicked_rel_y);
                }
            });
            drawing_area.add_controller(gesture);
        }

        let pane = Self {
            root_widget,
            stack,
            img_scroll,
            drawing_area,
            spinner,
            cancel_btn,
            error_label,
            output_dir: Rc::new(
                output_dir.unwrap_or_else(|| PathBuf::from("/tmp/zerkalo_preview")),
            ),
            extra_args: Rc::new(extra_args),
            root_file: Rc::new(RefCell::new(root_file)),
            zoom: zoom_draw2,
            auto_fit: Rc::new(RefCell::new(true)),
            on_compile_done: Rc::new(RefCell::new(None)),
            on_compile_cancelled: Rc::new(RefCell::new(None)),
            on_compile_time: Rc::new(RefCell::new(None)),
            on_compile_start: Rc::new(RefCell::new(None)),
            spin_lbl: spin_lbl_store,
            compile_start_instant: Rc::new(RefCell::new(None)),
            on_zoom_changed: Rc::new(RefCell::new(None)),
            on_page_changed: Rc::new(RefCell::new(None)),
            on_click_jump,
            on_word_click_jump,
            page_pixbufs,
            watch_active: Rc::new(RefCell::new(false)),
            compile_gen: Rc::new(RefCell::new(0)),
            compile_in_flight: Rc::new(Cell::new(false)),
            compile_pending: Rc::new(Cell::new(false)),
            buffer_snapshot: Rc::new(RefCell::new(HashMap::new())),
            cv_elements_path: Rc::new(RefCell::new(None)),
            bib_path: Rc::new(RefCell::new(None)),
            draft_mode: Rc::new(RefCell::new(false)),
            first_load: Rc::new(RefCell::new(true)),
            zoom_osd,
            osd_timer: Rc::new(RefCell::new(None)),
            osd_fade_timer: Rc::new(RefCell::new(None)),
        };

        // Refit to width whenever the scroll viewport width changes (window resize).
        {
            let pane_r = pane.clone();
            pane.img_scroll
                .hadjustment()
                .connect_page_size_notify(move |_| {
                    if *pane_r.auto_fit.borrow() && !pane_r.page_pixbufs.borrow().is_empty() {
                        pane_r.fit_width();
                    }
                });
        }

        // Wire scroll → page-changed once here; load_pixbufs_from_bytes must NOT
        // reconnect this signal or closures accumulate O(N) across compiles.
        {
            let pane_s = pane.clone();
            pane.img_scroll
                .vadjustment()
                .connect_value_changed(move |_| {
                    pane_s.fire_page_changed();
                });
        }

        // ── Keyboard navigation for the preview pane ─────────────────────────
        // +/=  zoom in,  -  zoom out,  0  fit-to-width,  Space/Shift+Space scroll page
        {
            let pane_k = pane.clone();
            let key_ctrl = EventControllerKey::new();
            key_ctrl.connect_key_pressed(move |_, key, _, modifier| {
                use gtk4::gdk::{Key, ModifierType};
                let shift = modifier.contains(ModifierType::SHIFT_MASK);
                match key {
                    Key::plus | Key::equal => {
                        let z = (pane_k.zoom() * 1.15).min(4.0);
                        pane_k.set_zoom(z);
                        pane_k.show_zoom_osd(z);
                        return glib::Propagation::Stop;
                    }
                    Key::minus => {
                        let z = (pane_k.zoom() / 1.15).max(0.25);
                        pane_k.set_zoom(z);
                        pane_k.show_zoom_osd(z);
                        return glib::Propagation::Stop;
                    }
                    Key::_0 => {
                        pane_k.fit_width();
                        let z = pane_k.zoom();
                        pane_k.show_zoom_osd(z);
                        return glib::Propagation::Stop;
                    }
                    Key::space => {
                        let adj = pane_k.img_scroll.vadjustment();
                        let step = adj.page_size() * 0.9;
                        let new_val = if shift {
                            (adj.value() - step).max(adj.lower())
                        } else {
                            (adj.value() + step).min(adj.upper() - adj.page_size())
                        };
                        adj.set_value(new_val);
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
                glib::Propagation::Proceed
            });
            pane.img_scroll.set_focusable(true);
            pane.img_scroll.add_controller(key_ctrl);
        }

        // Wire cancel button once
        let gen_c = pane.compile_gen.clone();
        let spinner_c = pane.spinner.clone();
        let cancel_c = pane.cancel_btn.clone();
        let stack_c = pane.stack.clone();
        let pixbufs_c = pane.page_pixbufs.clone();
        let on_cancelled_c = pane.on_compile_cancelled.clone();
        pane.cancel_btn.connect_clicked(move |_| {
            *gen_c.borrow_mut() += 1;
            spinner_c.set_spinning(false);
            cancel_c.set_visible(false);
            if pixbufs_c.borrow().is_empty() {
                stack_c.set_visible_child_name("empty");
            } else {
                stack_c.set_visible_child_name("ready");
            }
            if let Some(f) = on_cancelled_c.borrow().as_ref() {
                f();
            }
        });

        pane
    }

    pub fn show_zoom_osd(&self, zoom: f64) {
        self.zoom_osd.set_text(&format!("{:.0}%", zoom * 100.0));
        self.zoom_osd.remove_css_class("osd-hidden");
        self.zoom_osd.set_visible(true);
        if let Some(id) = self.osd_timer.borrow_mut().take() {
            id.remove();
        }
        if let Some(id) = self.osd_fade_timer.borrow_mut().take() {
            id.remove();
        }
        let osd_c = self.zoom_osd.clone();
        let timer_c = self.osd_timer.clone();
        let fade_timer_c = self.osd_fade_timer.clone();
        let source =
            glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
                osd_c.add_css_class("osd-hidden");
                let osd_fade = osd_c.clone();
                let fade_timer_c2 = fade_timer_c.clone();
                let fade_source = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(220),
                    move || {
                        osd_fade.set_visible(false);
                        *fade_timer_c2.borrow_mut() = None;
                    },
                );
                *fade_timer_c.borrow_mut() = Some(fade_source);
                *timer_c.borrow_mut() = None;
            });
        *self.osd_timer.borrow_mut() = Some(source);
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root_widget
    }

    pub fn set_root_file(&self, path: PathBuf) {
        // The print cache holds a whole laid-out document; without this it
        // would outlive the file it came from once another is opened.
        crate::ui::print::invalidate_cache();
        *self.root_file.borrow_mut() = Some(path);
        *self.first_load.borrow_mut() = true;
    }

    pub fn clear_root_file(&self) {
        crate::ui::print::invalidate_cache();
        *self.root_file.borrow_mut() = None;
    }

    /// The inputs the preview itself compiles with: root file, unsaved buffer
    /// contents, and sys inputs. Anything else that compiles the same document —
    /// printing, export — must go through this rather than assembling its own,
    /// which is how printing ended up silently omitting the CV data and
    /// producing nothing at all for a CV document.
    ///
    /// Draft mode is deliberately excluded: the caller is producing final
    /// output, not a preview.
    pub fn compile_inputs(
        &self,
    ) -> Option<(
        PathBuf,
        HashMap<PathBuf, String>,
        HashMap<String, String>,
        Option<PathBuf>,
    )> {
        let root = self.root_file_path()?;
        let mut sys_inputs = HashMap::new();
        if let Some((k, v)) = self.cv_data_sys_input() {
            sys_inputs.insert(k, v);
        }
        Some((
            root,
            self.buffer_snapshot.borrow().clone(),
            sys_inputs,
            self.bib_path.borrow().clone(),
        ))
    }

    pub fn set_buffer_snapshot(&self, path: PathBuf, text: String) {
        self.buffer_snapshot.borrow_mut().insert(path, text);
    }

    /// Sets (or clears, with `None`) the CV mode data path — see
    /// `effective_cv_elements` in `app_window.rs`. Re-read fresh on every
    /// compile via `cv_data_sys_input`, not cached here.
    pub fn set_cv_elements_path(&self, path: Option<PathBuf>) {
        *self.cv_elements_path.borrow_mut() = path;
    }

    /// Sets (or clears, with `None`) the configured bibliography path, so
    /// the compile sandbox can widen to reach it when it lives outside the
    /// project — see the `bib_path` field's own doc comment.
    pub fn set_bib_path(&self, path: Option<PathBuf>) {
        *self.bib_path.borrow_mut() = path;
    }

    /// Reads `cv_elements_path` fresh (if set) and returns the
    /// `skrizhal-cv-data` sys.input entry for it, logging (not failing) on
    /// a read error so a moved/deleted CV file doesn't break compilation —
    /// it just leaves `#cv-entry`/`#cv-section` seeing no data.
    fn cv_data_sys_input(&self) -> Option<(String, String)> {
        let path = self.cv_elements_path.borrow().clone()?;
        match std::fs::read_to_string(&path) {
            Ok(yaml) => Some(("skrizhal-cv-data".to_string(), yaml)),
            Err(e) => {
                tracing::warn!("CV mode: couldn't read {}: {e}", path.display());
                None
            }
        }
    }

    pub fn set_draft_mode(&self, draft: bool) {
        *self.draft_mode.borrow_mut() = draft;
    }

    #[allow(dead_code)]
    pub fn is_draft_mode(&self) -> bool {
        *self.draft_mode.borrow()
    }

    pub fn output_dir(&self) -> PathBuf {
        (*self.output_dir).clone()
    }

    pub fn root_file_path(&self) -> Option<PathBuf> {
        self.root_file.borrow().clone()
    }

    pub fn extra_args(&self) -> Vec<String> {
        (*self.extra_args).clone()
    }

    pub fn zoom(&self) -> f64 {
        *self.zoom.borrow()
    }

    pub fn set_zoom(&self, z: f64) {
        // Capture the content fraction at the vertical centre of the viewport
        // before resizing, so we can restore it after.
        let v_frac = {
            let adj = self.img_scroll.vadjustment();
            let range = adj.upper() - adj.lower();
            let page = adj.page_size();
            if range > page {
                Some((adj.value() + page / 2.0) / range)
            } else {
                None
            }
        };

        *self.zoom.borrow_mut() = z.clamp(0.25, 4.0);
        self.refit_drawing_area_centered(v_frac);

        let actual = *self.zoom.borrow();
        if let Some(f) = self.on_zoom_changed.borrow().as_ref() {
            f(actual);
        }
    }

    pub fn fit_width(&self) {
        *self.auto_fit.borrow_mut() = true;
        let scroll_w = self.img_scroll.allocated_width() as f64;
        let pb_w = self
            .page_pixbufs
            .borrow()
            .first()
            .map(|pb| pb.width() as f64)
            .unwrap_or(0.0);
        if pb_w > 0.0 && scroll_w > 16.0 {
            // Don't call set_zoom here — that would set auto_fit = false.
            let z = ((scroll_w - 16.0) / pb_w).clamp(0.25, 4.0);
            *self.zoom.borrow_mut() = z;
            self.refit_drawing_area_centered(None);
            if let Some(f) = self.on_zoom_changed.borrow().as_ref() {
                f(z);
            }
        }
    }

    pub fn fit_page(&self) {
        *self.auto_fit.borrow_mut() = false;
        let scroll_w = self.img_scroll.allocated_width() as f64;
        let scroll_h = self.img_scroll.allocated_height() as f64;
        let pbs = self.page_pixbufs.borrow();
        let pb_w = pbs.first().map(|pb| pb.width() as f64).unwrap_or(0.0);
        let pb_h = pbs.first().map(|pb| pb.height() as f64).unwrap_or(0.0);
        drop(pbs);
        if pb_w > 0.0 && pb_h > 0.0 && scroll_w > 16.0 && scroll_h > 16.0 {
            let z = ((scroll_w - 16.0) / pb_w).min((scroll_h - 16.0) / pb_h);
            self.set_zoom(z);
        }
    }

    pub fn set_on_compile_done(&self, f: impl Fn(Option<String>, String) + 'static) {
        *self.on_compile_done.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_compile_cancelled(&self, f: impl Fn() + 'static) {
        *self.on_compile_cancelled.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_compile_time(&self, f: impl Fn(u64, Option<usize>) + 'static) {
        *self.on_compile_time.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_compile_start(&self, f: impl Fn() + 'static) {
        *self.on_compile_start.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_zoom_changed(&self, f: impl Fn(f64) + 'static) {
        *self.on_zoom_changed.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_page_changed(&self, f: impl Fn(usize, usize) + 'static) {
        *self.on_page_changed.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_click_jump(&self, f: impl Fn(usize, f64) + 'static) {
        *self.on_click_jump.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_word_click_jump(&self, f: impl Fn(usize, f64, f64) + 'static) {
        *self.on_word_click_jump.borrow_mut() = Some(Box::new(f));
    }

    #[allow(dead_code)]
    pub fn fire_jump_to_current_page(&self) {
        let page = self.current_page_idx();
        if let Some(f) = self.on_click_jump.borrow().as_ref() {
            f(page, 0.45);
        }
    }

    pub fn page_count(&self) -> usize {
        self.page_pixbufs.borrow().len()
    }

    pub fn current_page_idx(&self) -> usize {
        let z = *self.zoom.borrow();
        let adj = self.img_scroll.vadjustment();
        let mid_y = adj.value() + adj.page_size() / 2.0;
        let pbs = self.page_pixbufs.borrow();
        let mut y = 0.0f64;
        for (i, pb) in pbs.iter().enumerate() {
            let page_h = pb.height() as f64 * z + 20.0;
            if mid_y < y + page_h {
                return i;
            }
            y += page_h;
        }
        pbs.len().saturating_sub(1)
    }

    #[allow(dead_code)] // kept alongside the other scroll helpers
    pub fn scroll_to_fraction(&self, frac: f64) {
        let adj = self.img_scroll.vadjustment();
        let range = adj.upper() - adj.lower() - adj.page_size();
        if range > 0.0 {
            adj.set_value(frac.clamp(0.0, 1.0) * range + adj.lower());
        }
    }

    pub fn scroll_to_page(&self, idx: usize) {
        let z = *self.zoom.borrow();
        let pbs = self.page_pixbufs.borrow();
        let mut y = 0.0f64;
        for (i, pb) in pbs.iter().enumerate() {
            if i == idx {
                break;
            }
            y += pb.height() as f64 * z + 20.0;
        }
        drop(pbs);
        self.img_scroll.vadjustment().set_value(y);
        self.fire_page_changed();
    }

    fn fire_page_changed(&self) {
        let current = self.current_page_idx();
        let total = self.page_count();
        if total > 0 {
            if let Some(f) = self.on_page_changed.borrow().as_ref() {
                f(current, total);
            }
        }
    }

    // ── Watch mode ────────────────────────────────────────────────────────────

    #[allow(dead_code)]
    pub fn start_watch(&self) {
        self.stop_watch();
        if self.root_file.borrow().is_none() {
            return;
        }
        *self.watch_active.borrow_mut() = true;
        self.trigger_compile();

        let root_file_rc = self.root_file.clone();
        let last_mtime: Rc<RefCell<Option<std::time::SystemTime>>> =
            Rc::new(RefCell::new(root_file_rc.borrow().as_ref().and_then(|p| {
                std::fs::metadata(p).and_then(|m| m.modified()).ok()
            })));

        let pane = self.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            if !*pane.watch_active.borrow() {
                return glib::ControlFlow::Break;
            }
            let current_mtime = root_file_rc
                .borrow()
                .as_ref()
                .and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
            let changed = match (*last_mtime.borrow(), current_mtime) {
                (Some(old), Some(new)) => old != new,
                (None, Some(_)) => true,
                _ => false,
            };
            if changed {
                *last_mtime.borrow_mut() = current_mtime;
                pane.trigger_compile();
            }
            glib::ControlFlow::Continue
        });
    }

    #[allow(dead_code)]
    pub fn stop_watch(&self) {
        *self.watch_active.borrow_mut() = false;
    }

    #[allow(dead_code)]
    pub fn is_watching(&self) -> bool {
        *self.watch_active.borrow()
    }

    // ── Compile ───────────────────────────────────────────────────────────────

    pub fn trigger_compile(&self) {
        let root = match self.root_file.borrow().clone() {
            Some(f) => f,
            None => {
                self.error_label
                    .set_label("No root file detected.\nCreate a main.typ file.");
                self.stack.set_visible_child_name("error");
                return;
            }
        };

        if self.compile_in_flight.get() {
            self.compile_pending.set(true);
            return;
        }

        *self.compile_gen.borrow_mut() += 1;
        let my_gen = *self.compile_gen.borrow();
        let gen_rc = self.compile_gen.clone();
        self.compile_in_flight.set(true);

        if let Some(f) = self.on_compile_start.borrow().as_ref() {
            f();
        }
        self.spinner.set_spinning(true);
        self.cancel_btn.set_visible(false);
        self.stack.set_visible_child_name("compiling");
        self.spin_lbl.set_text("Compiling\u{2026}");
        *self.compile_start_instant.borrow_mut() = Some(Instant::now());
        {
            let lbl = self.spin_lbl.clone();
            let start_rc = self.compile_start_instant.clone();
            let gen_for_spin = self.compile_gen.clone();
            let spin_gen = my_gen;
            glib::timeout_add_local(Duration::from_millis(500), move || {
                if *gen_for_spin.borrow() != spin_gen {
                    return glib::ControlFlow::Break;
                }
                if let Some(t) = *start_rc.borrow() {
                    let secs = t.elapsed().as_secs();
                    lbl.set_text(&format!("Compiling\u{2026} {secs}s"));
                }
                glib::ControlFlow::Continue
            });
        }

        // Show cancel button after 2 seconds
        let cancel_c = self.cancel_btn.clone();
        let gen_for_cancel = self.compile_gen.clone();
        glib::timeout_add_local_once(Duration::from_secs(2), move || {
            if *gen_for_cancel.borrow() == my_gen {
                cancel_c.set_visible(true);
            }
        });

        let (tx, rx) = mpsc::sync_channel::<CompileResult>(1);

        let bib_path = self.bib_path.borrow().clone();
        let snapshots = self.buffer_snapshot.borrow().clone();
        let draft = *self.draft_mode.borrow();
        let pixel_per_pt = if draft { 1.0f32 } else { 2.0f32 };
        let mut sys_inputs = std::collections::HashMap::new();
        if draft {
            sys_inputs.insert("draft".to_string(), "true".to_string());
        }
        if let Some((k, v)) = self.cv_data_sys_input() {
            sys_inputs.insert(k, v);
        }
        std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            let result = crate::compiler::compile_to_rgba_pages(
                &root,
                pixel_per_pt,
                &snapshots,
                &sys_inputs,
                bib_path.as_deref(),
            );
            let elapsed = t0.elapsed();
            tx.send(match result {
                Ok((pages, warnings)) => CompileResult::Success(pages, warnings, elapsed),
                Err(msg) => CompileResult::Error(msg, elapsed),
            })
            .ok();
        });

        let rx = Rc::new(rx);
        let pane = self.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if *gen_rc.borrow() != my_gen {
                // Cancelled. The worker thread can't be stopped and is still
                // running, but nothing is waiting on it now, and a cancel is a
                // deliberate "stop" — so drop any queued request rather than
                // starting one immediately.
                pane.spinner.set_spinning(false);
                pane.compile_in_flight.set(false);
                pane.compile_pending.set(false);
                return glib::ControlFlow::Break;
            }
            match rx.try_recv() {
                Ok(result) => {
                    pane.spinner.set_spinning(false);
                    pane.cancel_btn.set_visible(false);
                    match result {
                        CompileResult::Success(pages, warnings, elapsed) => {
                            pane.load_pixbufs_from_pages(pages);
                            pane.stack.set_visible_child_name("ready");
                            let page_count = pane.page_count();
                            if let Some(f) = pane.on_compile_done.borrow().as_ref() {
                                f(None, warnings);
                            }
                            if let Some(f) = pane.on_compile_time.borrow().as_ref() {
                                f(elapsed.as_millis() as u64, Some(page_count));
                            }
                        }
                        CompileResult::Error(msg, elapsed) => {
                            pane.error_label.set_label(&msg);
                            pane.stack.set_visible_child_name("error");
                            if let Some(f) = pane.on_compile_done.borrow().as_ref() {
                                f(Some(msg), String::new());
                            }
                            if let Some(f) = pane.on_compile_time.borrow().as_ref() {
                                f(elapsed.as_millis() as u64, None);
                            }
                        }
                    }
                    pane.compile_in_flight.set(false);
                    if pane.compile_pending.replace(false) {
                        pane.trigger_compile();
                    }
                    glib::ControlFlow::Break
                }
                Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => {
                    pane.spinner.set_spinning(false);
                    pane.cancel_btn.set_visible(false);
                    pane.compile_in_flight.set(false);
                    if pane.compile_pending.replace(false) {
                        pane.trigger_compile();
                    }
                    glib::ControlFlow::Break
                }
            }
        });
    }

    pub fn refresh_display(&self) {
        self.trigger_compile();
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    /// Wrap already-rendered RGBA pages as pixbufs. `Pixbuf::from_bytes` takes
    /// ownership of the buffer without copying or decoding, so this is cheap
    /// regardless of page count — the old version decoded a PNG per page here,
    /// on the main thread, after every compile.
    fn load_pixbufs_from_pages(&self, pages: Vec<crate::compiler::RenderedPage>) {
        let is_first = *self.first_load.borrow();

        let pixbufs: Vec<Pixbuf> = pages
            .into_iter()
            .map(|p| {
                let rowstride = (p.width * 4) as i32;
                Pixbuf::from_bytes(
                    &glib::Bytes::from_owned(p.rgba),
                    gtk4::gdk_pixbuf::Colorspace::Rgb,
                    true,
                    8,
                    p.width as i32,
                    p.height as i32,
                    rowstride,
                )
            })
            .collect();
        *self.page_pixbufs.borrow_mut() = pixbufs;

        if is_first {
            self.refit_drawing_area_centered(None);
            *self.first_load.borrow_mut() = false;
            if *self.auto_fit.borrow() {
                let pane = self.clone();
                glib::idle_add_local_once(move || {
                    pane.fit_width();
                });
            }
        } else if *self.auto_fit.borrow() {
            // Compute the correct zoom synchronously before the single redraw so
            // there is never an intermediate frame rendered at the old zoom level
            // (which caused the shadow at the bottom of the last page to flicker).
            let scroll_w = self.img_scroll.allocated_width() as f64;
            let pb_w = self
                .page_pixbufs
                .borrow()
                .first()
                .map(|pb| pb.width() as f64)
                .unwrap_or(0.0);
            if pb_w > 0.0 && scroll_w > 16.0 {
                let z = ((scroll_w - 16.0) / pb_w).clamp(0.25, 4.0);
                *self.zoom.borrow_mut() = z;
                if let Some(f) = self.on_zoom_changed.borrow().as_ref() {
                    f(z);
                }
            }
            // Don't restore scroll position by fraction here — recompiles happen on
            // every keystroke, and the document's total height changes as the user
            // types, so a fraction-based restore visibly drifts the viewport even
            // though the user never scrolled. Leaving the adjustment untouched keeps
            // the same pixel offset (GTK clamps it automatically if content shrank).
            self.refit_drawing_area_centered(None);
        } else {
            self.refit_drawing_area_centered(None);
        }

        self.fire_page_changed();
    }

    #[allow(dead_code)]
    fn refit_drawing_area(&self) {
        self.refit_drawing_area_centered(None);
    }

    fn refit_drawing_area_centered(&self, v_frac: Option<f64>) {
        let z = *self.zoom.borrow();
        let pbs = self.page_pixbufs.borrow();
        let mut total_h = 0i32;
        let mut max_w = 0i32;
        for pb in pbs.iter() {
            let w = (pb.width() as f64 * z).round() as i32;
            let h = (pb.height() as f64 * z).round() as i32;
            max_w = max_w.max(w);
            total_h += h + 20;
        }
        drop(pbs);
        self.drawing_area.set_content_width(max_w.max(1));
        self.drawing_area.set_content_height(total_h.max(1));
        self.drawing_area.queue_draw();
        if let Some(frac) = v_frac {
            let scroll = self.img_scroll.clone();
            glib::idle_add_local_once(move || {
                let adj = scroll.vadjustment();
                let range = adj.upper() - adj.lower();
                let page = adj.page_size();
                if range > page {
                    let target = (range * frac - page / 2.0).clamp(adj.lower(), adj.upper() - page);
                    adj.set_value(target);
                }
            });
        }
    }
}

/// Inputs `ensure_pdf_path`/pdftotext need, gathered on the main thread from
/// `PreviewPane`'s `Rc<RefCell<...>>` fields (not `Send`) before handing off
/// to the background thread that does the actual (potentially slow) work.
struct PdfTextInputs {
    root: PathBuf,
    output_dir: PathBuf,
    snapshots: HashMap<PathBuf, String>,
    sys_inputs: HashMap<String, String>,
    bib_path: Option<PathBuf>,
}

fn gather_pdf_text_inputs(pane: &PreviewPane) -> Option<PdfTextInputs> {
    let root = pane.root_file.borrow().clone()?;
    let mut sys_inputs = HashMap::new();
    if let Some((k, v)) = pane.cv_data_sys_input() {
        sys_inputs.insert(k, v);
    }
    Some(PdfTextInputs {
        root,
        output_dir: pane.output_dir(),
        snapshots: pane.buffer_snapshot.borrow().clone(),
        sys_inputs,
        bib_path: pane.bib_path.borrow().clone(),
    })
}

fn ensure_pdf_path(inputs: &PdfTextInputs) -> Option<PathBuf> {
    let stem = inputs.root.file_stem()?.to_str()?.to_string();
    let pdf_path = inputs.output_dir.join(format!("{stem}.pdf"));
    if !pdf_path.exists() {
        let bytes = crate::compiler::compile_to_pdf_bytes(
            &inputs.root,
            &inputs.snapshots,
            &inputs.sys_inputs,
            inputs.bib_path.as_deref(),
        )
        .ok()?;
        std::fs::write(&pdf_path, bytes).ok()?;
    }
    Some(pdf_path)
}

/// Runs `ensure_pdf_path` + `pdftotext -layout` on a background thread (this
/// can compile the whole document if the PDF isn't cached yet, which is slow
/// enough on large documents to freeze the UI if run inline) and delivers the
/// extracted page text to `on_done` on the main thread via the same
/// spawn-thread/channel/`timeout_add_local` pattern used by `do_sync`.
pub fn extract_page_text_via_pdftotext_async(
    pane: &PreviewPane,
    page: usize,
    on_done: impl FnOnce(Option<String>) + 'static,
) {
    let Some(inputs) = gather_pdf_text_inputs(pane) else {
        on_done(None);
        return;
    };

    let (tx, rx) = mpsc::sync_channel::<Option<String>>(1);
    std::thread::spawn(move || {
        let result = (|| {
            let pdf_path = ensure_pdf_path(&inputs)?;
            let page_str = (page + 1).to_string();
            let out = crate::git_sync::host_command("pdftotext")
                .args([
                    "-layout",
                    "-f",
                    &page_str,
                    "-l",
                    &page_str,
                    pdf_path.to_str().unwrap_or(""),
                    "-",
                ])
                .output()
                .ok()?;
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).to_string())
            } else {
                None
            }
        })();
        tx.send(result).ok();
    });

    poll_pdf_text_result(rx, on_done);
}

/// Shared poll loop for the two `*_async` pdftotext extractors above.
fn poll_pdf_text_result(
    rx: mpsc::Receiver<Option<String>>,
    on_done: impl FnOnce(Option<String>) + 'static,
) {
    let rx = Rc::new(rx);
    let on_done = Rc::new(RefCell::new(Some(on_done)));
    glib::timeout_add_local(Duration::from_millis(50), move || match rx.try_recv() {
        Ok(result) => {
            if let Some(f) = on_done.borrow_mut().take() {
                f(result);
            }
            glib::ControlFlow::Break
        }
        Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(TryRecvError::Disconnected) => {
            if let Some(f) = on_done.borrow_mut().take() {
                f(None);
            }
            glib::ControlFlow::Break
        }
    });
}

struct PdfWord {
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
    text: String,
}

fn unescape_xml(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Runs `pdftotext -bbox` on a background thread and finds the word nearest
/// the given fractional (rel_x, rel_y) click position, delivering it together
/// with its immediate neighbors as a short phrase (specific enough to
/// disambiguate a single common word when searched for in the source buffer)
/// to `on_done` on the main thread. Same async shape as
/// `extract_page_text_via_pdftotext_async` — see its doc comment.
pub fn extract_word_at_position_async(
    pane: &PreviewPane,
    page: usize,
    rel_x: f64,
    rel_y: f64,
    on_done: impl FnOnce(Option<String>) + 'static,
) {
    let Some(inputs) = gather_pdf_text_inputs(pane) else {
        on_done(None);
        return;
    };

    let (tx, rx) = mpsc::sync_channel::<Option<String>>(1);
    std::thread::spawn(move || {
        let result = (|| {
            let pdf_path = ensure_pdf_path(&inputs)?;
            let page_str = (page + 1).to_string();
            let out = crate::git_sync::host_command("pdftotext")
                .args([
                    "-bbox",
                    "-f",
                    &page_str,
                    "-l",
                    &page_str,
                    pdf_path.to_str().unwrap_or(""),
                    "-",
                ])
                .output()
                .ok()?;
            if !out.status.success() {
                return None;
            }
            let xml = String::from_utf8_lossy(&out.stdout);
            word_at_position_from_bbox_xml(&xml, rel_x, rel_y)
        })();
        tx.send(result).ok();
    });

    poll_pdf_text_result(rx, on_done);
}

fn word_at_position_from_bbox_xml(xml: &str, rel_x: f64, rel_y: f64) -> Option<String> {
    let page_re = regex::Regex::new(r#"<page width="([0-9.]+)" height="([0-9.]+)">"#).ok()?;
    let caps = page_re.captures(xml)?;
    let page_w: f64 = caps[1].parse().ok()?;
    let page_h: f64 = caps[2].parse().ok()?;

    let word_re = regex::Regex::new(
        r#"<word xMin="([0-9.]+)" yMin="([0-9.]+)" xMax="([0-9.]+)" yMax="([0-9.]+)">([^<]*)</word>"#,
    ).ok()?;
    let words: Vec<PdfWord> = word_re
        .captures_iter(xml)
        .filter_map(|c| {
            Some(PdfWord {
                x_min: c[1].parse().ok()?,
                y_min: c[2].parse().ok()?,
                x_max: c[3].parse().ok()?,
                y_max: c[4].parse().ok()?,
                text: unescape_xml(&c[5]),
            })
        })
        .filter(|w| !w.text.trim().is_empty())
        .collect();
    if words.is_empty() {
        return None;
    }

    let px = rel_x * page_w;
    let py = rel_y * page_h;
    let target_idx = words
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let da = bbox_distance(a, px, py);
            let db = bbox_distance(b, px, py);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)?;

    let mut phrase_words = Vec::new();
    if target_idx > 0 {
        phrase_words.push(words[target_idx - 1].text.as_str());
    }
    phrase_words.push(words[target_idx].text.as_str());
    if target_idx + 1 < words.len() {
        phrase_words.push(words[target_idx + 1].text.as_str());
    }
    Some(phrase_words.join(" "))
}

fn bbox_distance(w: &PdfWord, px: f64, py: f64) -> f64 {
    let dx = if px < w.x_min {
        w.x_min - px
    } else if px > w.x_max {
        px - w.x_max
    } else {
        0.0
    };
    let dy = if py < w.y_min {
        w.y_min - py
    } else if py > w.y_max {
        py - w.y_max
    } else {
        0.0
    };
    dx * dx + dy * dy
}

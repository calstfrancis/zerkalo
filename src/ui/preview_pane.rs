use std::cell::RefCell;
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
    Orientation, Overlay, ScrolledWindow, Separator, Spinner, Stack,
};
use std::time::Instant;

// ── Result sent from compile thread ──────────────────────────────────────────

enum CompileResult {
    Success(Vec<Vec<u8>>, std::time::Duration),
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
    on_compile_done: Rc<RefCell<Option<Box<dyn Fn(Option<String>)>>>>,
    on_compile_time: Rc<RefCell<Option<Box<dyn Fn(u64, Option<usize>)>>>>,
    on_compile_start: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    spin_lbl: Label,
    compile_start_instant: Rc<RefCell<Option<Instant>>>,
    on_zoom_changed: Rc<RefCell<Option<Box<dyn Fn(f64)>>>>,
    on_page_changed: Rc<RefCell<Option<Box<dyn Fn(usize, usize)>>>>,
    on_click_jump: Rc<RefCell<Option<Box<dyn Fn(usize, f64)>>>>,
    page_pixbufs: Rc<RefCell<Vec<Pixbuf>>>,
    watch_active: Rc<RefCell<bool>>,
    compile_gen: Rc<RefCell<u64>>,
    buffer_snapshot: Rc<RefCell<HashMap<PathBuf, String>>>,
    draft_mode: Rc<RefCell<bool>>,
    first_load: Rc<RefCell<bool>>,
    zoom_osd: Label,
    osd_timer: Rc<RefCell<Option<glib::SourceId>>>,
    zoom_label: Label,
    page_label: Label,
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

        // ── Zoom control bar ──────────────────────────────────────────────────
        let zoom_bar = GtkBox::new(Orientation::Horizontal, 0);
        zoom_bar.set_hexpand(true);

        let page_label = Label::new(Some(""));
        page_label.add_css_class("caption");
        page_label.add_css_class("dim-label");
        page_label.set_margin_start(8);
        page_label.set_margin_end(4);
        page_label.set_visible(false);
        zoom_bar.append(&page_label);

        let zoom_spacer = GtkBox::new(Orientation::Horizontal, 0);
        zoom_spacer.set_hexpand(true);
        zoom_bar.append(&zoom_spacer);

        let zoom_minus_btn = Button::with_label("\u{2212}");
        zoom_minus_btn.add_css_class("flat");
        zoom_minus_btn.add_css_class("caption");
        zoom_minus_btn.set_tooltip_text(Some("Zoom out (−10%)"));
        zoom_minus_btn.set_margin_top(2);
        zoom_minus_btn.set_margin_bottom(2);
        zoom_bar.append(&zoom_minus_btn);

        let zoom_label = Label::new(Some("100%"));
        zoom_label.add_css_class("caption");
        zoom_label.add_css_class("dim-label");
        zoom_label.set_width_chars(5);
        zoom_label.set_xalign(0.5);
        zoom_label.set_margin_start(2);
        zoom_label.set_margin_end(2);
        zoom_bar.append(&zoom_label);

        let zoom_plus_btn = Button::with_label("+");
        zoom_plus_btn.add_css_class("flat");
        zoom_plus_btn.add_css_class("caption");
        zoom_plus_btn.set_tooltip_text(Some("Zoom in (+10%)"));
        zoom_plus_btn.set_margin_top(2);
        zoom_plus_btn.set_margin_bottom(2);
        zoom_bar.append(&zoom_plus_btn);

        let zoom_sep = Separator::new(Orientation::Horizontal);
        root_widget.append(&zoom_sep);
        root_widget.append(&zoom_bar);

        let css_provider = gtk4::CssProvider::new();
        css_provider.load_from_data(
            ".zoom-osd { background: alpha(@window_bg_color, 0.85); \
             border-radius: 6px; padding: 4px 10px; \
             font-size: 0.85em; font-weight: bold; \
             box-shadow: 0 1px 4px alpha(black, 0.3); }"
        );
        gtk4::style_context_add_provider_for_display(
            &gtk4::gdk::Display::default().unwrap(),
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let page_pixbufs: Rc<RefCell<Vec<Pixbuf>>> = Rc::new(RefCell::new(Vec::new()));

        // Wire up draw function
        let pixbufs_draw = page_pixbufs.clone();
        let zoom_draw: Rc<RefCell<f64>> = Rc::new(RefCell::new(1.0));
        let zoom_draw2 = zoom_draw.clone();

        drawing_area.set_draw_func(move |_area, ctx, w, _h| {
            let z = *zoom_draw.borrow();
            let pbs = pixbufs_draw.borrow();
            const PAGE_GAP: f64 = 20.0;

            // Light gray canvas background (visible between pages)
            ctx.set_source_rgb(0.82, 0.82, 0.82);
            ctx.paint().ok();

            let mut y = 0.0f64;
            for pb in pbs.iter() {
                let pw = pb.width() as f64 * z;
                let ph = pb.height() as f64 * z;
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

        // Ctrl+Click → click-to-jump callback
        {
            let on_click_jump_c = on_click_jump.clone();
            let page_pixbufs_c = page_pixbufs.clone();
            let zoom_c = zoom_draw2.clone();
            let scroll_c = img_scroll.clone();
            let gesture = GestureClick::new();
            gesture.set_button(1);
            gesture.connect_pressed(move |g, _n, x, y| {
                let state = g.current_event_state();
                if !state.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
                    return;
                }
                let _ = x;
                let zoom = *zoom_c.borrow();
                let adj_val = scroll_c.vadjustment().value();
                let doc_y = y + adj_val;
                let pbs = page_pixbufs_c.borrow();
                let mut cum_y = 0.0f64;
                let mut clicked_page = pbs.len().saturating_sub(1);
                let mut clicked_rel_y = 1.0f64;
                for (i, pb) in pbs.iter().enumerate() {
                    let raw_h = pb.height() as f64 * zoom;
                    let page_h = raw_h + 20.0;
                    if doc_y < cum_y + page_h {
                        clicked_page = i;
                        clicked_rel_y = if raw_h > 0.0 { ((doc_y - cum_y) / raw_h).clamp(0.0, 1.0) } else { 0.0 };
                        break;
                    }
                    cum_y += page_h;
                }
                drop(pbs);
                if let Some(f) = on_click_jump_c.borrow().as_ref() {
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
            on_compile_time: Rc::new(RefCell::new(None)),
            on_compile_start: Rc::new(RefCell::new(None)),
            spin_lbl: spin_lbl_store,
            compile_start_instant: Rc::new(RefCell::new(None)),
            on_zoom_changed: Rc::new(RefCell::new(None)),
            on_page_changed: Rc::new(RefCell::new(None)),
            on_click_jump,
            page_pixbufs,
            watch_active: Rc::new(RefCell::new(false)),
            compile_gen: Rc::new(RefCell::new(0)),
            buffer_snapshot: Rc::new(RefCell::new(HashMap::new())),
            draft_mode: Rc::new(RefCell::new(false)),
            first_load: Rc::new(RefCell::new(true)),
            zoom_osd,
            osd_timer: Rc::new(RefCell::new(None)),
            zoom_label: zoom_label.clone(),
            page_label: page_label.clone(),
        };

        // Refit to width whenever the scroll viewport width changes (window resize).
        {
            let pane_r = pane.clone();
            pane.img_scroll.hadjustment().connect_page_size_notify(move |_| {
                if *pane_r.auto_fit.borrow() && !pane_r.page_pixbufs.borrow().is_empty() {
                    pane_r.fit_width();
                }
            });
        }

        // Wire scroll → page-changed once here; load_pixbufs_from_bytes must NOT
        // reconnect this signal or closures accumulate O(N) across compiles.
        {
            let pane_s = pane.clone();
            pane.img_scroll.vadjustment().connect_value_changed(move |_| {
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

        // ── Zoom step buttons ─────────────────────────────────────────────────
        {
            let pane_minus = pane.clone();
            zoom_minus_btn.connect_clicked(move |_| {
                let z = (pane_minus.zoom() - 0.10).max(0.25);
                let z = (z * 100.0).round() / 100.0;
                pane_minus.set_zoom(z);
                pane_minus.update_zoom_label(z);
            });
        }
        {
            let pane_plus = pane.clone();
            zoom_plus_btn.connect_clicked(move |_| {
                let z = (pane_plus.zoom() + 0.10).min(4.0);
                let z = (z * 100.0).round() / 100.0;
                pane_plus.set_zoom(z);
                pane_plus.update_zoom_label(z);
            });
        }

        // Wire cancel button once
        let gen_c = pane.compile_gen.clone();
        let spinner_c = pane.spinner.clone();
        let cancel_c = pane.cancel_btn.clone();
        let stack_c = pane.stack.clone();
        let pixbufs_c = pane.page_pixbufs.clone();
        pane.cancel_btn.connect_clicked(move |_| {
            *gen_c.borrow_mut() += 1;
            spinner_c.set_spinning(false);
            cancel_c.set_visible(false);
            if pixbufs_c.borrow().is_empty() {
                stack_c.set_visible_child_name("empty");
            } else {
                stack_c.set_visible_child_name("ready");
            }
        });

        pane
    }

    fn update_zoom_label(&self, zoom: f64) {
        self.zoom_label.set_text(&format!("{:.0}%", zoom * 100.0));
    }

    fn show_zoom_osd(&self, zoom: f64) {
        self.update_zoom_label(zoom);
        self.zoom_osd.set_text(&format!("{:.0}%", zoom * 100.0));
        self.zoom_osd.set_visible(true);
        if let Some(id) = self.osd_timer.borrow_mut().take() {
            id.remove();
        }
        let osd_c = self.zoom_osd.clone();
        let timer_c = self.osd_timer.clone();
        let source = glib::timeout_add_local_once(
            std::time::Duration::from_millis(1500),
            move || {
                osd_c.set_visible(false);
                *timer_c.borrow_mut() = None;
            },
        );
        *self.osd_timer.borrow_mut() = Some(source);
    }

    pub fn widget(&self) -> &GtkBox {
        &self.root_widget
    }

    pub fn set_root_file(&self, path: PathBuf) {
        *self.root_file.borrow_mut() = Some(path);
        *self.first_load.borrow_mut() = true;
    }

    pub fn clear_root_file(&self) {
        *self.root_file.borrow_mut() = None;
    }

    pub fn set_buffer_snapshot(&self, path: PathBuf, text: String) {
        self.buffer_snapshot.borrow_mut().insert(path, text);
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
        self.update_zoom_label(actual);
        if let Some(f) = self.on_zoom_changed.borrow().as_ref() {
            f(actual);
        }
    }

    pub fn fit_width(&self) {
        *self.auto_fit.borrow_mut() = true;
        let scroll_w = self.img_scroll.allocated_width() as f64;
        let pb_w = self.page_pixbufs.borrow().first()
            .map(|pb| pb.width() as f64)
            .unwrap_or(0.0);
        if pb_w > 0.0 && scroll_w > 16.0 {
            // Don't call set_zoom here — that would set auto_fit = false.
            let z = ((scroll_w - 16.0) / pb_w).clamp(0.25, 4.0);
            *self.zoom.borrow_mut() = z;
            self.refit_drawing_area_centered(None);
            self.update_zoom_label(z);
            if let Some(f) = self.on_zoom_changed.borrow().as_ref() { f(z); }
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

    pub fn set_on_compile_done(&self, f: impl Fn(Option<String>) + 'static) {
        *self.on_compile_done.borrow_mut() = Some(Box::new(f));
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
            if i == idx { break; }
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
            self.page_label.set_text(&format!("Page {} / {}", current + 1, total));
            self.page_label.set_visible(true);
            if let Some(f) = self.on_page_changed.borrow().as_ref() {
                f(current, total);
            }
        } else {
            self.page_label.set_visible(false);
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
            Rc::new(RefCell::new(
                root_file_rc.borrow().as_ref().and_then(|p| {
                    std::fs::metadata(p).and_then(|m| m.modified()).ok()
                }),
            ));

        let pane = self.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            if !*pane.watch_active.borrow() {
                return glib::ControlFlow::Break;
            }
            let current_mtime = root_file_rc.borrow().as_ref().and_then(|p| {
                std::fs::metadata(p).and_then(|m| m.modified()).ok()
            });
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

        *self.compile_gen.borrow_mut() += 1;
        let my_gen = *self.compile_gen.borrow();
        let gen_rc = self.compile_gen.clone();

        if let Some(f) = self.on_compile_start.borrow().as_ref() { f(); }
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

        let snapshots = self.buffer_snapshot.borrow().clone();
        let draft = *self.draft_mode.borrow();
        let pixel_per_pt = if draft { 1.0f32 } else { 2.0f32 };
        let mut sys_inputs = std::collections::HashMap::new();
        if draft {
            sys_inputs.insert("draft".to_string(), "true".to_string());
        }
        std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            let result = crate::compiler::compile_to_png_bytes(&root, pixel_per_pt, &snapshots, &sys_inputs);
            let elapsed = t0.elapsed();
            tx.send(match result {
                Ok(pages) => CompileResult::Success(pages, elapsed),
                Err(msg) => CompileResult::Error(msg, elapsed),
            })
            .ok();
        });

        let rx = Rc::new(rx);
        let pane = self.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if *gen_rc.borrow() != my_gen {
                pane.spinner.set_spinning(false);
                return glib::ControlFlow::Break;
            }
            match rx.try_recv() {
                Ok(result) => {
                    pane.spinner.set_spinning(false);
                    pane.cancel_btn.set_visible(false);
                    match result {
                        CompileResult::Success(pages, elapsed) => {
                            pane.load_pixbufs_from_bytes(&pages);
                            pane.stack.set_visible_child_name("ready");
                            let page_count = pane.page_count();
                            if let Some(f) = pane.on_compile_done.borrow().as_ref() {
                                f(None);
                            }
                            if let Some(f) = pane.on_compile_time.borrow().as_ref() {
                                f(elapsed.as_millis() as u64, Some(page_count));
                            }
                        }
                        CompileResult::Error(msg, elapsed) => {
                            pane.error_label.set_label(&msg);
                            pane.stack.set_visible_child_name("error");
                            if let Some(f) = pane.on_compile_done.borrow().as_ref() {
                                f(Some(msg));
                            }
                            if let Some(f) = pane.on_compile_time.borrow().as_ref() {
                                f(elapsed.as_millis() as u64, None);
                            }
                        }
                    }
                    glib::ControlFlow::Break
                }
                Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => {
                    pane.spinner.set_spinning(false);
                    pane.cancel_btn.set_visible(false);
                    glib::ControlFlow::Break
                }
            }
        });

    }

    pub fn refresh_display(&self) {
        self.trigger_compile();
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    fn load_pixbufs_from_bytes(&self, pages: &[Vec<u8>]) {
        let is_first = *self.first_load.borrow();

        // Capture vertical scroll fraction before replacing content (recompiles only).
        let saved_v_frac: Option<f64> = if !is_first {
            let adj = self.img_scroll.vadjustment();
            let range = adj.upper() - adj.lower();
            let page = adj.page_size();
            if range > page {
                Some((adj.value() + page / 2.0) / range)
            } else {
                None
            }
        } else {
            None
        };

        let mut pixbufs = Vec::new();
        for png_bytes in pages {
            let gbytes = glib::Bytes::from(png_bytes.as_slice());
            let stream = gtk4::gio::MemoryInputStream::from_bytes(&gbytes);
            match Pixbuf::from_stream(&stream, None::<&gtk4::gio::Cancellable>) {
                Ok(pb) => pixbufs.push(pb),
                Err(e) => tracing::warn!("Failed to decode preview PNG from bytes: {e}"),
            }
        }
        *self.page_pixbufs.borrow_mut() = pixbufs;

        if is_first {
            self.refit_drawing_area_centered(saved_v_frac);
            *self.first_load.borrow_mut() = false;
            if *self.auto_fit.borrow() {
                let pane = self.clone();
                glib::idle_add_local_once(move || { pane.fit_width(); });
            }
        } else if *self.auto_fit.borrow() {
            // Compute the correct zoom synchronously before the single redraw so
            // there is never an intermediate frame rendered at the old zoom level
            // (which caused the shadow at the bottom of the last page to flicker).
            let scroll_w = self.img_scroll.allocated_width() as f64;
            let pb_w = self.page_pixbufs.borrow().first()
                .map(|pb| pb.width() as f64)
                .unwrap_or(0.0);
            if pb_w > 0.0 && scroll_w > 16.0 {
                let z = ((scroll_w - 16.0) / pb_w).clamp(0.25, 4.0);
                *self.zoom.borrow_mut() = z;
                self.update_zoom_label(z);
                if let Some(f) = self.on_zoom_changed.borrow().as_ref() { f(z); }
            }
            self.refit_drawing_area_centered(saved_v_frac);
        } else {
            self.refit_drawing_area_centered(saved_v_frac);
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
                    let target = (range * frac - page / 2.0)
                        .clamp(adj.lower(), adj.upper() - page);
                    adj.set_value(target);
                }
            });
        }
    }
}

pub fn extract_page_text_via_pdftotext(pane: &PreviewPane, page: usize, _y_start: f64, _y_end: f64) -> Option<String> {
    let root = pane.root_file.borrow().clone()?;
    let stem = root.file_stem()?.to_str()?.to_string();
    let pdf_path = pane.output_dir().join(format!("{stem}.pdf"));
    if !pdf_path.exists() {
        let snapshots = pane.buffer_snapshot.borrow().clone();
        let bytes = crate::compiler::compile_to_pdf_bytes(&root, &snapshots, &std::collections::HashMap::new()).ok()?;
        std::fs::write(&pdf_path, bytes).ok()?;
    }
    let page_str = (page + 1).to_string();
    let out = crate::git_sync::host_command("pdftotext")
        .args(["-layout", "-f", &page_str, "-l", &page_str,
               pdf_path.to_str().unwrap_or(""), "-"])
        .output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        None
    }
}


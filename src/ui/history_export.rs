//! The writing history window: the record of how one document was written, to
//! read on screen and save as a file for a teacher. The text itself is built in
//! `writing_history`; this shows it, lets the person pick a plain or exact reading, adds their name and course, and
//! saves it as text or PDF.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk4::prelude::*;
use libadwaita as adw;

use super::notify::Say;
use crate::writing_history::{History, Item};

/// Longest span still drawn one bar per day; longer ones get a bar per week.
const DAILY_LIMIT_DAYS: i64 = 120;

#[derive(Clone, Copy, PartialEq)]
enum Metric {
    Words,
    Lines,
}

#[derive(Clone, Copy, PartialEq)]
enum Span {
    Days(i64),
    All,
}

#[derive(Clone)]
struct Bar {
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
    lines: usize,
    words: usize,
    lines_out: usize,
    words_out: usize,
}

impl Bar {
    fn written(&self, m: Metric) -> usize {
        match m {
            Metric::Words => self.words,
            Metric::Lines => self.lines,
        }
    }

    fn taken_out(&self, m: Metric) -> usize {
        match m {
            Metric::Words => self.words_out,
            Metric::Lines => self.lines_out,
        }
    }

    fn describe(&self) -> String {
        let when = if self.start == self.end {
            self.start.format("%a %-d %b").to_string()
        } else {
            format!(
                "{} to {}",
                self.start.format("%-d %b"),
                self.end.format("%-d %b")
            )
        };
        let n =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        format!(
            "{when}: {} and {} written, {} and {} taken out",
            n(self.words, "word", "words"),
            n(self.lines, "line", "lines"),
            n(self.words_out, "word", "words"),
            n(self.lines_out, "line", "lines"),
        )
    }
}

type Bars = Rc<RefCell<Vec<Bar>>>;

pub fn run(
    parent: &impl IsA<gtk4::Window>,
    toasts: &adw::ToastOverlay,
    project_root: &Path,
    active_file: Option<PathBuf>,
    backup: Rc<dyn Fn()>,
) {
    let Some(active_file) = active_file else {
        toasts.say(adw::Toast::new(
            "Open the document you want a writing history for first.",
        ));
        return;
    };
    let win = adw::Window::builder()
        .title("Writing history")
        .default_width(980)
        .default_height(740)
        .width_request(420)
        .transient_for(parent)
        .build();

    // ── Header ───────────────────────────────────────────────────────────
    let header = adw::HeaderBar::new();
    header.add_css_class("fond-chrome");

    let sidebar_toggle = gtk4::ToggleButton::new();
    sidebar_toggle.set_icon_name("sidebar-show-symbolic");
    sidebar_toggle.set_active(true);
    sidebar_toggle.set_tooltip_text(Some("Show or hide the list of versions"));
    header.pack_start(&sidebar_toggle);

    let save_menu = gtk4::MenuButton::new();
    save_menu.set_label("Save…");
    save_menu.add_css_class("suggested-action");
    save_menu.set_sensitive(false);
    save_menu.set_tooltip_text(Some(
        "Save this record as a file to hand to a teacher or attach to your work",
    ));
    let save_popover = gtk4::Popover::new();
    let save_choices = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    let save_text = gtk4::Button::with_label("As a text file");
    let save_pdf = gtk4::Button::with_label("As a PDF");
    for b in [&save_text, &save_pdf] {
        b.add_css_class("flat");
        save_choices.append(b);
    }
    save_popover.set_child(Some(&save_choices));
    save_menu.set_popover(Some(&save_popover));
    let copy_btn = gtk4::Button::from_icon_name("edit-copy-symbolic");
    copy_btn.set_sensitive(false);
    copy_btn.set_tooltip_text(Some("Copy the whole record"));
    copy_btn.update_property(&[gtk4::accessible::Property::Label("Copy the whole record")]);
    header.pack_end(&save_menu);
    header.pack_end(&copy_btn);

    // ── Who it is from, and how to read it ───────────────────────────────
    let author = gtk4::Entry::new();
    author.set_placeholder_text(Some("Your name"));
    author.set_hexpand(true);
    author.set_text(&crate::config::shared().borrow().history_author);
    let course = gtk4::Entry::new();
    course.set_placeholder_text(Some("Course or class"));
    course.set_hexpand(true);
    course.set_text(&crate::config::shared().borrow().history_course);
    for e in [&author, &course] {
        e.set_tooltip_text(Some("Put at the top of the record. Zerkalo remembers it."));
    }

    let plain_btn = gtk4::ToggleButton::with_label("In words");
    let exact_btn = gtk4::ToggleButton::with_label("Exact changes");
    exact_btn.set_group(Some(&plain_btn));
    plain_btn.set_active(true);
    plain_btn.set_tooltip_text(Some("What each version added, as text anyone can read"));
    exact_btn.set_tooltip_text(Some(
        "Every line that changed, marked + for added and - for taken out",
    ));
    let reading = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    reading.add_css_class("linked");
    reading.append(&plain_btn);
    reading.append(&exact_btn);

    let summary = gtk4::Label::new(None);
    summary.set_xalign(0.0);
    summary.set_wrap(true);
    summary.set_width_chars(24);
    summary.set_hexpand(true);

    let top_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    top_row.append(&summary);
    top_row.append(&reading);
    let name_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    name_row.append(&author);
    name_row.append(&course);
    let controls = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    controls.set_margin_start(18);
    controls.set_margin_end(18);
    controls.set_margin_top(10);
    controls.set_margin_bottom(10);
    controls.append(&top_row);
    controls.append(&name_row);

    let notice = adw::Banner::new("");

    // ── The record ───────────────────────────────────────────────────────
    let buffer = gtk4::TextBuffer::new(None);
    let colors = super::theme::diff_colors();
    buffer.create_tag(
        Some("added"),
        &[
            ("foreground", &colors.added_fg),
            ("paragraph-background", &colors.added_bg),
        ],
    );
    buffer.create_tag(
        Some("removed"),
        &[
            ("foreground", &colors.removed_fg),
            ("paragraph-background", &colors.removed_bg),
        ],
    );
    buffer.create_tag(
        Some("version"),
        &[
            ("weight", &700),
            ("scale", &1.15),
            ("pixels-above-lines", &14),
        ],
    );
    buffer.create_tag(
        Some("file"),
        &[("weight", &700), ("pixels-above-lines", &8)],
    );
    buffer.create_tag(Some("section"), &[("weight", &700), ("scale", &1.1)]);
    buffer.create_tag(Some("quiet"), &[("foreground", &"gray")]);

    let view = gtk4::TextView::with_buffer(&buffer);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.set_wrap_mode(gtk4::WrapMode::WordChar);
    view.set_left_margin(18);
    view.set_right_margin(18);
    view.set_top_margin(8);
    view.set_bottom_margin(18);
    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_child(Some(&view));

    let spinner = gtk4::Spinner::new();
    spinner.start();
    spinner.set_halign(gtk4::Align::Center);
    spinner.set_valign(gtk4::Align::Center);
    spinner.set_width_request(36);
    spinner.set_height_request(36);
    let empty = adw::StatusPage::new();
    empty.set_icon_name(Some("document-open-recent-symbolic"));
    empty.set_title("No history yet");

    let stack = gtk4::Stack::new();
    stack.add_named(&spinner, Some("loading"));
    stack.add_named(&scroll, Some("text"));
    stack.add_named(&empty, Some("empty"));
    stack.set_vexpand(true);
    stack.set_hexpand(true);

    // ── The list of versions, with the days they were written ────────────
    let bars: Bars = Rc::new(RefCell::new(Vec::new()));
    let hovered: Rc<Cell<Option<usize>>> = Rc::new(Cell::new(None));
    let metric = Rc::new(Cell::new(Metric::Words));
    let span = Rc::new(Cell::new(Span::Days(30)));
    let history: Rc<RefCell<Option<History>>> = Rc::new(RefCell::new(None));
    let marks: Rc<RefCell<Vec<gtk4::TextMark>>> = Rc::new(RefCell::new(Vec::new()));
    let chart_title = gtk4::Label::new(None);
    chart_title.add_css_class("caption-heading");
    chart_title.set_xalign(0.0);
    let chart = gtk4::DrawingArea::new();
    chart.set_content_height(90);
    chart.set_hexpand(true);
    chart.set_focusable(true);
    chart.set_has_tooltip(true);
    {
        let (bars, hovered, metric) = (bars.clone(), hovered.clone(), metric.clone());
        chart.set_draw_func(move |area, cr, w, h| {
            let bars = bars.borrow();
            if bars.is_empty() {
                return;
            }
            let m = metric.get();
            let color = |name: &str, fallback| super::theme::rgb(area, name).unwrap_or(fallback);
            let (r, g, b) = color("accent_bg_color", (0.2, 0.5, 0.9));
            let (dr, dg, db) = color("destructive_bg_color", (0.8, 0.2, 0.2));
            let (fr, fg, fb) = color("window_fg_color", (0.5, 0.5, 0.5));
            let (w, h) = (w as f64, h as f64);
            let base = (h * 0.72).round();
            let (up, down) = (base - 2.0, h - base - 1.0);
            let peak_up = bars.iter().map(|b| b.written(m)).max().unwrap_or(0).max(1) as f64;
            let peak_down = bars
                .iter()
                .map(|b| b.taken_out(m))
                .max()
                .unwrap_or(0)
                .max(1) as f64;
            let scale = (up / peak_up).min(down / peak_down);
            let slot = w / bars.len() as f64;
            let bar_w = (slot * 0.7).clamp(1.0, 18.0);
            let tall = |n: usize| {
                if n > 0 {
                    (n as f64 * scale).max(2.0)
                } else {
                    0.0
                }
            };
            for (i, bar) in bars.iter().enumerate() {
                let alpha = if hovered.get().is_none_or(|j| j == i) {
                    1.0
                } else {
                    0.45
                };
                let x = i as f64 * slot + (slot - bar_w) / 2.0;
                cr.set_source_rgba(r, g, b, alpha);
                let height = tall(bar.written(m));
                cr.rectangle(x, base - height, bar_w, height);
                let _ = cr.fill();
                cr.set_source_rgba(dr, dg, db, alpha * 0.6);
                let height = tall(bar.taken_out(m)).min(down);
                cr.rectangle(x, base, bar_w, height);
                let _ = cr.fill();
            }
            cr.set_source_rgba(fr, fg, fb, 0.25);
            cr.rectangle(0.0, base, w, 1.0);
            let _ = cr.fill();
        });
    }
    let bar_at = {
        let bars = bars.clone();
        move |area: &gtk4::DrawingArea, x: f64| -> Option<usize> {
            let n = bars.borrow().len();
            let w = area.width() as f64;
            (n > 0 && w > 0.0 && x >= 0.0 && x < w)
                .then(|| ((x / w * n as f64) as usize).min(n - 1))
        }
    };
    let point_at = {
        let (chart, hovered, bars) = (chart.clone(), hovered.clone(), bars.clone());
        move |i: Option<usize>| {
            if hovered.get() != i {
                hovered.set(i);
                if let Some(bar) = i.and_then(|i| bars.borrow().get(i).cloned()) {
                    chart.update_property(&[gtk4::accessible::Property::Label(&bar.describe())]);
                }
                chart.queue_draw();
            }
        }
    };
    {
        let motion = gtk4::EventControllerMotion::new();
        {
            let (chart, bar_at, point_at) = (chart.clone(), bar_at.clone(), point_at.clone());
            motion.connect_motion(move |_, x, _| point_at(bar_at(&chart, x)));
        }
        {
            let point_at = point_at.clone();
            motion.connect_leave(move |_| point_at(None));
        }
        chart.add_controller(motion);
        let focus = gtk4::EventControllerFocus::new();
        {
            let (bars, hovered, point_at) = (bars.clone(), hovered.clone(), point_at.clone());
            focus.connect_enter(move |_| {
                let n = bars.borrow().len();
                if hovered.get().is_none() && n > 0 {
                    point_at(Some(n - 1));
                }
            });
        }
        {
            let point_at = point_at.clone();
            focus.connect_leave(move |_| point_at(None));
        }
        chart.add_controller(focus);
        let (bars, bar_at) = (bars.clone(), bar_at.clone());
        chart.connect_query_tooltip(move |area, x, _, _, tip| {
            let Some(bar) = bar_at(area, x as f64).and_then(|i| bars.borrow().get(i).cloned())
            else {
                return false;
            };
            tip.set_text(Some(&bar.describe()));
            true
        });
    }
    let chart_range = gtk4::Label::new(None);
    chart_range.add_css_class("caption");
    chart_range.add_css_class("dim-label");
    chart_range.set_xalign(0.0);

    let chart_toggle = |labels: &[&str]| {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        row.add_css_class("linked");
        let mut first: Option<gtk4::ToggleButton> = None;
        let mut buttons = Vec::new();
        for label in labels {
            let b = gtk4::ToggleButton::with_label(label);
            b.add_css_class("caption");
            if let Some(f) = &first {
                b.set_group(Some(f));
            } else {
                first = Some(b.clone());
                b.set_active(true);
            }
            row.append(&b);
            buttons.push(b);
        }
        (row, buttons)
    };
    let (metric_row, metric_buttons) = chart_toggle(&["Words", "Lines"]);
    let (span_row, span_buttons) = chart_toggle(&["30 days", "90 days", "All"]);
    let chart_controls = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    chart_controls.append(&metric_row);
    chart_controls.append(&span_row);

    let versions_title = gtk4::Label::new(Some("Versions"));
    versions_title.add_css_class("caption-heading");
    versions_title.set_xalign(0.0);
    let list = gtk4::ListBox::new();
    list.add_css_class("navigation-sidebar");
    list.set_selection_mode(gtk4::SelectionMode::Single);
    let list_scroll = gtk4::ScrolledWindow::new();
    list_scroll.set_vexpand(true);
    list_scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
    list_scroll.set_child(Some(&list));

    let redraw_chart = {
        let (bars, chart, chart_title, chart_range, history, metric, span) = (
            bars.clone(),
            chart.clone(),
            chart_title.clone(),
            chart_range.clone(),
            history.clone(),
            metric.clone(),
            span.clone(),
        );
        Rc::new(move || {
            let items = history.borrow();
            update_chart(
                &bars,
                &chart,
                &chart_title,
                &chart_range,
                items.as_ref().map_or(&[][..], |h| &h.items[..]),
                metric.get(),
                span.get(),
            );
        })
    };
    for (b, m) in metric_buttons.iter().zip([Metric::Words, Metric::Lines]) {
        let (metric, redraw) = (metric.clone(), redraw_chart.clone());
        b.connect_toggled(move |b| {
            if b.is_active() {
                metric.set(m);
                redraw();
            }
        });
    }
    for (b, s) in span_buttons
        .iter()
        .zip([Span::Days(30), Span::Days(90), Span::All])
    {
        let (span, redraw) = (span.clone(), redraw_chart.clone());
        b.connect_toggled(move |b| {
            if b.is_active() {
                span.set(s);
                redraw();
            }
        });
    }
    let jump_to_bar = {
        let (bars, history, list) = (bars.clone(), history.clone(), list.clone());
        Rc::new(move |i: usize| {
            let Some(bar) = bars.borrow().get(i).cloned() else {
                return;
            };
            let history = history.borrow();
            let Some(n) = history.as_ref().and_then(|h| {
                h.items
                    .iter()
                    .position(|it| it.day >= bar.start && it.day <= bar.end)
            }) else {
                return;
            };
            if let Some(row) = list.row_at_index(n as i32) {
                list.select_row(Some(&row));
                row.activate();
            }
        })
    };
    {
        let click = gtk4::GestureClick::new();
        let (area, bar_at, jump) = (chart.clone(), bar_at.clone(), jump_to_bar.clone());
        click.connect_released(move |_, _, x, _| {
            area.grab_focus();
            if let Some(i) = bar_at(&area, x) {
                jump(i);
            }
        });
        chart.add_controller(click);
    }
    {
        let keys = gtk4::EventControllerKey::new();
        let (bars, hovered, point_at, jump) = (
            bars.clone(),
            hovered.clone(),
            point_at.clone(),
            jump_to_bar.clone(),
        );
        keys.connect_key_pressed(move |_, key, _, _| {
            use gtk4::gdk::Key;
            let n = bars.borrow().len();
            if n == 0 {
                return gtk4::glib::Propagation::Proceed;
            }
            let at = hovered.get().unwrap_or(n - 1);
            let target = match key {
                Key::Left => at.saturating_sub(1),
                Key::Right => (at + 1).min(n - 1),
                Key::Home => 0,
                Key::End => n - 1,
                Key::Return | Key::KP_Enter | Key::space => {
                    jump(at);
                    return gtk4::glib::Propagation::Stop;
                }
                _ => return gtk4::glib::Propagation::Proceed,
            };
            point_at(Some(target));
            gtk4::glib::Propagation::Stop
        });
        chart.add_controller(keys);
    }

    let side = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    side.set_width_request(260);
    side.set_hexpand(false);
    side.set_hexpand_set(true);
    side.set_margin_start(14);
    side.set_margin_end(14);
    side.set_margin_top(14);
    side.set_margin_bottom(14);
    side.append(&chart_title);
    side.append(&chart_controls);
    side.append(&chart);
    side.append(&chart_range);
    side.append(&versions_title);
    side.append(&list_scroll);

    let main = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    main.append(&side);
    let side_sep = gtk4::Separator::new(gtk4::Orientation::Vertical);
    main.append(&side_sep);
    main.append(&stack);
    {
        let side = side.clone();
        sidebar_toggle.connect_toggled(move |b| {
            side.set_visible(b.is_active());
            side_sep.set_visible(b.is_active());
        });
    }

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    body.append(&controls);
    body.append(&notice);
    body.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    body.append(&main);

    let toolbar = adw::ToolbarView::new();
    toolbar.set_top_bar_style(adw::ToolbarStyle::RaisedBorder);
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&body));
    win.set_content(Some(&toolbar));

    // ── Behaviour ────────────────────────────────────────────────────────
    let generation = Rc::new(Cell::new(0u32));
    let root = project_root.to_path_buf();

    let current_text = {
        let (history, plain_btn) = (history.clone(), plain_btn.clone());
        move || -> String {
            history
                .borrow()
                .as_ref()
                .map(|h| {
                    if plain_btn.is_active() {
                        h.plain.clone()
                    } else {
                        h.text.clone()
                    }
                })
                .unwrap_or_default()
        }
    };

    let show: Rc<dyn Fn()> = {
        let (history, buffer, marks, plain_btn) = (
            history.clone(),
            buffer.clone(),
            marks.clone(),
            plain_btn.clone(),
        );
        Rc::new(move || {
            if let Some(h) = history.borrow().as_ref() {
                let text = if plain_btn.is_active() {
                    &h.plain
                } else {
                    &h.text
                };
                *marks.borrow_mut() = fill(&buffer, text);
            }
        })
    };

    let load: Rc<dyn Fn()> =
        {
            let (stack, summary, notice, empty) = (
                stack.clone(),
                summary.clone(),
                notice.clone(),
                empty.clone(),
            );
            let (save_menu, copy_btn) = (save_menu.clone(), copy_btn.clone());
            let (history, generation, root) = (history.clone(), generation.clone(), root.clone());
            let active_file = active_file.clone();
            let (author, course) = (author.clone(), course.clone());
            let (redraw_chart, list) = (redraw_chart.clone(), list.clone());
            let (show, marks) = (show.clone(), marks.clone());
            Rc::new(move || {
                generation.set(generation.get() + 1);
                let mine = generation.get();
                stack.set_visible_child_name("loading");
                save_menu.set_sensitive(false);
                copy_btn.set_sensitive(false);
                summary.set_text("");
                notice.set_revealed(false);
                let only = Some(active_file.clone());
                crate::activity::flush();
                let extras = crate::writing_history::Extras {
                    author: author.text().to_string(),
                    course: course.text().to_string(),
                    activity: crate::activity::ActivityLog::load(&active_file),
                };
                let (root, today) = (
                    root.clone(),
                    chrono::Local::now().format("%-d %B %Y").to_string(),
                );
                let (stack, summary, notice, empty) = (
                    stack.clone(),
                    summary.clone(),
                    notice.clone(),
                    empty.clone(),
                );
                let (save_menu, copy_btn) = (save_menu.clone(), copy_btn.clone());
                let (history, generation) = (history.clone(), generation.clone());
                let (redraw_chart, list) = (redraw_chart.clone(), list.clone());
                let (show, marks) = (show.clone(), marks.clone());
                gtk4::glib::spawn_future_local(async move {
                    let built = gtk4::gio::spawn_blocking(move || {
                        crate::writing_history::build(&root, only.as_deref(), &today, &extras)
                    })
                    .await;
                    if generation.get() != mine {
                        return;
                    }
                    match built {
                        Ok(Ok(h)) => {
                            summary.set_text(&format!(
                            "{} saved {} over {} {} · {} lines written, {} taken out or rewritten",
                            h.versions,
                            if h.versions == 1 { "version" } else { "versions" },
                            h.days,
                            if h.days == 1 { "day" } else { "days" },
                            h.added,
                            h.removed
                        ));
                            if h.unsaved_files > 0 {
                                notice.set_title(
                                "Your latest changes haven't been backed up yet, so they aren't \
                                 in this record.",
                            );
                                notice.set_revealed(true);
                            }
                            *history.borrow_mut() = Some(h);
                            redraw_chart();
                            show();
                            fill_list(&list, history.borrow().as_ref().map(|h| &h.items[..]));
                            let _ = &marks;
                            stack.set_visible_child_name("text");
                            save_menu.set_sensitive(true);
                            copy_btn.set_sensitive(true);
                        }
                        Ok(Err(why)) => {
                            empty.set_description(Some(&why));
                            stack.set_visible_child_name("empty");
                        }
                        Err(_) => {
                            empty.set_description(Some("Couldn't read the history."));
                            stack.set_visible_child_name("empty");
                        }
                    }
                });
            })
        };

    {
        let show = show.clone();
        plain_btn.connect_toggled(move |_| show());
    }
    {
        let (view, marks) = (view.clone(), marks.clone());
        list.connect_row_activated(move |_, row| {
            let n = row.index() as usize;
            if let Some(mark) = marks.borrow().get(n) {
                view.scroll_to_mark(mark, 0.0, true, 0.0, 0.05);
            }
        });
    }
    {
        // Name and course go to the top of the record; reload once typing stops.
        let timer: Rc<RefCell<Option<gtk4::glib::SourceId>>> = Rc::new(RefCell::new(None));
        let changed = {
            let (load, author, course) = (load.clone(), author.clone(), course.clone());
            move || {
                if let Some(id) = timer.borrow_mut().take() {
                    id.remove();
                }
                let (load, author, course, timer2) =
                    (load.clone(), author.clone(), course.clone(), timer.clone());
                let id = gtk4::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(700),
                    move || {
                        *timer2.borrow_mut() = None;
                        let (a, c) = (author.text().to_string(), course.text().to_string());
                        let _ = crate::config::update(|cfg| {
                            cfg.history_author = a;
                            cfg.history_course = c;
                        });
                        load();
                    },
                );
                *timer.borrow_mut() = Some(id);
            }
        };
        let changed = Rc::new(changed);
        let c1 = changed.clone();
        author.connect_changed(move |_| c1());
        course.connect_changed(move |_| changed());
    }
    {
        let (backup, load, notice) = (backup, load.clone(), notice.clone());
        notice.set_button_label(Some("Back up now"));
        let n2 = notice.clone();
        n2.connect_button_clicked(move |b| {
            let before = crate::git_sync::syncs_finished();
            backup();
            b.set_title("Backing up…");
            b.set_button_label(None);
            let (load, notice) = (load.clone(), notice.clone());
            let tries = Rc::new(Cell::new(0u32));
            gtk4::glib::timeout_add_local(std::time::Duration::from_millis(600), move || {
                tries.set(tries.get() + 1);
                let done =
                    !crate::git_sync::sync_active() && crate::git_sync::syncs_finished() != before;
                if done || tries.get() > 200 {
                    notice.set_button_label(Some("Back up now"));
                    load();
                    return gtk4::glib::ControlFlow::Break;
                }
                gtk4::glib::ControlFlow::Continue
            });
        });
    }
    {
        let (current_text, copy_btn2) = (current_text.clone(), copy_btn.clone());
        copy_btn.connect_clicked(move |_| {
            copy_btn2.display().clipboard().set_text(&current_text());
        });
    }

    let file_stem = {
        let active_file = active_file.clone();
        move || -> String {
            active_file
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "project".into())
        }
    };
    for (button, pdf) in [(&save_text, false), (&save_pdf, true)] {
        let (win, toasts, popover) = (win.clone(), toasts.clone(), save_popover.clone());
        let (current_text, file_stem) = (current_text.clone(), file_stem.clone());
        button.connect_clicked(move |_| {
            popover.popdown();
            let dialog = gtk4::FileDialog::builder()
                .title("Save your writing history")
                .modal(true)
                .initial_name(format!(
                    "{}-writing-history.{}",
                    file_stem(),
                    if pdf { "pdf" } else { "txt" }
                ))
                .build();
            let (text, toasts) = (current_text(), toasts.clone());
            dialog.save(Some(&win), None::<&gtk4::gio::Cancellable>, move |result| {
                let Ok(file) = result else { return };
                let Some(out) = file.path() else { return };
                let toasts = toasts.clone();
                gtk4::glib::spawn_future_local(async move {
                    let written = gtk4::gio::spawn_blocking(move || write_record(&text, &out, pdf))
                        .await
                        .unwrap_or_else(|_| Err("something went wrong".into()));
                    let message = match written {
                        Ok(()) => "Saved your writing history.".to_string(),
                        Err(why) => format!("Couldn't save the file — {why}."),
                    };
                    toasts.say(adw::Toast::new(&message));
                });
            });
        });
    }

    win.present();
    load();
}

fn write_record(text: &str, out: &Path, pdf: bool) -> Result<(), String> {
    if !pdf {
        return crate::error::atomic_write(out, text.as_bytes())
            .map_err(|e| crate::error::io_reason(&e));
    }
    let scratch = std::env::temp_dir().join(format!("zerkalo-history-{}.typ", std::process::id()));
    std::fs::write(&scratch, crate::writing_history::to_typst(text))
        .map_err(|e| crate::error::io_reason(&e))?;
    let pdf_bytes = crate::compiler::compile_to_pdf_bytes(
        &scratch,
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        None,
    );
    let _ = std::fs::remove_file(&scratch);
    let bytes = pdf_bytes.map_err(|_| "the PDF couldn't be made".to_string())?;
    crate::error::atomic_write(out, &bytes).map_err(|e| crate::error::io_reason(&e))
}

fn update_chart(
    bars: &Bars,
    chart: &gtk4::DrawingArea,
    title: &gtk4::Label,
    range: &gtk4::Label,
    items: &[Item],
    metric: Metric,
    span: Span,
) {
    let mut out: Vec<Bar> = Vec::new();
    let mut step = 1;
    if let (Some(first), Some(last)) = (items.first(), items.last()) {
        let start = match span {
            Span::Days(n) => first.day.max(last.day - chrono::Duration::days(n - 1)),
            Span::All => first.day,
        };
        if (last.day - start).num_days() >= DAILY_LIMIT_DAYS {
            step = 7;
        }
        let mut d = start;
        while d <= last.day {
            let end = (d + chrono::Duration::days(step - 1)).min(last.day);
            let mut bar = Bar {
                start: d,
                end,
                lines: 0,
                words: 0,
                lines_out: 0,
                words_out: 0,
            };
            for i in items.iter().filter(|i| i.day >= d && i.day <= end) {
                bar.lines += i.added;
                bar.words += i.words;
                bar.lines_out += i.removed;
                bar.words_out += i.words_removed;
            }
            out.push(bar);
            d = end + chrono::Duration::days(1);
        }
        range.set_text(&format!(
            "{} to {}",
            start.format("%-d %b"),
            last.day.format("%-d %b")
        ));
    } else {
        range.set_text("");
    }
    let what = match metric {
        Metric::Words => "Words",
        Metric::Lines => "Lines",
    };
    let per = if step == 1 { "day" } else { "week" };
    title.set_text(&format!("{what} written each {per}"));
    chart.update_property(&[gtk4::accessible::Property::Label(&format!(
        "Chart of {} written each {per}, with what was taken out below. Use left and right to move between bars, and Enter to show that {per}'s versions.",
        what.to_lowercase()
    ))]);
    *bars.borrow_mut() = out;
    chart.queue_draw();
}

fn fill_list(list: &gtk4::ListBox, items: Option<&[Item]>) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
    for (i, item) in items.unwrap_or_default().iter().enumerate() {
        let row = gtk4::Box::new(gtk4::Orientation::Vertical, 1);
        row.set_margin_top(5);
        row.set_margin_bottom(5);
        row.set_margin_start(6);
        row.set_margin_end(6);
        let title = gtk4::Label::new(Some(&format!("Version {} · {}", i + 1, item.label)));
        title.set_xalign(0.0);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        let detail = gtk4::Label::new(Some(&format!(
            "{} added, {} taken out",
            item.added, item.removed
        )));
        detail.set_xalign(0.0);
        detail.add_css_class("caption");
        detail.add_css_class("dim-label");
        row.append(&title);
        row.append(&detail);
        list.append(&row);
    }
}

/// The record with its changes coloured: added lines green, removed red,
/// each version's heading and each file's name bold, the rest plain. Returns a
/// mark at the start of each version, in order, for jumping to it.
fn fill(buffer: &gtk4::TextBuffer, text: &str) -> Vec<gtk4::TextMark> {
    buffer.set_text("");
    let mut iter = buffer.start_iter();
    let table = buffer.tag_table();
    let mut marks = Vec::new();
    for line in text.lines() {
        let is_version = line.starts_with("Version ") && line.contains(" of ");
        let tag = if is_version {
            "version"
        } else if line.starts_with("diff --git")
            || line.starts_with("In ") && line.contains(" added")
        {
            "file"
        } else if matches!(
            line,
            "HOW THIS WAS WRITTEN"
                | "Reading this record"
                | "How to check it"
                | "Typing sessions"
                | "Larger pieces of text pasted in"
        ) {
            "section"
        } else if line.starts_with("+++")
            || line.starts_with("---")
            || line.starts_with("@@")
            || line.starts_with("index ")
            || line.starts_with("====")
        {
            "quiet"
        } else if line.starts_with('+') {
            "added"
        } else if line.starts_with('-') {
            "removed"
        } else {
            ""
        };
        if is_version {
            marks.push(buffer.create_mark(None, &iter, true));
        }
        let with_newline = format!("{line}\n");
        match table.lookup(tag) {
            Some(t) => buffer.insert_with_tags(&mut iter, &with_newline, &[&t]),
            None => buffer.insert(&mut iter, &with_newline),
        }
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_can_be_saved_as_a_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("record.pdf");
        let text = "HOW THIS WAS WRITTEN\n\nVersion 1 of 1  —  today\n+added \"line\"\n-gone\n";
        write_record(text, &out, true).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }
}

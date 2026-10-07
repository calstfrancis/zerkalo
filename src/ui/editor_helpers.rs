//! Writing helpers wired onto each editor view: list continuation, indenting
//! list items, hanging indents, paste-a-link, moving lines, expand/shrink
//! selection and the matching-delimiter highlight. The text rules themselves
//! live in `text_ops` so they can be tested without GTK.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk4::gdk::{Key, ModifierType};
use gtk4::prelude::*;
use gtk4::{glib, EventControllerKey, PropagationPhase, TextTag};
use sourceview5::{Buffer, View};

use super::editor_pane::{base_key, hidden_template_end};
use super::text_ops::{self, Continuation};

const HANG_PREFIX: &str = "zk-hang:";
const DELIM_TAG: &str = "zk-delim-match";
const HANG_DEBOUNCE: Duration = Duration::from_millis(400);

pub fn wire(view: &View, buffer: &Buffer) {
    wire_lists(view, buffer);
    wire_paste_link(view, buffer);
    wire_move_lines(view, buffer);
    wire_expand_selection(view, buffer);
    wire_hanging_indents(view, buffer);
    wire_delimiter_match(view, buffer);
}

fn capture_keys(
    view: &View,
    handler: impl Fn(Key, u32, ModifierType) -> glib::Propagation + 'static,
) {
    let ctrl = EventControllerKey::new();
    ctrl.set_propagation_phase(PropagationPhase::Capture);
    ctrl.connect_key_pressed(move |_, key, keycode, mods| handler(key, keycode, mods));
    view.add_controller(ctrl);
}

fn line_text(buf: &Buffer, line: i32) -> String {
    let Some(start) = buf.iter_at_line(line) else {
        return String::new();
    };
    let mut end = start;
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    buf.text(&start, &end, false).to_string()
}

fn plain(mods: ModifierType) -> bool {
    !mods.intersects(
        ModifierType::CONTROL_MASK
            | ModifierType::ALT_MASK
            | ModifierType::SHIFT_MASK
            | ModifierType::SUPER_MASK,
    )
}

// ── Lists: Enter continues, Tab / Shift+Tab nest ────────────────────────────

fn wire_lists(view: &View, buffer: &Buffer) {
    let buf = buffer.clone();
    capture_keys(view, move |key, _, mods| {
        let enter = matches!(key, Key::Return | Key::KP_Enter);
        let tab = matches!(key, Key::Tab | Key::ISO_Left_Tab);
        if buf.has_selection()
            || !(enter && plain(mods)
                || tab && !mods.intersects(ModifierType::CONTROL_MASK | ModifierType::ALT_MASK))
        {
            return glib::Propagation::Proceed;
        }
        let cursor = buf.iter_at_offset(buf.cursor_position());
        let line = cursor.line();
        let text = line_text(&buf, line);

        if enter {
            if !cursor.ends_line() {
                return glib::Propagation::Proceed;
            }
            return match text_ops::continuation(&text) {
                Continuation::None => glib::Propagation::Proceed,
                Continuation::EndList => {
                    let start = buf.iter_at_line(line).unwrap_or(cursor);
                    let (mut s, mut e) = (start, cursor);
                    buf.begin_user_action();
                    buf.delete(&mut s, &mut e);
                    buf.end_user_action();
                    glib::Propagation::Stop
                }
                Continuation::Next(prefix) => {
                    // Let the newline (and any autocorrect on the last word) happen
                    // first, then put the marker on the line it created.
                    let buf = buf.clone();
                    glib::idle_add_local_full(glib::Priority::HIGH, move || {
                        let at = buf.iter_at_offset(buf.cursor_position());
                        if at.line() != line + 1 || !line_text(&buf, at.line()).trim().is_empty() {
                            return glib::ControlFlow::Break;
                        }
                        let Some(mut s) = buf.iter_at_line(at.line()) else {
                            return glib::ControlFlow::Break;
                        };
                        let mut e = s;
                        if !e.ends_line() {
                            e.forward_to_line_end();
                        }
                        buf.begin_user_action();
                        buf.delete(&mut s, &mut e);
                        let mut ins = buf.iter_at_line(at.line()).unwrap_or(s);
                        buf.insert(&mut ins, &prefix);
                        buf.end_user_action();
                        glib::ControlFlow::Break
                    });
                    glib::Propagation::Proceed
                }
            };
        }

        if text_ops::continuation(&text) == Continuation::None {
            return glib::Propagation::Proceed;
        }
        let Some(mut start) = buf.iter_at_line(line) else {
            return glib::Propagation::Proceed;
        };
        buf.begin_user_action();
        if mods.contains(ModifierType::SHIFT_MASK) || key == Key::ISO_Left_Tab {
            let strip = text
                .chars()
                .take(2)
                .take_while(|c| *c == ' ')
                .count()
                .max(usize::from(text.starts_with('\t')));
            let mut end = start;
            end.forward_chars(strip as i32);
            buf.delete(&mut start, &mut end);
        } else {
            buf.insert(&mut start, "  ");
        }
        buf.end_user_action();
        glib::Propagation::Stop
    });
}

// ── Hanging indent for wrapped list items ──────────────────────────────────

fn hang_tag_for(view: &View, buffer: &Buffer, prefix: &str) -> TextTag {
    let name = format!("{HANG_PREFIX}{prefix}");
    let table = buffer.tag_table();
    let tag = table.lookup(&name).unwrap_or_else(|| {
        let tag = TextTag::new(Some(&name));
        table.add(&tag);
        tag
    });
    tune_hang_tag(view, &tag, prefix);
    tag
}

fn tune_hang_tag(view: &View, tag: &TextTag, prefix: &str) {
    let measure = |s: &str| view.create_pango_layout(Some(s)).pixel_size().0;
    let width = (measure(&format!("{prefix}|")) - measure("|")).max(0);
    // A negative indent keeps the first line at the margin and pushes the
    // wrapped lines in by that much.
    tag.set_left_margin(view.left_margin());
    tag.set_indent(-width);
}

/// Margins change with Simple Mode and font changes; re-measure existing tags.
pub fn retune_hanging_tags(view: &View, buffer: &Buffer) {
    let mut tags = Vec::new();
    buffer.tag_table().foreach(|t| tags.push(t.clone()));
    for tag in tags {
        if let Some(prefix) = tag
            .name()
            .and_then(|n| n.strip_prefix(HANG_PREFIX).map(str::to_owned))
        {
            tune_hang_tag(view, &tag, &prefix);
        }
    }
}

fn wire_hanging_indents(view: &View, buffer: &Buffer) {
    let view = view.clone();
    let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let cache: Rc<RefCell<Vec<(i32, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let sweep = {
        let view = view.clone();
        let cache = cache.clone();
        move |buf: &Buffer| {
            let items: Vec<(i32, String)> = (0..buf.line_count())
                .filter_map(|ln| text_ops::list_prefix(&line_text(buf, ln)).map(|p| (ln, p)))
                .collect();
            if *cache.borrow() == items {
                return;
            }
            let (s, e) = buf.bounds();
            let mut old = Vec::new();
            buf.tag_table().foreach(|t| {
                if t.name().is_some_and(|n| n.starts_with(HANG_PREFIX)) {
                    old.push(t.clone());
                }
            });
            for tag in &old {
                buf.remove_tag(tag, &s, &e);
            }
            for (ln, prefix) in &items {
                let tag = hang_tag_for(&view, buf, prefix);
                if let Some(a) = buf.iter_at_line(*ln) {
                    let mut b = a;
                    b.forward_to_line_end();
                    b.forward_char();
                    buf.apply_tag(&tag, &a, &b);
                }
            }
            *cache.borrow_mut() = items;
        }
    };
    let sweep = Rc::new(sweep);
    {
        let sweep = sweep.clone();
        let t = timer.clone();
        buffer.connect_changed(move |buf| {
            if let Some(id) = t.borrow_mut().take() {
                id.remove();
            }
            let buf = buf.clone();
            let sweep = sweep.clone();
            let t2 = t.clone();
            *t.borrow_mut() = Some(glib::timeout_add_local_once(HANG_DEBOUNCE, move || {
                *t2.borrow_mut() = None;
                sweep(&buf);
            }));
        });
    }
    let buf = buffer.clone();
    glib::idle_add_local_once(move || sweep(&buf));
}

// ── Paste a link over selected text ─────────────────────────────────────────

fn wire_paste_link(view: &View, buffer: &Buffer) {
    let buf = buffer.clone();
    let v = view.clone();
    capture_keys(view, move |key, keycode, mods| {
        let key = base_key(key, keycode);
        let paste = (mods.contains(ModifierType::CONTROL_MASK)
            && !mods.intersects(ModifierType::ALT_MASK | ModifierType::SHIFT_MASK)
            && key == Key::v)
            || (mods.contains(ModifierType::SHIFT_MASK)
                && !mods.intersects(ModifierType::CONTROL_MASK | ModifierType::ALT_MASK)
                && key == Key::Insert);
        let Some((s, e)) = buf.selection_bounds().filter(|_| paste) else {
            return glib::Propagation::Proceed;
        };
        let selected = buf.text(&s, &e, false).to_string();
        if selected.contains(['\n', '[', ']']) {
            return glib::Propagation::Proceed;
        }
        let (so, eo) = (s.offset(), e.offset());
        let buf = buf.clone();
        let v = v.clone();
        glib::spawn_future_local(async move {
            let clip = v.clipboard();
            let url = match clip.read_text_future().await {
                Ok(Some(t)) if text_ops::is_bare_url(&t) => Some(t.trim().to_string()),
                _ => None,
            };
            let unchanged = buf
                .selection_bounds()
                .is_some_and(|(s, e)| s.offset() == so && e.offset() == eo);
            match url {
                Some(url) if unchanged => {
                    let (mut s, mut e) = (buf.iter_at_offset(so), buf.iter_at_offset(eo));
                    buf.begin_user_action();
                    buf.delete(&mut s, &mut e);
                    let mut at = buf.iter_at_offset(so);
                    buf.insert(&mut at, &format!("#link(\"{url}\")[{selected}]"));
                    buf.end_user_action();
                }
                _ => v.emit_by_name::<()>("paste-clipboard", &[]),
            }
        });
        glib::Propagation::Stop
    });
}

// ── Move lines (Alt+Up / Alt+Down) ──────────────────────────────────────────

fn wire_move_lines(view: &View, buffer: &Buffer) {
    let buf = buffer.clone();
    let v = view.clone();
    capture_keys(view, move |key, _, mods| {
        let only_alt = mods.contains(ModifierType::ALT_MASK)
            && !mods.intersects(ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK);
        if !only_alt || !matches!(key, Key::Up | Key::Down) {
            return glib::Propagation::Proceed;
        }
        let up = key == Key::Up;
        let insert = buf.iter_at_offset(buf.cursor_position());
        let (first, mut last, sel) = match buf.selection_bounds() {
            Some((s, e)) => {
                let end_line = if e.line_offset() == 0 && e.line() > s.line() {
                    e.line() - 1
                } else {
                    e.line()
                };
                (s.line(), end_line, Some((s.offset(), e.offset())))
            }
            None => (insert.line(), insert.line(), None),
        };
        last = last.max(first);
        let total = buf.line_count();
        if (up && first == 0) || (!up && last + 1 >= total) {
            return glib::Propagation::Stop;
        }
        let neighbour = if up { first - 1 } else { last + 1 };
        let block: Vec<String> = (first..=last).map(|l| line_text(&buf, l)).collect();
        let other = line_text(&buf, neighbour);
        let (region_from, region_to) = if up {
            (neighbour, last)
        } else {
            (first, neighbour)
        };
        let Some(mut from) = buf.iter_at_line(region_from) else {
            return glib::Propagation::Stop;
        };
        if hidden_template_end(&buf, from.offset()).is_some() {
            return glib::Propagation::Stop;
        }
        let mut to = buf.iter_at_line(region_to).unwrap_or(from);
        if !to.ends_line() {
            to.forward_to_line_end();
        }
        let block_text = block.join("\n");
        let replacement = if up {
            format!("{block_text}\n{other}")
        } else {
            format!("{other}\n{block_text}")
        };
        let shift = (other.chars().count() + 1) as i32;
        let shift = if up { -shift } else { shift };
        let cursor_offset = buf.cursor_position();
        buf.begin_user_action();
        let from_off = from.offset();
        buf.delete(&mut from, &mut to);
        let mut at = buf.iter_at_offset(from_off);
        buf.insert(&mut at, &replacement);
        buf.end_user_action();
        match sel {
            Some((s, e)) => {
                let (a, b) = (buf.iter_at_offset(s + shift), buf.iter_at_offset(e + shift));
                buf.select_range(&a, &b);
            }
            None => buf.place_cursor(&buf.iter_at_offset(cursor_offset + shift)),
        }
        v.scroll_to_mark(&buf.get_insert(), 0.1, false, 0.0, 0.5);
        glib::Propagation::Stop
    });
}

// ── Expand / shrink selection (Alt+Shift+Right / Left) ──────────────────────

fn wire_expand_selection(view: &View, buffer: &Buffer) {
    let buf = buffer.clone();
    let v = view.clone();
    let history: Rc<RefCell<Vec<(i32, i32)>>> = Rc::new(RefCell::new(Vec::new()));
    let last: Rc<Cell<(i32, i32)>> = Rc::new(Cell::new((-1, -1)));
    capture_keys(view, move |key, _, mods| {
        let wanted = ModifierType::ALT_MASK | ModifierType::SHIFT_MASK;
        if !mods.contains(wanted)
            || mods.contains(ModifierType::CONTROL_MASK)
            || !matches!(key, Key::Right | Key::Left)
        {
            return glib::Propagation::Proceed;
        }
        let cur = match buf.selection_bounds() {
            Some((s, e)) => (s.offset(), e.offset()),
            None => (buf.cursor_position(), buf.cursor_position()),
        };
        let apply = |(a, b): (i32, i32)| {
            buf.select_range(&buf.iter_at_offset(a), &buf.iter_at_offset(b));
            v.scroll_to_mark(&buf.get_insert(), 0.1, false, 0.0, 0.5);
        };
        if key == Key::Right {
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
            let Some((a, b)) = text_ops::expand_selection(&text, cur.0 as usize, cur.1 as usize)
            else {
                return glib::Propagation::Stop;
            };
            if cur != last.get() {
                history.borrow_mut().clear();
            }
            history.borrow_mut().push(cur);
            let new = (a as i32, b as i32);
            // The template block is not the writer's to select into.
            if hidden_template_end(&buf, new.0).is_some() {
                history.borrow_mut().pop();
                return glib::Propagation::Stop;
            }
            apply(new);
            last.set(new);
        } else if cur == last.get() {
            if let Some(prev) = history.borrow_mut().pop() {
                apply(prev);
                last.set(prev);
            }
        } else {
            return glib::Propagation::Proceed;
        }
        glib::Propagation::Stop
    });
}

// ── Matching delimiter highlight (`*…*`, `_…_`, `$…$`, `"…"`) ──────────────

fn paragraph_window(buf: &Buffer, line: i32) -> (String, i32) {
    const LIMIT: i32 = 80;
    let blank = |l: i32| line_text(buf, l).trim().is_empty();
    let mut first = line;
    while first > 0 && line - first < LIMIT && !blank(first - 1) {
        first -= 1;
    }
    let mut last = line;
    while last + 1 < buf.line_count() && last - line < LIMIT && !blank(last + 1) {
        last += 1;
    }
    let start = buf.iter_at_line(first).unwrap_or_else(|| buf.start_iter());
    let mut end = buf.iter_at_line(last).unwrap_or_else(|| buf.end_iter());
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    (buf.text(&start, &end, false).to_string(), start.offset())
}

fn wire_delimiter_match(view: &View, buffer: &Buffer) {
    let view = view.clone();
    let shown: Rc<Cell<Option<(i32, i32)>>> = Rc::new(Cell::new(None));
    buffer.connect_cursor_position_notify(move |buf| {
        if let (Some((a, b)), Some(tag)) = (shown.take(), buf.tag_table().lookup(DELIM_TAG)) {
            let max = buf.char_count();
            for off in [a, b] {
                if off < max {
                    buf.remove_tag(&tag, &buf.iter_at_offset(off), &buf.iter_at_offset(off + 1));
                }
            }
        }
        if buf.has_selection() {
            return;
        }
        let pos = buf.cursor_position();
        let (text, base) = paragraph_window(buf, buf.iter_at_offset(pos).line());
        for idx in [pos, pos - 1] {
            if idx < base {
                continue;
            }
            let local = (idx - base) as usize;
            let Some(partner) = text_ops::matching_delimiter(&text, local) else {
                continue;
            };
            let table = buf.tag_table();
            let tag = table.lookup(DELIM_TAG).unwrap_or_else(|| {
                let tag = TextTag::new(Some(DELIM_TAG));
                table.add(&tag);
                tag
            });
            let (r, g, b) = super::theme::rgb(&view, "accent_color").unwrap_or((0.2, 0.5, 0.9));
            tag.set_background_rgba(Some(&gtk4::gdk::RGBA::new(
                r as f32, g as f32, b as f32, 0.30,
            )));
            let other = base + partner as i32;
            for off in [idx, other] {
                buf.apply_tag(&tag, &buf.iter_at_offset(off), &buf.iter_at_offset(off + 1));
            }
            shown.set(Some((idx, other)));
            return;
        }
    });
}

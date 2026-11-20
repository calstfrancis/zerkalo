//! The spell-check walk-through: a small window that steps through every
//! misspelled word in the open document, one at a time, and asks what to do
//! with each — change it, skip it, or teach the dictionary.
//!
//! It makes its own pass over the text instead of reading the underlines, so
//! it works with spell check switched off and with "only when I ask". Each
//! word is tracked with a pair of text marks, so changing one word never
//! throws the position of the rest off.

use super::*;
use crate::spellcheck::SpellChecker;
use gtk4::{Align, Entry, ListBox, SelectionMode, TextMark};

struct Item {
    start: TextMark,
    end: TextMark,
    word: String,
    suggestions: Vec<String>,
}

struct Walk {
    buffer: Buffer,
    view: View,
    spell: Rc<RefCell<SpellChecker>>,
    items: Vec<Item>,
    at: Cell<usize>,
    stack: gtk4::Stack,
    progress: Label,
    word: Label,
    context: Label,
    entry: Entry,
    list: ListBox,
    change_all: Button,
    add_project: Button,
    done_text: Label,
}

/// The words around `at` on the same line, with the flagged word in bold.
pub(super) fn context_markup(line: &str, start: usize, end: usize) -> String {
    const SIDE: usize = 48;
    let chars: Vec<char> = line.chars().collect();
    let (start, end) = (start.min(chars.len()), end.min(chars.len()));
    let from = start.saturating_sub(SIDE);
    let to = (end + SIDE).min(chars.len());
    let esc = |a: usize, b: usize| {
        glib::markup_escape_text(&chars[a..b].iter().collect::<String>()).to_string()
    };
    format!(
        "{}{}<b>{}</b>{}{}",
        if from > 0 { "\u{2026}" } else { "" },
        esc(from, start),
        esc(start, end),
        esc(end, to),
        if to < chars.len() { "\u{2026}" } else { "" },
    )
}

impl EditorPane {
    /// Opens the walk-through for the document being edited.
    pub fn open_spell_walk(&self, parent: &gtk4::Window) {
        let Some((view, buffer)) = self.active_view_buffer() else {
            return;
        };
        let (languages, ignored) = {
            let sc = self.spell_checker.borrow();
            (sc.languages.clone(), sc.ignored())
        };
        let (s, e) = buffer.bounds();
        let text = buffer.text(&s, &e, true).to_string();

        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let words = crate::spellcheck::extract_words(&text);
            let mut seen = HashSet::new();
            let unique: Vec<String> = words
                .iter()
                .filter(|(_, _, w)| {
                    !ignored.contains(&w.to_lowercase()) && seen.insert(w.to_lowercase())
                })
                .map(|(_, _, w)| w.clone())
                .collect();
            let refs: Vec<&str> = unique.iter().map(|w| w.as_str()).collect();
            let misspelled = crate::spellcheck::check_words_batch(&refs, &languages);
            let _ = tx.send((words, misspelled));
        });

        let spell = self.spell_checker.clone();
        let parent = parent.clone();
        glib::timeout_add_local(Duration::from_millis(40), move || match rx.try_recv() {
            Ok((words, misspelled)) => {
                let mut items = Vec::new();
                for (start, end, word) in words {
                    if let Some(sugg) = misspelled.get(&word.to_lowercase()) {
                        let s = buffer.iter_at_offset(start as i32);
                        let e = buffer.iter_at_offset(end as i32);
                        items.push(Item {
                            start: buffer.create_mark(None, &s, true),
                            end: buffer.create_mark(None, &e, false),
                            word,
                            suggestions: sugg.iter().take(6).cloned().collect(),
                        });
                    }
                }
                Walk::open(&parent, buffer.clone(), view.clone(), spell.clone(), items);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        });
    }
}

fn flat(label: &str) -> Button {
    let b = Button::with_label(label);
    b.set_hexpand(true);
    b
}

impl Walk {
    fn open(
        parent: &gtk4::Window,
        buffer: Buffer,
        view: View,
        spell: Rc<RefCell<SpellChecker>>,
        items: Vec<Item>,
    ) {
        let window = adw::Window::new();
        window.set_title(Some("Check spelling"));
        window.set_transient_for(Some(parent));
        window.set_default_size(420, 520);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&adw::HeaderBar::new());
        let stack = gtk4::Stack::new();
        stack.set_vexpand(true);
        root.append(&stack);

        // One word at a time.
        let page = GtkBox::new(Orientation::Vertical, 8);
        for set in [
            GtkBox::set_margin_top,
            GtkBox::set_margin_bottom,
            GtkBox::set_margin_start,
            GtkBox::set_margin_end,
        ] {
            set(&page, 14);
        }
        let progress = Label::new(None);
        progress.add_css_class("dim-label");
        progress.add_css_class("caption");
        progress.set_halign(Align::Start);
        let word = Label::new(None);
        word.add_css_class("title-2");
        word.set_halign(Align::Start);
        word.set_selectable(true);
        let context = Label::new(None);
        context.set_halign(Align::Start);
        context.set_wrap(true);
        context.set_xalign(0.0);
        context.add_css_class("dim-label");
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Change it to\u{2026}"));
        let list = ListBox::new();
        list.set_selection_mode(SelectionMode::None);
        list.add_css_class("boxed-list");
        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_child(Some(&list));

        let change = flat("Change");
        change.add_css_class("suggested-action");
        let change_all = flat("Change all");
        let skip = flat("Skip");
        let skip_all = flat("Skip all");
        let add = flat("Add to dictionary");
        let add_project = flat("Add to project dictionary");
        let back = flat("Back");

        let row = |a: &Button, b: &Button| {
            let r = GtkBox::new(Orientation::Horizontal, 8);
            r.set_homogeneous(true);
            r.append(a);
            r.append(b);
            r
        };
        page.append(&progress);
        page.append(&word);
        page.append(&context);
        page.append(&entry);
        page.append(&scroll);
        page.append(&row(&change, &change_all));
        page.append(&row(&skip, &skip_all));
        page.append(&row(&add, &add_project));
        page.append(&back);
        stack.add_named(&page, Some("word"));

        // Nothing left.
        let done = GtkBox::new(Orientation::Vertical, 12);
        done.set_valign(Align::Center);
        done.set_margin_start(24);
        done.set_margin_end(24);
        let done_text = Label::new(None);
        done_text.add_css_class("title-3");
        done_text.set_wrap(true);
        done_text.set_justify(gtk4::Justification::Center);
        let close = Button::with_label("Close");
        close.add_css_class("suggested-action");
        close.set_halign(Align::Center);
        done.append(&done_text);
        done.append(&close);
        stack.add_named(&done, Some("done"));

        window.set_content(Some(&root));

        let walk = Rc::new(Walk {
            buffer,
            view,
            spell,
            items,
            at: Cell::new(0),
            stack,
            progress,
            word,
            context,
            entry: entry.clone(),
            list,
            change_all: change_all.clone(),
            add_project: add_project.clone(),
            done_text,
        });

        let w = walk.clone();
        change.connect_clicked(move |_| w.change(false));
        let w = walk.clone();
        entry.connect_activate(move |_| w.change(false));
        let w = walk.clone();
        change_all.connect_clicked(move |_| w.change(true));
        let w = walk.clone();
        skip.connect_clicked(move |_| w.step(true));
        let w = walk.clone();
        skip_all.connect_clicked(move |_| w.teach(false));
        let w = walk.clone();
        add.connect_clicked(move |_| w.teach(true));
        let w = walk.clone();
        add_project.connect_clicked(move |_| w.teach_project());
        let w = walk.clone();
        back.connect_clicked(move |_| w.step(false));
        let win = window.clone();
        close.connect_clicked(move |_| win.close());

        let esc = EventControllerKey::new();
        let win = window.clone();
        esc.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                win.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(esc);

        let w = walk.clone();
        window.connect_close_request(move |_| {
            for item in &w.items {
                w.buffer.delete_mark(&item.start);
                w.buffer.delete_mark(&item.end);
            }
            glib::Propagation::Proceed
        });

        window.present();
        walk.add_project
            .set_visible(walk.spell.borrow().has_project_dict());
        walk.go(0, true);
    }

    fn live(&self, i: usize) -> bool {
        let item = &self.items[i];
        !item.start.is_deleted()
            && !item.end.is_deleted()
            && !self.spell.borrow().is_ignored(&item.word)
            && self.buffer.iter_at_mark(&item.start) < self.buffer.iter_at_mark(&item.end)
    }

    /// Shows the first word still needing a decision at or after `from` (or,
    /// going back, at or before it).
    fn go(&self, from: usize, forward: bool) {
        let n = self.items.len();
        let found = if forward {
            (from..n).find(|&i| self.live(i))
        } else {
            (0..=from.min(n.saturating_sub(1)))
                .rev()
                .find(|&i| n > 0 && self.live(i))
        };
        match found {
            Some(i) => {
                self.at.set(i);
                self.show(i);
            }
            None if !forward => {}
            None => {
                self.done_text.set_text(if n == 0 {
                    "No misspelled words."
                } else {
                    "That was the last misspelled word."
                });
                self.stack.set_visible_child_name("done");
            }
        }
    }

    /// Moves on from the current word (`forward`) or back to the one before.
    fn step(&self, forward: bool) {
        let at = self.at.get();
        if forward {
            self.go(at + 1, true);
        } else if at > 0 {
            self.go(at - 1, false);
        }
    }

    fn show(&self, i: usize) {
        let item = &self.items[i];
        self.stack.set_visible_child_name("word");
        let left = (0..self.items.len()).filter(|&j| self.live(j)).count();
        self.progress.set_text(&format!("{left} left to check"));
        self.word.set_text(&item.word);

        let s = self.buffer.iter_at_mark(&item.start);
        let e = self.buffer.iter_at_mark(&item.end);
        let mut ls = s;
        ls.set_line_offset(0);
        let mut le = s;
        if !le.ends_line() {
            le.forward_to_line_end();
        }
        let line = self.buffer.text(&ls, &le, false).to_string();
        self.context.set_markup(&context_markup(
            &line,
            s.line_offset() as usize,
            (s.line_offset() + (e.offset() - s.offset())) as usize,
        ));

        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        for sug in &item.suggestions {
            let b = Button::with_label(sug);
            b.add_css_class("flat");
            if let Some(l) = b.child().and_then(|c| c.downcast::<Label>().ok()) {
                l.set_xalign(0.0);
            }
            let entry = self.entry.clone();
            let text = sug.clone();
            b.connect_clicked(move |_| entry.set_text(&text));
            self.list.append(&b);
        }
        let same = (i..self.items.len())
            .filter(|&j| {
                self.live(j) && self.items[j].word.to_lowercase() == item.word.to_lowercase()
            })
            .count();
        self.change_all.set_sensitive(same > 1);
        self.entry.set_text(
            item.suggestions
                .first()
                .map(|s| s.as_str())
                .unwrap_or(&item.word),
        );
        self.entry.grab_focus();
        self.entry.select_region(0, -1);

        self.buffer.select_range(&s, &e);
        self.view.scroll_to_mark(&item.start, 0.0, true, 0.0, 0.5);
    }

    fn replace(&self, i: usize, with: &str) {
        let item = &self.items[i];
        let mut s = self.buffer.iter_at_mark(&item.start);
        let mut e = self.buffer.iter_at_mark(&item.end);
        self.buffer.begin_user_action();
        self.buffer.delete(&mut s, &mut e);
        self.buffer.insert(&mut s, with);
        self.buffer.end_user_action();
    }

    fn change(&self, all: bool) {
        let with = self.entry.text().to_string();
        let i = self.at.get();
        if with.trim().is_empty() {
            return;
        }
        let word = self.items[i].word.to_lowercase();
        self.replace(i, &with);
        if all {
            for j in i + 1..self.items.len() {
                if self.items[j].word.to_lowercase() == word && self.live(j) {
                    self.replace(j, &with);
                }
            }
        }
        self.go(i + 1, true);
    }

    fn forget_underlines(&self, word: &str) {
        if let Some(tag) = self.buffer.tag_table().lookup("zerkalo-spell") {
            remove_spell_word_tags(&self.buffer, &tag, word);
        }
    }

    /// "Skip all" (`dictionary` false) or "Add to dictionary".
    fn teach(&self, dictionary: bool) {
        let i = self.at.get();
        let word = self.items[i].word.clone();
        {
            let mut sc = self.spell.borrow_mut();
            if dictionary {
                sc.add_to_user_dict(&word);
            } else {
                sc.ignore(&word);
            }
        }
        self.forget_underlines(&word);
        self.go(i + 1, true);
    }

    fn teach_project(&self) {
        let i = self.at.get();
        let word = self.items[i].word.clone();
        self.spell.borrow_mut().add_to_project_dict(&word);
        self.forget_underlines(&word);
        self.go(i + 1, true);
    }
}

#[cfg(test)]
mod tests {
    use super::context_markup;

    #[test]
    fn the_flagged_word_is_bold_and_markup_is_escaped() {
        assert_eq!(
            context_markup("a <b> teh c", 6, 9),
            "a &lt;b&gt; <b>teh</b> c"
        );
    }

    #[test]
    fn a_long_line_is_trimmed_around_the_word() {
        let line = format!("{} teh {}", "x".repeat(100), "y".repeat(100));
        let m = context_markup(&line, 101, 104);
        assert!(m.starts_with('\u{2026}') && m.ends_with('\u{2026}'));
        assert!(m.contains("<b>teh</b>"));
    }
}

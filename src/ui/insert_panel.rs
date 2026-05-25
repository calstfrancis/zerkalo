use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Orientation, Separator};

// (button_label, tooltip, snippet)
const SNIPPETS: &[(&str, &str, &str)] = &[
    ("H1",   "Level-1 Heading (= …)",      "= Heading\n"),
    ("H2",   "Level-2 Heading (== …)",     "== Sub-heading\n"),
    ("H3",   "Level-3 Heading (=== …)",    "=== Sub-sub-heading\n"),
    ("---",  "",                            ""),
    ("**",   "Bold (*…*)",                  "*bold*"),
    ("__",   "Italic (_…_)",                "_italic_"),
    ("~~",   "Strikethrough (#strike(…))",  "#strike[text]"),
    ("``",   "Inline code",                 "`code`"),
    ("---",  "",                            ""),
    ("∫",    "Inline math",                 "$x$"),
    ("∫∫",   "Display math",               "$\n  x = y\n$\n"),
    ("---",  "",                            ""),
    ("fig",  "Figure",
             "#figure(\n  image(\"filename.png\"),\n  caption: [Caption],\n)\n"),
    ("tbl",  "Table",
             "#table(\n  columns: (1fr, 1fr),\n  table.header([*A*], [*B*]),\n  [], [],\n)\n"),
    ("lst",  "Bullet list",                "- Item 1\n- Item 2\n"),
    ("1.",   "Numbered list",              "+ Item 1\n+ Item 2\n"),
    ("---",  "",                            ""),
    ("col",  "Two-column layout",
             "#columns(2)[\n  Left column.\n  #colbreak()\n  Right column.\n]\n"),
    ("pgb",  "Page break",                 "#pagebreak()\n"),
    ("lnk",  "Hyperlink",                  "#link(\"https://example.com\")[text]"),
    ("@",    "Cross-reference",            "@label"),
    ("lbl",  "Label",                      "<label>"),
];

#[derive(Clone)]
pub struct InsertPanel {
    pub widget: GtkBox,
    on_insert: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
}

impl InsertPanel {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 2);
        widget.set_width_request(52);
        widget.set_vexpand(true);
        widget.set_margin_top(4);
        widget.set_margin_bottom(4);
        widget.set_margin_start(2);
        widget.set_margin_end(2);

        let on_insert: Rc<RefCell<Option<Box<dyn Fn(&str)>>>> = Rc::new(RefCell::new(None));

        for &(lbl, tip, snippet) in SNIPPETS {
            if lbl == "---" {
                let sep = Separator::new(Orientation::Horizontal);
                sep.set_margin_top(2);
                sep.set_margin_bottom(2);
                widget.append(&sep);
                continue;
            }

            let btn = Button::new();
            let inner = Label::new(Some(lbl));
            inner.set_width_chars(4);
            inner.add_css_class("monospace");
            btn.set_child(Some(&inner));
            btn.add_css_class("flat");
            if !tip.is_empty() {
                btn.set_tooltip_text(Some(tip));
            }
            btn.set_size_request(48, 34);

            let cb = on_insert.clone();
            let s = snippet.to_string();
            btn.connect_clicked(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f(&s);
                }
            });

            widget.append(&btn);
        }

        Self { widget, on_insert }
    }

    pub fn set_on_insert(&self, f: impl Fn(&str) + 'static) {
        *self.on_insert.borrow_mut() = Some(Box::new(f));
    }
}

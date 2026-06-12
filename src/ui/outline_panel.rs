use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, FlowBox, Image, Label, ListBox, ListBoxRow, Notebook, Orientation,
    ScrolledWindow, SelectionMode, Separator, Stack, ToggleButton,
};

type JumpCb = Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>;
type InsertCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

#[derive(Clone)]
pub struct OutlinePanel {
    widget: GtkBox,
    list_box: ListBox,
    on_jump: JumpCb,
    on_symbol_insert: InsertCb,
    #[allow(dead_code)] stack: Stack,
    #[allow(dead_code)] outline_btn: ToggleButton,
    #[allow(dead_code)] symbols_btn: ToggleButton,
    /// (file_path, line_number) for each outline row — supports single and multi-file.
    row_positions: Rc<RefCell<Vec<(PathBuf, u32)>>>,
    max_depth: Rc<Cell<u32>>,
    /// Cached input for depth-filter re-renders.
    cached_files: Rc<RefCell<Vec<(PathBuf, String)>>>,
}

impl OutlinePanel {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);
        widget.set_vexpand(true);

        // ── Gost-style segmented control: Outline | Symbols ──────────────────
        let seg_box = GtkBox::new(Orientation::Horizontal, 0);
        seg_box.add_css_class("linked");
        seg_box.set_margin_start(8);
        seg_box.set_margin_end(8);
        seg_box.set_margin_top(8);
        seg_box.set_margin_bottom(8);

        let outline_btn = ToggleButton::new();
        {
            let img = Image::from_icon_name("view-list-symbolic");
            img.set_pixel_size(20);
            outline_btn.set_child(Some(&img));
        }
        outline_btn.set_tooltip_text(Some("Document outline"));
        outline_btn.set_hexpand(true);
        outline_btn.set_active(true);

        let symbols_btn = ToggleButton::new();
        {
            let img = Image::from_icon_name("input-keyboard-symbolic");
            img.set_pixel_size(20);
            symbols_btn.set_child(Some(&img));
        }
        symbols_btn.set_tooltip_text(Some("Insert symbols"));
        symbols_btn.set_hexpand(true);
        symbols_btn.set_group(Some(&outline_btn));

        seg_box.append(&outline_btn);
        seg_box.append(&symbols_btn);

        widget.append(&seg_box);
        widget.append(&Separator::new(Orientation::Horizontal));

        // ── Depth filter row ─────────────────────────────────────────────────
        let max_depth: Rc<Cell<u32>> = Rc::new(Cell::new(u32::MAX));

        let depth_box = GtkBox::new(Orientation::Horizontal, 0);
        depth_box.add_css_class("linked");
        depth_box.set_margin_start(8);
        depth_box.set_margin_end(8);
        depth_box.set_margin_top(4);
        depth_box.set_margin_bottom(4);

        let depth_lbl = Label::new(Some("Depth: "));
        depth_lbl.add_css_class("caption");
        depth_lbl.add_css_class("dim-label");
        depth_lbl.set_margin_end(4);
        depth_box.append(&depth_lbl);

        let all_btn = ToggleButton::with_label("All");
        all_btn.set_active(true);
        all_btn.add_css_class("flat");
        all_btn.add_css_class("caption");

        // Depth buttons — closures stored for wiring after Self is built
        let depth_buttons: Vec<(ToggleButton, u32)> = [("H1", 1u32), ("H1–2", 2), ("H1–3", 3)]
            .iter()
            .map(|(label, depth)| {
                let btn = ToggleButton::with_label(label);
                btn.set_group(Some(&all_btn));
                btn.add_css_class("flat");
                btn.add_css_class("caption");
                depth_box.append(&btn);
                (btn, *depth)
            })
            .collect();
        depth_box.append(&all_btn);

        widget.append(&depth_box);
        widget.append(&Separator::new(Orientation::Horizontal));

        let stack = Stack::new();
        stack.set_vexpand(true);

        // ── Outline page ─────────────────────────────────────────────────────

        let outline_scroll = ScrolledWindow::new();
        outline_scroll.set_vexpand(true);
        outline_scroll.set_hexpand(true);
        outline_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Single);
        list_box.add_css_class("navigation-sidebar");
        outline_scroll.set_child(Some(&list_box));

        stack.add_named(&outline_scroll, Some("outline"));

        // ── Symbol insert page ───────────────────────────────────────────────

        let sym_notebook = Notebook::new();
        sym_notebook.set_scrollable(true);
        sym_notebook.set_vexpand(true);

        let on_symbol_insert: InsertCb = Rc::new(RefCell::new(None));

        for (tab_name, chars) in symbol_tabs() {
            let scroll = ScrolledWindow::new();
            scroll.set_vexpand(true);
            scroll.set_hexpand(true);

            let flow = FlowBox::new();
            flow.set_homogeneous(true);
            flow.set_row_spacing(0);
            flow.set_column_spacing(0);
            flow.set_selection_mode(SelectionMode::None);
            flow.set_margin_start(4);
            flow.set_margin_end(4);
            flow.set_margin_top(4);
            flow.set_margin_bottom(4);
            flow.set_max_children_per_line(8);
            flow.set_min_children_per_line(2);

            for (ch, name) in chars {
                let cb = on_symbol_insert.clone();
                let ch_s = ch.to_string();
                let btn = gtk4::Button::with_label(ch);
                btn.add_css_class("flat");
                let cp = ch.chars().next().map(|c| c as u32).unwrap_or(0);
                btn.set_tooltip_text(Some(&format!("{name} ({ch}) · U+{cp:04X}")));
                btn.connect_clicked(move |_| {
                    if let Some(f) = cb.borrow().as_ref() {
                        f(ch_s.clone());
                    }
                });
                flow.append(&btn);
            }

            scroll.set_child(Some(&flow));
            let tab_lbl = gtk4::Label::new(Some(tab_name));
            tab_lbl.add_css_class("caption");
            sym_notebook.append_page(&scroll, Some(&tab_lbl));
        }

        stack.add_named(&sym_notebook, Some("symbols"));
        stack.set_visible_child_name("outline");

        // Wire segmented control → stack
        {
            let stack_c = stack.clone();
            outline_btn.connect_toggled(move |btn| {
                if btn.is_active() {
                    stack_c.set_visible_child_name("outline");
                }
            });
        }
        {
            let stack_c = stack.clone();
            symbols_btn.connect_toggled(move |btn| {
                if btn.is_active() {
                    stack_c.set_visible_child_name("symbols");
                }
            });
        }

        widget.append(&stack);

        let on_jump: JumpCb = Rc::new(RefCell::new(None));
        let row_positions: Rc<RefCell<Vec<(PathBuf, u32)>>> = Rc::new(RefCell::new(Vec::new()));

        {
            let on_jump_c = on_jump.clone();
            let row_positions_c = row_positions.clone();
            list_box.connect_row_activated(move |_, row| {
                let idx = row.index() as usize;
                let positions = row_positions_c.borrow();
                if let Some((path, ln)) = positions.get(idx).cloned() {
                    if let Some(f) = on_jump_c.borrow().as_ref() {
                        f(path, ln);
                    }
                }
            });
        }

        let panel = Self {
            widget, list_box, on_jump, on_symbol_insert, stack, outline_btn, symbols_btn,
            row_positions, max_depth,
            cached_files: Rc::new(RefCell::new(Vec::new())),
        };

        // Wire depth toggle buttons
        for (btn, depth) in depth_buttons {
            let p = panel.clone();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    p.max_depth.set(depth);
                    let files = p.cached_files.borrow().clone();
                    p.repopulate(&files);
                }
            });
        }
        {
            let p = panel.clone();
            all_btn.connect_toggled(move |b| {
                if b.is_active() {
                    p.max_depth.set(u32::MAX);
                    let files = p.cached_files.borrow().clone();
                    p.repopulate(&files);
                }
            });
        }

        panel
    }

    #[allow(dead_code)]
    pub fn set_mode(&self, mode: &str) {
        self.stack.set_visible_child_name(mode);
        match mode {
            "outline" => self.outline_btn.set_active(true),
            "symbols" => self.symbols_btn.set_active(true),
            _ => {}
        }
    }

    /// Update outline from a single file (single-document mode).
    pub fn update(&self, content: &str, path: &PathBuf) {
        let files = vec![(path.clone(), content.to_string())];
        *self.cached_files.borrow_mut() = files.clone();
        self.repopulate(&files);
    }

    /// Update outline from all project files (multi-file mode).
    /// Files should be in document order (root first, then included files).
    #[allow(dead_code)]
    pub fn update_project(&self, files: Vec<(PathBuf, String)>) {
        *self.cached_files.borrow_mut() = files.clone();
        self.repopulate(&files);
    }

    fn repopulate(&self, files: &[(PathBuf, String)]) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let max_depth = self.max_depth.get();
        let multi_file = files.len() > 1;
        let mut positions_vec: Vec<(PathBuf, u32)> = Vec::new();

        for (path, content) in files {
            let all_lines: Vec<&str> = content.lines().collect();
            let n = all_lines.len();
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Collect headings: (line_idx, level, text)
            let headings: Vec<(usize, usize, String)> = all_lines.iter()
                .enumerate()
                .filter_map(|(i, line)| {
                    if !line.starts_with('=') { return None; }
                    let stripped = line.trim_start_matches('=');
                    let level = line.len() - stripped.len();
                    if level == 0 || !stripped.starts_with(' ') { return None; }
                    if level as u32 > max_depth { return None; }
                    let text = stripped.trim_start().to_string();
                    if text.is_empty() { return None; }
                    Some((i, level, text))
                })
                .collect();

            if headings.is_empty() { continue; }

            for (h_idx, (line_idx, level, text)) in headings.iter().enumerate() {
                let next_line_idx = headings.get(h_idx + 1).map(|(li, _, _)| *li).unwrap_or(n);
                let word_count: u32 = all_lines[line_idx + 1..next_line_idx]
                    .iter()
                    .map(|l| count_words_typst(l))
                    .sum();

                let ln = (line_idx + 1) as u32;
                positions_vec.push((path.clone(), ln));

                let row = ListBoxRow::new();
                row.set_activatable(true);

                let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);

                let label = gtk4::Label::new(Some(text));
                label.set_xalign(0.0);
                label.set_hexpand(true);
                label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                label.set_margin_start(8 + (*level as i32 - 1) * 14);
                label.set_margin_end(4);
                label.set_margin_top(4);
                label.set_margin_bottom(4);
                if *level == 1 {
                    label.add_css_class("heading");
                }

                let count_lbl = gtk4::Label::new(Some(&word_count.to_string()));
                count_lbl.add_css_class("dim-label");
                count_lbl.add_css_class("caption");
                count_lbl.set_margin_end(8);
                count_lbl.set_valign(gtk4::Align::Center);

                row_box.append(&label);
                row_box.append(&count_lbl);

                // In multi-file mode show file name as tooltip instead of inline label.
                if multi_file {
                    row.set_tooltip_text(Some(&file_name));
                }

                row.set_child(Some(&row_box));
                self.list_box.append(&row);
            }
        }

        *self.row_positions.borrow_mut() = positions_vec;
    }

    /// Select the outline row nearest to (and not past) `line` in `path`.
    pub fn select_for_line(&self, path: &PathBuf, line: u32) {
        let positions = self.row_positions.borrow();
        let idx = positions.iter().rposition(|(p, l)| p == path && *l <= line);
        match idx {
            Some(i) => {
                self.list_box.select_row(self.list_box.row_at_index(i as i32).as_ref());
            }
            None => {
                self.list_box.unselect_all();
            }
        }
    }

    pub fn set_on_jump(&self, f: impl Fn(PathBuf, u32) + 'static) {
        *self.on_jump.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_on_symbol_insert(&self, f: impl Fn(String) + 'static) {
        *self.on_symbol_insert.borrow_mut() = Some(Box::new(f));
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }
}

// ── Symbol character sets ─────────────────────────────────────────────────────

fn symbol_tabs() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        ("Cyr", vec![
            ("А","A"), ("Б","B"), ("В","V"), ("Г","G"), ("Д","D"),
            ("Е","Ye"), ("Ж","Zh"), ("З","Z"), ("И","I"), ("К","K"),
            ("Л","L"), ("М","M"), ("Н","N"), ("О","O"), ("П","P"),
            ("Р","R"), ("С","S"), ("Т","T"), ("У","U"), ("Ф","F"),
            ("Х","Kh"), ("Ц","Ts"), ("Ч","Ch"), ("Ш","Sh"), ("Щ","Shch"),
            ("Ъ","Hard sign"), ("Ы","Yeru"), ("Ь","Soft sign"), ("Э","E"), ("Ю","Yu"), ("Я","Ya"),
            ("а","a"), ("б","b"), ("в","v"), ("г","g"), ("д","d"),
            ("е","ye"), ("ж","zh"), ("з","z"), ("и","i"), ("к","k"),
            ("л","l"), ("м","m"), ("н","n"), ("о","o"), ("п","p"),
            ("р","r"), ("с","s"), ("т","t"), ("у","u"), ("ф","f"),
            ("х","kh"), ("ц","ts"), ("ч","ch"), ("ш","sh"), ("щ","shch"),
            ("ъ","hard sign"), ("ы","yeru"), ("ь","soft sign"), ("э","e"), ("ю","yu"), ("я","ya"),
            ("Ё","Yo"), ("ё","yo"), ("Є","Ukrainian Ye"), ("є","ukrainian ye"),
            ("І","Ukrainian I"), ("і","ukrainian i"), ("Ї","Yi"), ("ї","yi"),
            ("Ѕ","Dze"), ("ѕ","dze"), ("Ѡ","Omega"), ("ѡ","omega"),
            ("Ѣ","Yat"), ("ѣ","yat"), ("Ѧ","Little Yus"), ("ѧ","little yus"),
            ("Ѩ","Iotified Little Yus"), ("ѩ","iotified little yus"),
            ("Ѫ","Big Yus"), ("ѫ","big yus"), ("Ѭ","Iotified Big Yus"), ("ѭ","iotified big yus"),
            ("Ѯ","Ksi"), ("ѯ","ksi"), ("Ѱ","Psi"), ("ѱ","psi"),
            ("Ѳ","Fita"), ("ѳ","fita"), ("Ѵ","Izhitsa"), ("ѵ","izhitsa"),
        ]),
        ("Greek", vec![
            ("Α","Alpha"), ("Β","Beta"), ("Γ","Gamma"), ("Δ","Delta"), ("Ε","Epsilon"),
            ("Ζ","Zeta"), ("Η","Eta"), ("Θ","Theta"), ("Ι","Iota"), ("Κ","Kappa"),
            ("Λ","Lambda"), ("Μ","Mu"), ("Ν","Nu"), ("Ξ","Xi"), ("Ο","Omicron"),
            ("Π","Pi"), ("Ρ","Rho"), ("Σ","Sigma"), ("Τ","Tau"), ("Υ","Upsilon"),
            ("Φ","Phi"), ("Χ","Chi"), ("Ψ","Psi"), ("Ω","Omega"),
            ("α","alpha"), ("β","beta"), ("γ","gamma"), ("δ","delta"), ("ε","epsilon"),
            ("ζ","zeta"), ("η","eta"), ("θ","theta"), ("ι","iota"), ("κ","kappa"),
            ("λ","lambda"), ("μ","mu"), ("ν","nu"), ("ξ","xi"), ("ο","omicron"),
            ("π","pi"), ("ρ","rho"), ("σ","sigma"), ("ς","final sigma"), ("τ","tau"),
            ("υ","upsilon"), ("φ","phi"), ("χ","chi"), ("ψ","psi"), ("ω","omega"),
            ("ά","alpha + tonos"), ("έ","epsilon + tonos"), ("ή","eta + tonos"),
            ("ί","iota + tonos"), ("ό","omicron + tonos"), ("ύ","upsilon + tonos"),
            ("ώ","omega + tonos"), ("ΐ","iota + dialytika + tonos"), ("ΰ","upsilon + dialytika + tonos"),
            ("ἀ","alpha + smooth"), ("ἁ","alpha + rough"), ("ἂ","alpha + smooth + grave"),
            ("ἃ","alpha + rough + grave"), ("ἄ","alpha + smooth + acute"), ("ἅ","alpha + rough + acute"),
            ("ἆ","alpha + smooth + circumflex"), ("ἇ","alpha + rough + circumflex"),
            ("ἐ","epsilon + smooth"), ("ἑ","epsilon + rough"), ("ἒ","epsilon + smooth + grave"),
            ("ἓ","epsilon + rough + grave"), ("ἔ","epsilon + smooth + acute"), ("ἕ","epsilon + rough + acute"),
            ("ἠ","eta + smooth"), ("ἡ","eta + rough"), ("ἢ","eta + smooth + grave"),
            ("ἣ","eta + rough + grave"), ("ἤ","eta + smooth + acute"), ("ἥ","eta + rough + acute"),
            ("ἦ","eta + smooth + circumflex"), ("ἧ","eta + rough + circumflex"),
            ("ἰ","iota + smooth"), ("ἱ","iota + rough"), ("ἲ","iota + smooth + grave"),
            ("ἳ","iota + rough + grave"), ("ἴ","iota + smooth + acute"), ("ἵ","iota + rough + acute"),
            ("ἶ","iota + smooth + circumflex"), ("ἷ","iota + rough + circumflex"),
            ("ὀ","omicron + smooth"), ("ὁ","omicron + rough"), ("ὂ","omicron + smooth + grave"),
            ("ὃ","omicron + rough + grave"), ("ὄ","omicron + smooth + acute"), ("ὅ","omicron + rough + acute"),
            ("ὐ","upsilon + smooth"), ("ὑ","upsilon + rough"), ("ὒ","upsilon + smooth + grave"),
            ("ὓ","upsilon + rough + grave"), ("ὔ","upsilon + smooth + acute"), ("ὕ","upsilon + rough + acute"),
            ("ὖ","upsilon + smooth + circumflex"), ("ὗ","upsilon + rough + circumflex"),
            ("ὠ","omega + smooth"), ("ὡ","omega + rough"), ("ὢ","omega + smooth + grave"),
            ("ὣ","omega + rough + grave"), ("ὤ","omega + smooth + acute"), ("ὥ","omega + rough + acute"),
            ("ὦ","omega + smooth + circumflex"), ("ὧ","omega + rough + circumflex"),
            ("ᾳ","alpha + iota subscript"), ("ῃ","eta + iota subscript"), ("ῳ","omega + iota subscript"),
            ("·","middle dot"), ("῾","rough breathing mark"), ("᾿","smooth breathing mark"),
            ("͂","combining circumflex"), ("̈","combining diaeresis"),
        ]),
        ("Heb", vec![
            ("א","Alef"), ("ב","Bet"), ("ג","Gimel"), ("ד","Dalet"), ("ה","He"),
            ("ו","Vav"), ("ז","Zayin"), ("ח","Het"), ("ט","Tet"), ("י","Yod"),
            ("כ","Kaf"), ("ל","Lamed"), ("מ","Mem"), ("נ","Nun"), ("ס","Samekh"),
            ("ע","Ayin"), ("פ","Pe"), ("צ","Tsadi"), ("ק","Qof"), ("ר","Resh"),
            ("ש","Shin"), ("ת","Tav"),
            ("ך","Final Kaf"), ("ם","Final Mem"), ("ן","Final Nun"), ("ף","Final Pe"), ("ץ","Final Tsadi"),
            ("שׁ","Shin dot"), ("שׂ","Sin dot"), ("בּ","Bet + dagesh"), ("גּ","Gimel + dagesh"),
            ("דּ","Dalet + dagesh"), ("הּ","He + dagesh"), ("וּ","Vav + dagesh"), ("זּ","Zayin + dagesh"),
            ("טּ","Tet + dagesh"), ("יּ","Yod + dagesh"), ("כּ","Kaf + dagesh"), ("לּ","Lamed + dagesh"),
            ("מּ","Mem + dagesh"), ("נּ","Nun + dagesh"), ("סּ","Samekh + dagesh"), ("פּ","Pe + dagesh"),
            ("צּ","Tsadi + dagesh"), ("קּ","Qof + dagesh"), ("רּ","Resh + dagesh"), ("תּ","Tav + dagesh"),
            ("ָ","Qamats"), ("ַ","Patah"), ("ֵ","Tsere"), ("ֶ","Segol"), ("ִ","Hiriq"),
            ("ֻ","Qubuts"), ("ּ","Dagesh"), ("ְ","Sheva"), ("ֱ","Hataf Segol"),
            ("ֲ","Hataf Patah"), ("ֳ","Hataf Qamats"), ("ׁ","Shin dot"), ("ׂ","Sin dot"),
            ("׃","Sof Pasuq"), ("׀","Paseq"),
        ]),
        ("Sans", vec![
            ("अ","a"), ("आ","ā"), ("इ","i"), ("ई","ī"), ("उ","u"), ("ऊ","ū"),
            ("ऋ","ṛ"), ("ए","e"), ("ऐ","ai"), ("ओ","o"), ("औ","au"),
            ("अं","aṃ (anusvara)"), ("अः","aḥ (visarga)"),
            ("क","ka"), ("ख","kha"), ("ग","ga"), ("घ","gha"), ("ङ","ṅa"),
            ("च","ca"), ("छ","cha"), ("ज","ja"), ("झ","jha"), ("ञ","ña"),
            ("ट","ṭa"), ("ठ","ṭha"), ("ड","ḍa"), ("ढ","ḍha"), ("ण","ṇa"),
            ("त","ta"), ("थ","tha"), ("द","da"), ("ध","dha"), ("न","na"),
            ("प","pa"), ("फ","pha"), ("ब","ba"), ("भ","bha"), ("म","ma"),
            ("य","ya"), ("र","ra"), ("ल","la"), ("व","va"),
            ("श","śa"), ("ष","ṣa"), ("स","sa"), ("ह","ha"),
            ("ा","ā matra"), ("ि","i matra"), ("ी","ī matra"), ("ु","u matra"),
            ("ू","ū matra"), ("ृ","ṛ matra"), ("े","e matra"), ("ै","ai matra"),
            ("ो","o matra"), ("ौ","au matra"), ("ं","anusvara"), ("ः","visarga"),
            ("्","virāma"), ("ँ","chandrabindu"),
            ("०","0"), ("१","1"), ("२","2"), ("३","3"), ("४","4"),
            ("५","5"), ("६","6"), ("७","7"), ("८","8"), ("९","9"),
            ("।","danda"), ("॥","double danda"), ("ॐ","Om"),
        ]),
    ]
}

fn count_words_typst(text: &str) -> u32 {
    let mut count = 0u32;
    let mut remaining = text;
    while !remaining.is_empty() {
        if let Some(pos) = remaining.find("#lorem(") {
            count += remaining[..pos].split_whitespace().count() as u32;
            let after = &remaining[pos + 7..];
            if let Some(end) = after.find(')') {
                if let Ok(n) = after[..end].trim().parse::<u32>() {
                    count += n;
                }
                remaining = &after[end + 1..];
            } else {
                break;
            }
        } else {
            count += remaining.split_whitespace().count() as u32;
            break;
        }
    }
    count
}

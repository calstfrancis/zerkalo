use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, FlowBox, ListBox, ListBoxRow, Notebook, Orientation, ScrolledWindow,
    SelectionMode, Stack,
};

type JumpCb = Rc<RefCell<Option<Box<dyn Fn(PathBuf, u32)>>>>;
type InsertCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

#[derive(Clone)]
pub struct OutlinePanel {
    widget: GtkBox,
    list_box: ListBox,
    on_jump: JumpCb,
    on_symbol_insert: InsertCb,
    stack: Stack,
}

impl OutlinePanel {
    pub fn new() -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let stack = Stack::new();
        stack.set_vexpand(true);

        // ── Outline page ─────────────────────────────────────────────────────

        let outline_scroll = ScrolledWindow::new();
        outline_scroll.set_vexpand(true);
        outline_scroll.set_hexpand(true);

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::None);
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
            flow.set_min_children_per_line(4);

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

        widget.append(&stack);

        let on_jump: JumpCb = Rc::new(RefCell::new(None));

        Self { widget, list_box, on_jump, on_symbol_insert, stack }
    }

    pub fn set_mode(&self, mode: &str) {
        self.stack.set_visible_child_name(mode);
    }

    pub fn update(&self, content: &str, path: &PathBuf) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        for (i, line) in content.lines().enumerate() {
            if !line.starts_with('=') {
                continue;
            }
            let stripped = line.trim_start_matches('=');
            let level = line.len() - stripped.len();
            if level == 0 || !stripped.starts_with(' ') {
                continue;
            }
            let text = stripped.trim_start().to_string();
            if text.is_empty() {
                continue;
            }

            let row = ListBoxRow::new();
            row.set_activatable(true);

            let label = gtk4::Label::new(Some(&text));
            label.set_xalign(0.0);
            label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            label.set_margin_start(8 + (level as i32 - 1) * 14);
            label.set_margin_end(8);
            label.set_margin_top(4);
            label.set_margin_bottom(4);
            if level == 1 {
                label.add_css_class("heading");
            }
            row.set_child(Some(&label));

            let cb = self.on_jump.clone();
            let p = path.clone();
            let ln = (i + 1) as u32;
            row.connect_activate(move |_| {
                if let Some(f) = cb.borrow().as_ref() {
                    f(p.clone(), ln);
                }
            });

            self.list_box.append(&row);
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

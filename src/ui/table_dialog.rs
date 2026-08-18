//! A form-driven `#table(...)` generator — dialog/generator model (matching
//! `template_dialog`'s "generate a form, emit code" pattern), not a live
//! preview-overlay editor. Generates a complete table block and hands
//! control back to the text editor; no live-editing of an already-inserted
//! table's structure (a natural follow-up once this model is proven, not
//! part of this dialog).

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, DropDown, Entry, Grid, Label, Orientation,
    ScrolledWindow, SpinButton, StringList,
};
use libadwaita as adw;
use adw::prelude::*;

const MIN_DIM: u32 = 1;
const MAX_DIM: u32 = 20;
const MAX_SPAN: u32 = 20;

#[derive(Clone)]
struct CellWidgets {
    // The box actually attached to the grid — `gtk_grid_remove` only
    // operates on direct grid children, so this (not `content`/`colspan`/
    // `rowspan`, which are grandchildren of the grid via this wrapper) is
    // what shrinking a row/column must remove.
    wrapper: GtkBox,
    content: Entry,
    colspan: SpinButton,
    rowspan: SpinButton,
}

type InsertCb = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

#[derive(Clone)]
pub struct TableDialog {
    window: adw::Window,
    on_insert: InsertCb,
}

impl TableDialog {
    pub fn new(parent: &impl IsA<gtk4::Window>) -> Self {
        let window = adw::Window::builder()
            .title("Insert Table")
            .transient_for(parent)
            .modal(true)
            .default_width(640)
            .default_height(520)
            .build();

        let on_insert: InsertCb = Rc::new(RefCell::new(None));

        let header = adw::HeaderBar::new();
        header.add_css_class("fond-chrome");
        let cancel_btn = Button::with_label("Cancel");
        header.pack_start(&cancel_btn);
        let insert_btn = Button::with_label("Insert Table");
        insert_btn.add_css_class("suggested-action");
        header.pack_end(&insert_btn);

        // ── Controls: dimensions + header row ───────────────────────────────
        let controls = GtkBox::new(Orientation::Horizontal, 12);
        controls.set_margin_start(12);
        controls.set_margin_end(12);
        controls.set_margin_top(12);
        controls.set_margin_bottom(6);

        let rows_label = Label::new(Some("Rows"));
        let rows_spin = SpinButton::with_range(MIN_DIM as f64, MAX_DIM as f64, 1.0);
        rows_spin.set_value(2.0);
        let cols_label = Label::new(Some("Columns"));
        let cols_spin = SpinButton::with_range(MIN_DIM as f64, MAX_DIM as f64, 1.0);
        cols_spin.set_value(2.0);
        let header_check = CheckButton::with_label("First row is a header");
        header_check.set_active(true);

        controls.append(&rows_label);
        controls.append(&rows_spin);
        controls.append(&cols_label);
        controls.append(&cols_spin);
        controls.append(&header_check);

        // ── Per-column alignment ─────────────────────────────────────────────
        let align_row = GtkBox::new(Orientation::Horizontal, 6);
        align_row.set_margin_start(12);
        align_row.set_margin_end(12);
        align_row.set_margin_bottom(6);
        let align_label = Label::new(Some("Alignment:"));
        align_row.append(&align_label);
        let align_box = GtkBox::new(Orientation::Horizontal, 6);
        align_row.append(&align_box);

        // ── Grid of cell editors ─────────────────────────────────────────────
        let grid = Grid::new();
        grid.set_row_spacing(6);
        grid.set_column_spacing(6);
        grid.set_margin_start(12);
        grid.set_margin_end(12);
        grid.set_margin_bottom(12);

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_hexpand(true);
        scroll.set_child(Some(&grid));

        let cells: Rc<RefCell<Vec<Vec<CellWidgets>>>> = Rc::new(RefCell::new(Vec::new()));
        let aligns: Rc<RefCell<Vec<DropDown>>> = Rc::new(RefCell::new(Vec::new()));

        // Cell tooltip explains the span behavior once, rather than repeating
        // it per spin button — this is the one deliberate scope trim from a
        // full drag-to-merge interaction: a cell "covered" by an earlier
        // cell's span (per its colspan/rowspan) is skipped when generating
        // code, silently. Simpler than tracking live coverage across a
        // growing/shrinking grid, at the cost of not visually greying out
        // cells that will end up covered.
        let span_tooltip = "Cells covered by another cell's colspan/rowspan \
            are skipped when the table is generated";

        fn add_column(
            grid: &Grid,
            cells: &Rc<RefCell<Vec<Vec<CellWidgets>>>>,
            aligns: &Rc<RefCell<Vec<DropDown>>>,
            align_box: &GtkBox,
            span_tooltip: &'static str,
        ) {
            let col_idx = aligns.borrow().len() as i32;
            let align_model = StringList::new(&["Left", "Center", "Right"]);
            let align_dd = DropDown::new(Some(align_model), gtk4::Expression::NONE);
            align_box.append(&align_dd);
            aligns.borrow_mut().push(align_dd);

            let mut cells_b = cells.borrow_mut();
            for (row_idx, row) in cells_b.iter_mut().enumerate() {
                let widgets = new_cell_widgets(span_tooltip);
                attach_cell(grid, &widgets, row_idx as i32, col_idx);
                row.push(widgets);
            }
        }

        fn add_row(
            grid: &Grid,
            cells: &Rc<RefCell<Vec<Vec<CellWidgets>>>>,
            col_count: usize,
            span_tooltip: &'static str,
        ) {
            let row_idx = cells.borrow().len() as i32;
            let mut row = Vec::with_capacity(col_count);
            for col_idx in 0..col_count {
                let widgets = new_cell_widgets(span_tooltip);
                attach_cell(grid, &widgets, row_idx, col_idx as i32);
                row.push(widgets);
            }
            cells.borrow_mut().push(row);
        }

        fn remove_column(grid: &Grid, cells: &Rc<RefCell<Vec<Vec<CellWidgets>>>>, aligns: &Rc<RefCell<Vec<DropDown>>>, align_box: &GtkBox) {
            if let Some(dd) = aligns.borrow_mut().pop() {
                align_box.remove(&dd);
            }
            for row in cells.borrow_mut().iter_mut() {
                if let Some(widgets) = row.pop() {
                    grid.remove(&widgets.wrapper);
                }
            }
        }

        fn remove_row(grid: &Grid, cells: &Rc<RefCell<Vec<Vec<CellWidgets>>>>) {
            if let Some(row) = cells.borrow_mut().pop() {
                for widgets in row {
                    grid.remove(&widgets.wrapper);
                }
            }
        }

        // Seed the initial 2×2 grid.
        for _ in 0..2 {
            add_column(&grid, &cells, &aligns, &align_box, span_tooltip);
        }
        for _ in 0..2 {
            add_row(&grid, &cells, 2, span_tooltip);
        }

        {
            let grid_c = grid.clone();
            let cells_c = cells.clone();
            let aligns_c = aligns.clone();
            let align_box_c = align_box.clone();
            cols_spin.connect_value_changed(move |sp| {
                let target = sp.value() as usize;
                loop {
                    let current = aligns_c.borrow().len();
                    if current == target {
                        break;
                    } else if current < target {
                        add_column(&grid_c, &cells_c, &aligns_c, &align_box_c, span_tooltip);
                    } else {
                        remove_column(&grid_c, &cells_c, &aligns_c, &align_box_c);
                    }
                }
            });
        }
        {
            let grid_c = grid.clone();
            let cells_c = cells.clone();
            let cols_spin_c = cols_spin.clone();
            rows_spin.connect_value_changed(move |sp| {
                let target = sp.value() as usize;
                loop {
                    let current = cells_c.borrow().len();
                    if current == target {
                        break;
                    } else if current < target {
                        add_row(&grid_c, &cells_c, cols_spin_c.value() as usize, span_tooltip);
                    } else {
                        remove_row(&grid_c, &cells_c);
                    }
                }
            });
        }

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.append(&header);
        outer.append(&controls);
        outer.append(&align_row);
        outer.append(&scroll);
        window.set_content(Some(&outer));

        {
            let win = window.clone();
            cancel_btn.connect_clicked(move |_| win.close());
        }
        {
            let win = window.clone();
            let cells_c = cells.clone();
            let aligns_c = aligns.clone();
            let header_check_c = header_check.clone();
            let on_insert_c = on_insert.clone();
            insert_btn.connect_clicked(move |_| {
                let spec = read_table_spec(&cells_c, &aligns_c, header_check_c.is_active());
                let code = generate_table_code(&spec);
                if let Some(f) = on_insert_c.borrow().as_ref() {
                    f(code);
                }
                win.close();
            });
        }

        Self { window, on_insert }
    }

    pub fn set_on_insert(&self, f: impl Fn(String) + 'static) {
        *self.on_insert.borrow_mut() = Some(Box::new(f));
    }

    pub fn present(&self) {
        self.window.present();
    }
}

fn new_cell_widgets(span_tooltip: &str) -> CellWidgets {
    let content = Entry::new();
    content.set_placeholder_text(Some("Cell text"));
    content.set_hexpand(true);
    let colspan = SpinButton::with_range(1.0, MAX_SPAN as f64, 1.0);
    colspan.set_value(1.0);
    colspan.set_tooltip_text(Some(span_tooltip));
    colspan.set_width_chars(2);
    let rowspan = SpinButton::with_range(1.0, MAX_SPAN as f64, 1.0);
    rowspan.set_value(1.0);
    rowspan.set_tooltip_text(Some(span_tooltip));
    rowspan.set_width_chars(2);

    let wrapper = GtkBox::new(Orientation::Vertical, 2);
    wrapper.append(&content);
    let span_box = GtkBox::new(Orientation::Horizontal, 4);
    let colspan_lbl = Label::new(Some("cols:"));
    colspan_lbl.add_css_class("caption");
    let rowspan_lbl = Label::new(Some("rows:"));
    rowspan_lbl.add_css_class("caption");
    span_box.append(&colspan_lbl);
    span_box.append(&colspan);
    span_box.append(&rowspan_lbl);
    span_box.append(&rowspan);
    span_box.set_halign(Align::Start);
    wrapper.append(&span_box);

    CellWidgets { wrapper, content, colspan, rowspan }
}

/// `gtk_grid_remove` (called on shrink, see `remove_column`/`remove_row`
/// above) only operates on direct grid children — `widgets.wrapper` is that
/// direct child, so it must be what's attached here, not `content`/
/// `colspan`/`rowspan` individually.
fn attach_cell(grid: &Grid, widgets: &CellWidgets, row: i32, col: i32) {
    grid.attach(&widgets.wrapper, col, row, 1, 1);
}

/// One resolved cell in the dialog's current grid state.
struct TableCell {
    content: String,
    colspan: u32,
    rowspan: u32,
}

struct TableSpec {
    cells: Vec<Vec<TableCell>>,
    header_row: bool,
    aligns: Vec<&'static str>,
}

fn read_table_spec(
    cells: &Rc<RefCell<Vec<Vec<CellWidgets>>>>,
    aligns: &Rc<RefCell<Vec<DropDown>>>,
    header_row: bool,
) -> TableSpec {
    let cells = cells
        .borrow()
        .iter()
        .map(|row| {
            row.iter()
                .map(|w| TableCell {
                    content: w.content.text().to_string(),
                    colspan: w.colspan.value() as u32,
                    rowspan: w.rowspan.value() as u32,
                })
                .collect()
        })
        .collect();
    let aligns = aligns
        .borrow()
        .iter()
        .map(|dd| match dd.selected() {
            1 => "center",
            2 => "right",
            _ => "left",
        })
        .collect();
    TableSpec { cells, header_row, aligns }
}

/// Generates a `#table(...)` block from a resolved grid: computes which
/// cells are covered by an earlier cell's colspan/rowspan (and skips them),
/// wraps the first row in `table.header(...)` if `header_row` is set, and
/// emits `table.cell(colspan:, rowspan:)[...]` only for cells that actually
/// span more than one row/column, keeping the common case's output plain
/// `[content]`.
fn generate_table_code(spec: &TableSpec) -> String {
    let row_count = spec.cells.len();
    let col_count = spec.aligns.len();
    if row_count == 0 || col_count == 0 {
        return String::new();
    }

    let mut covered = vec![vec![false; col_count]; row_count];
    for (r, row) in spec.cells.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if covered[r][c] {
                continue;
            }
            let cs = cell.colspan.max(1);
            let rs = cell.rowspan.max(1);
            for dr in 0..rs {
                for dc in 0..cs {
                    let (rr, cc) = (r + dr as usize, c + dc as usize);
                    if rr < row_count && cc < col_count && (dr, dc) != (0, 0) {
                        covered[rr][cc] = true;
                    }
                }
            }
        }
    }

    let mut out = String::from("#table(\n");
    out.push_str(&format!("  columns: {col_count},\n"));
    out.push_str(&format!("  align: ({}),\n", spec.aligns.join(", ")));

    for (r, row) in spec.cells.iter().enumerate() {
        let is_header = spec.header_row && r == 0;
        let mut row_cells: Vec<String> = Vec::new();
        for (c, cell) in row.iter().enumerate() {
            if covered[r][c] {
                continue;
            }
            row_cells.push(render_cell(cell, is_header));
        }
        if row_cells.is_empty() {
            continue;
        }
        let joined = row_cells.join(", ");
        if is_header {
            out.push_str(&format!("  table.header({joined}),\n"));
        } else {
            out.push_str(&format!("  {joined},\n"));
        }
    }
    out.push_str(")\n");
    out
}

fn render_cell(cell: &TableCell, is_header: bool) -> String {
    let text = if is_header {
        format!("[*{}*]", escape_typst_content(&cell.content))
    } else {
        format!("[{}]", escape_typst_content(&cell.content))
    };
    let cs = cell.colspan.max(1);
    let rs = cell.rowspan.max(1);
    if cs <= 1 && rs <= 1 {
        text
    } else if rs <= 1 {
        format!("table.cell(colspan: {cs}){text}")
    } else if cs <= 1 {
        format!("table.cell(rowspan: {rs}){text}")
    } else {
        format!("table.cell(colspan: {cs}, rowspan: {rs}){text}")
    }
}

/// Escapes the handful of characters that would otherwise break out of a
/// Typst content block (`[...]`) if typed literally into a cell.
fn escape_typst_content(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('#', "\\#")
        .replace('@', "\\@")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(content: &str, colspan: u32, rowspan: u32) -> TableCell {
        TableCell { content: content.to_string(), colspan, rowspan }
    }

    fn plain_cell(content: &str) -> TableCell {
        cell(content, 1, 1)
    }

    #[test]
    fn simple_2x2_no_header_no_spans() {
        let spec = TableSpec {
            cells: vec![
                vec![plain_cell("A"), plain_cell("B")],
                vec![plain_cell("C"), plain_cell("D")],
            ],
            header_row: false,
            aligns: vec!["left", "left"],
        };
        let code = generate_table_code(&spec);
        assert_eq!(
            code,
            "#table(\n  columns: 2,\n  align: (left, left),\n  [A], [B],\n  [C], [D],\n)\n"
        );
    }

    #[test]
    fn header_row_wraps_in_table_header_and_bolds_content() {
        let spec = TableSpec {
            cells: vec![
                vec![plain_cell("Name"), plain_cell("Age")],
                vec![plain_cell("Alice"), plain_cell("30")],
            ],
            header_row: true,
            aligns: vec!["left", "center"],
        };
        let code = generate_table_code(&spec);
        assert!(code.contains("table.header([*Name*], [*Age*]),"));
        assert!(code.contains("[Alice], [30],"));
    }

    /// Surfaced by manual headless verification: a header row with cells
    /// left at their empty-content default (the dialog's placeholder text
    /// is not real content) produces `[**]` — two adjacent `*` markers with
    /// nothing between them, not a typo. Confirmed compiling below rather
    /// than just asserting the string shape, since "looks like empty bold"
    /// and "Typst actually parses this as empty bold, not an error" are two
    /// different claims.
    #[test]
    fn empty_header_cells_still_compile() {
        let spec = TableSpec {
            cells: vec![
                vec![plain_cell(""), plain_cell("")],
                vec![plain_cell(""), plain_cell("")],
            ],
            header_row: true,
            aligns: vec!["left", "left"],
        };
        let code = generate_table_code(&spec);
        assert!(code.contains("table.header([**], [**]),"));

        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("zerkalo_table_dialog_empty_header_{}_{n}.typ", std::process::id()));
        std::fs::write(&path, &code).unwrap();
        let result = crate::compiler::compile_to_pdf_bytes(&path, &Default::default(), &Default::default(), None);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_ok(), "empty header cell failed to compile: {:?}", result.err());
    }

    #[test]
    fn colspan_emits_table_cell_and_skips_covered_cell() {
        let spec = TableSpec {
            cells: vec![
                vec![cell("Merged", 2, 1), plain_cell("unused")],
                vec![plain_cell("X"), plain_cell("Y")],
            ],
            header_row: false,
            aligns: vec!["left", "left"],
        };
        let code = generate_table_code(&spec);
        assert!(code.contains("table.cell(colspan: 2)[Merged],\n"));
        assert!(!code.contains("unused"));
        assert!(code.contains("[X], [Y],"));
    }

    #[test]
    fn rowspan_covers_the_cell_beneath_it() {
        let spec = TableSpec {
            cells: vec![
                vec![cell("Tall", 1, 2), plain_cell("B")],
                vec![plain_cell("unused"), plain_cell("D")],
            ],
            header_row: false,
            aligns: vec!["left", "left"],
        };
        let code = generate_table_code(&spec);
        assert!(code.contains("table.cell(rowspan: 2)[Tall],"));
        assert!(!code.contains("unused"));
        assert!(code.contains("[D]"));
    }

    #[test]
    fn combined_colspan_and_rowspan() {
        let spec = TableSpec {
            cells: vec![vec![cell("Big", 2, 2), plain_cell("x")], vec![plain_cell("y"), plain_cell("z")]],
            header_row: false,
            aligns: vec!["left", "left"],
        };
        let code = generate_table_code(&spec);
        assert!(code.contains("table.cell(colspan: 2, rowspan: 2)[Big]"));
    }

    #[test]
    fn span_is_clamped_to_grid_bounds_without_panicking() {
        let spec = TableSpec {
            cells: vec![vec![cell("Overflow", 5, 5)]],
            header_row: false,
            aligns: vec!["left"],
        };
        let code = generate_table_code(&spec);
        assert!(code.contains("table.cell(colspan: 5, rowspan: 5)[Overflow]"));
    }

    #[test]
    fn empty_grid_produces_empty_string() {
        let spec = TableSpec { cells: vec![], header_row: false, aligns: vec![] };
        assert_eq!(generate_table_code(&spec), "");
    }

    #[test]
    fn escapes_brackets_and_special_characters_in_cell_content() {
        let spec = TableSpec {
            cells: vec![vec![plain_cell("a [b] #c @d")]],
            header_row: false,
            aligns: vec!["left"],
        };
        let code = generate_table_code(&spec);
        assert!(code.contains("[a \\[b\\] \\#c \\@d]"));
    }

    /// The real test: does Typst's compiler actually accept this syntax, not
    /// just "does it look plausible." Exercises a header row, a colspan cell,
    /// a rowspan cell, escaped content, and every alignment value in one
    /// document — if any of `generate_table_code`'s Typst syntax is wrong,
    /// this is what catches it, the way `template_dialog`'s own
    /// every-combination compile tests do for template generation.
    #[test]
    fn generated_code_actually_compiles_in_typst() {
        let spec = TableSpec {
            cells: vec![
                vec![plain_cell("Name"), plain_cell("Age"), plain_cell("City")],
                vec![cell("Alice [bracket]", 2, 1), plain_cell("unused")],
                vec![cell("Bob", 1, 1), plain_cell("25"), plain_cell("LA #tag @ref")],
            ],
            header_row: true,
            aligns: vec!["left", "center", "right"],
        };
        let code = generate_table_code(&spec);

        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir();
        let path = dir.join(format!("zerkalo_table_dialog_test_{}_{n}.typ", std::process::id()));
        std::fs::write(&path, &code).unwrap();

        let result = crate::compiler::compile_to_pdf_bytes(&path, &Default::default(), &Default::default(), None);
        let _ = std::fs::remove_file(&path);

        assert!(result.is_ok(), "generated table failed to compile:\n{code}\n\nerror: {:?}", result.err());
    }
}

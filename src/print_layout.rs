//! Pure print-layout logic: paper geometry, page-range resolution, imposition
//! ordering.
//!
//! Kept free of GTK and Typst types so every decision the print path makes can
//! be tested directly. `src/ui/print.rs` adapts the real document into these
//! types and adapts the results back out.

/// Typst lays out in points; portals and paper sizes speak millimetres.
const POINTS_PER_MM: f64 = 72.0 / 25.4;

/// Two page sizes count as the same paper if they agree to within this many
/// points. Typst's layout arithmetic leaves sub-point drift on pages that are
/// nominally identical, so an exact comparison reports uniform documents as
/// mixed.
const SIZE_EPSILON_PT: f64 = 0.5;

/// Paper sizes recognised by name, as portrait width × height in millimetres.
/// Only used to label the paper in the UI — nothing branches on the match.
const KNOWN_SIZES: &[(&str, f64, f64)] = &[
    ("A3", 297.0, 420.0),
    ("A4", 210.0, 297.0),
    ("A5", 148.0, 210.0),
    ("A6", 105.0, 148.0),
    ("B5", 176.0, 250.0),
    ("Letter", 215.9, 279.4),
    ("Legal", 215.9, 355.6),
    ("Tabloid", 279.4, 431.8),
    ("Executive", 184.1, 266.7),
];

/// How close a size must be to a named one to be called by that name.
const NAME_TOLERANCE_MM: f64 = 1.5;

/// The paper a document wants, derived from its laid-out pages.
///
/// Both print paths need this: the portal is told the paper up front so its
/// dialog opens on the document's real size rather than the desktop default,
/// and the GTK fallback needs it to build a `PaperSize` for the same reason.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperSpec {
    /// Width of the first page as laid out — landscape documents are wider
    /// than tall here.
    pub width_pt: f64,
    pub height_pt: f64,
    /// False when the document mixes page sizes, which Typst allows via a
    /// mid-document `#set page()`. Only the first page's size can be sent to
    /// the printer, so the caller warns instead of silently cropping the rest.
    pub uniform: bool,
}

impl PaperSpec {
    /// Derive the spec from every page's size, in layout order.
    ///
    /// Returns `None` for a document with no pages — there is nothing to print
    /// and no size to describe.
    pub fn from_page_sizes(sizes: &[(f64, f64)]) -> Option<Self> {
        let (width_pt, height_pt) = *sizes.first()?;
        let uniform = sizes.iter().all(|(w, h)| {
            (w - width_pt).abs() <= SIZE_EPSILON_PT && (h - height_pt).abs() <= SIZE_EPSILON_PT
        });
        Some(PaperSpec { width_pt, height_pt, uniform })
    }

    pub fn is_landscape(&self) -> bool {
        self.width_pt > self.height_pt
    }

    /// Paper dimensions with orientation factored out.
    ///
    /// Print systems describe paper in portrait and carry the rotation in a
    /// separate orientation field, so a landscape document must be reported as
    /// portrait paper plus landscape orientation — reporting the laid-out
    /// dimensions directly gets it silently rotated twice.
    pub fn portrait_mm(&self) -> (f64, f64) {
        let w = self.width_pt / POINTS_PER_MM;
        let h = self.height_pt / POINTS_PER_MM;
        if w > h {
            (h, w)
        } else {
            (w, h)
        }
    }

    /// Human-readable paper description for the print sheet, e.g.
    /// "A4 portrait" or "120 × 200 mm landscape".
    pub fn describe(&self) -> String {
        let (pw, ph) = self.portrait_mm();
        let orientation = if self.is_landscape() { "landscape" } else { "portrait" };
        let named = KNOWN_SIZES.iter().find(|(_, w, h)| {
            (pw - w).abs() <= NAME_TOLERANCE_MM && (ph - h).abs() <= NAME_TOLERANCE_MM
        });
        match named {
            Some((name, _, _)) => format!("{name} {orientation}"),
            None => format!("{:.0} × {:.0} mm {orientation}", pw, ph),
        }
    }
}

// ── Page ranges in the document's own numbering ──────────────────────────────

/// Maps between the numbers printed on the page and the physical page order.
///
/// Typst documents routinely disagree with their own page order: roman front
/// matter, `counter(page).update()`, appendices restarting at 1. The print
/// portal only understands physical position, so a range the user types has to
/// be translated — otherwise "print page 12" prints whichever sheet happens to
/// sit twelfth, not the one with 12 on it.
pub struct PageNumbering {
    /// Logical page number for each physical page, in layout order.
    logical: Vec<u64>,
}

impl PageNumbering {
    pub fn new(logical: Vec<u64>) -> Self {
        PageNumbering { logical }
    }

    pub fn len(&self) -> usize {
        self.logical.len()
    }

    #[allow(dead_code)] // completes the PageNumbering API alongside len()
    pub fn is_empty(&self) -> bool {
        self.logical.is_empty()
    }

    /// True when the printed numbers already match physical order, i.e. page
    /// *n* is the *n*th sheet. The print sheet says so, because when they
    /// disagree the distinction is worth surfacing and when they agree
    /// mentioning it is just noise.
    pub fn matches_physical_order(&self) -> bool {
        self.logical.iter().enumerate().all(|(i, n)| *n == i as u64 + 1)
    }

    /// Resolve a range expression written in the document's own numbering into
    /// physical page indices (0-based, ascending, deduplicated).
    ///
    /// Accepts comma-separated `N`, `A-B`, `-B` (from the start) and `A-` (to
    /// the end). An empty or all-whitespace expression means the whole
    /// document.
    ///
    /// A logical number can appear on more than one page once a counter has
    /// been reset, so every page carrying a matching number is included —
    /// silently picking the first would drop pages the user asked for.
    pub fn resolve(&self, spec: &str) -> Result<Vec<usize>, String> {
        if self.logical.is_empty() {
            return Err("The document has no pages.".into());
        }
        let spec = spec.trim();
        if spec.is_empty() {
            return Ok((0..self.logical.len()).collect());
        }

        let lowest = *self.logical.iter().min().unwrap_or(&1);
        let highest = *self.logical.iter().max().unwrap_or(&1);

        let mut wanted = Vec::new();
        for token in spec.split(',') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            let (from, to) = match token.split_once('-') {
                None => {
                    let n = parse_page_number(token)?;
                    (n, n)
                }
                Some((start, end)) => {
                    let from =
                        if start.trim().is_empty() { lowest } else { parse_page_number(start)? };
                    let to = if end.trim().is_empty() { highest } else { parse_page_number(end)? };
                    if from > to {
                        return Err(format!("“{token}” counts backwards."));
                    }
                    (from, to)
                }
            };
            for (index, number) in self.logical.iter().enumerate() {
                if *number >= from && *number <= to {
                    wanted.push(index);
                }
            }
        }

        wanted.sort_unstable();
        wanted.dedup();
        if wanted.is_empty() {
            return Err(format!("No pages are numbered {spec}."));
        }
        Ok(wanted)
    }
}

fn parse_page_number(text: &str) -> Result<u64, String> {
    text.trim()
        .parse::<u64>()
        .map_err(|_| format!("“{}” isn't a page number.", text.trim()))
}

/// Render physical indices as the 1-based, comma-separated range string print
/// systems expect (`"1-3,7"`). Consecutive runs are collapsed because CUPS
/// range lists have a length limit that a page-by-page list hits on long
/// documents.
pub fn physical_ranges_string(indices: &[usize]) -> String {
    let mut out = String::new();
    let mut iter = indices.iter().copied().peekable();
    while let Some(start) = iter.next() {
        let mut end = start;
        while iter.peek() == Some(&(end + 1)) {
            end = iter.next().unwrap_or(end);
        }
        if !out.is_empty() {
            out.push(',');
        }
        if start == end {
            out.push_str(&(start + 1).to_string());
        } else {
            out.push_str(&format!("{}-{}", start + 1, end + 1));
        }
    }
    out
}

// ── Imposition ordering ──────────────────────────────────────────────────────

/// How pages are arranged onto sheets before printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Imposition {
    /// One document page per sheet — the document goes to the printer as-is.
    #[default]
    Off,
    /// Two pages side by side on a landscape sheet, in reading order.
    TwoUp,
    /// Four pages per sheet, in reading order, left to right then down.
    FourUp,
    /// Saddle-stitch booklet: two pages per side, ordered so that printing
    /// duplex, folding the stack in half and stapling the fold yields a
    /// readable booklet.
    Booklet,
}

impl Imposition {
    /// How many document pages share one side of a sheet.
    pub fn slots_per_side(self) -> usize {
        match self {
            Imposition::Off => 1,
            Imposition::TwoUp | Imposition::Booklet => 2,
            Imposition::FourUp => 4,
        }
    }

    /// Slot grid as (columns, rows).
    pub fn grid(self) -> (usize, usize) {
        match self {
            Imposition::Off => (1, 1),
            Imposition::TwoUp | Imposition::Booklet => (2, 1),
            Imposition::FourUp => (2, 2),
        }
    }

    /// Whether the sheet is rotated relative to the document page. Two pages
    /// side by side want a landscape sheet from portrait pages; four pages in a
    /// 2×2 grid keep the page's own proportions.
    pub fn rotates_sheet(self) -> bool {
        matches!(self, Imposition::TwoUp | Imposition::Booklet)
    }

    /// The order the print sheet lists them in, and the order their config
    /// strings are indexed by.
    pub const ALL: [Imposition; 4] =
        [Imposition::Off, Imposition::TwoUp, Imposition::FourUp, Imposition::Booklet];

    pub fn config_key(self) -> &'static str {
        match self {
            Imposition::Off => "off",
            Imposition::TwoUp => "two-up",
            Imposition::FourUp => "four-up",
            Imposition::Booklet => "booklet",
        }
    }

    /// An unrecognised key falls back to one page per sheet rather than
    /// failing — a config written by a newer version must not break printing.
    pub fn from_config_key(key: &str) -> Self {
        Imposition::ALL
            .into_iter()
            .find(|imp| imp.config_key() == key)
            .unwrap_or(Imposition::Off)
    }

    pub fn label(self) -> &'static str {
        match self {
            Imposition::Off => "One page per sheet",
            Imposition::TwoUp => "Two pages per sheet",
            Imposition::FourUp => "Four pages per sheet",
            Imposition::Booklet => "Booklet (fold and staple)",
        }
    }

    /// Arrange `pages` onto sheet sides.
    ///
    /// Each inner vector is one side of one sheet; `None` is a slot left blank.
    /// Sides come back in the order they must be printed, so duplex printing
    /// pairs them correctly without further reordering.
    pub fn arrange(self, pages: &[usize]) -> Vec<Vec<Option<usize>>> {
        match self {
            Imposition::Off => pages.iter().map(|p| vec![Some(*p)]).collect(),
            Imposition::Booklet => booklet_sides(pages),
            _ => {
                let per = self.slots_per_side();
                pages
                    .chunks(per)
                    .map(|chunk| {
                        let mut side: Vec<Option<usize>> = chunk.iter().map(|p| Some(*p)).collect();
                        side.resize(per, None);
                        side
                    })
                    .collect()
            }
        }
    }
}

/// Saddle-stitch ordering.
///
/// The sheet count is rounded up to a multiple of four — a booklet is folded
/// sheets, and a folded sheet always carries four pages, so a document that
/// doesn't divide evenly gets blank slots rather than a short final sheet.
/// Blanks land at the end of the document, which is where a reader expects
/// them.
///
/// For a padded total `t`, sheet `i` shows pages `t-1-2i` and `2i` on its
/// front, and `2i+1` and `t-2-2i` on its back. Folding the stack down the
/// middle then puts them in reading order.
fn booklet_sides(pages: &[usize]) -> Vec<Vec<Option<usize>>> {
    if pages.is_empty() {
        return Vec::new();
    }
    let total = pages.len().div_ceil(4) * 4;
    let at = |slot: usize| -> Option<usize> { pages.get(slot).copied() };

    let mut sides = Vec::with_capacity(total / 2);
    for sheet in 0..total / 4 {
        let i = sheet * 2;
        sides.push(vec![at(total - 1 - i), at(i)]);
        sides.push(vec![at(i + 1), at(total - 2 - i)]);
    }
    sides
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PaperSpec ────────────────────────────────────────────────────────────

    const A4: (f64, f64) = (595.28, 841.89);
    const A5: (f64, f64) = (419.53, 595.28);

    #[test]
    fn empty_document_has_no_paper_spec() {
        assert!(PaperSpec::from_page_sizes(&[]).is_none());
    }

    #[test]
    fn uniform_document_reports_uniform() {
        let spec = PaperSpec::from_page_sizes(&[A4, A4, A4]).unwrap();
        assert!(spec.uniform);
        assert!(!spec.is_landscape());
    }

    #[test]
    fn sub_point_drift_still_counts_as_uniform() {
        // Typst's layout arithmetic leaves tiny differences between pages that
        // are nominally the same size; an exact comparison would report every
        // real document as mixed and warn on all of them.
        let spec = PaperSpec::from_page_sizes(&[A4, (595.28, 841.6), (595.0, 841.89)]).unwrap();
        assert!(spec.uniform);
    }

    #[test]
    fn mixed_page_sizes_are_detected() {
        let spec = PaperSpec::from_page_sizes(&[A4, A5]).unwrap();
        assert!(!spec.uniform, "a document mixing A4 and A5 is not uniform");
        assert_eq!(spec.width_pt, A4.0, "the first page decides the paper sent");
    }

    #[test]
    fn landscape_paper_is_reported_as_portrait_plus_orientation() {
        // Print systems carry rotation separately; handing them the laid-out
        // dimensions of a landscape page gets it rotated twice.
        let spec = PaperSpec::from_page_sizes(&[(A4.1, A4.0)]).unwrap();
        assert!(spec.is_landscape());
        let (w, h) = spec.portrait_mm();
        assert!(w < h, "portrait_mm must return the short side first");
        assert!((w - 210.0).abs() < 1.0, "got {w}");
        assert!((h - 297.0).abs() < 1.0, "got {h}");
    }

    #[test]
    fn known_sizes_are_named() {
        assert_eq!(PaperSpec::from_page_sizes(&[A4]).unwrap().describe(), "A4 portrait");
        assert_eq!(PaperSpec::from_page_sizes(&[A5]).unwrap().describe(), "A5 portrait");
        assert_eq!(
            PaperSpec::from_page_sizes(&[(A4.1, A4.0)]).unwrap().describe(),
            "A4 landscape"
        );
    }

    #[test]
    fn unknown_sizes_fall_back_to_millimetres() {
        let spec = PaperSpec::from_page_sizes(&[(283.46, 566.93)]).unwrap();
        assert_eq!(spec.describe(), "100 × 200 mm portrait");
    }

    // ── Page ranges ──────────────────────────────────────────────────────────

    fn sequential(n: u64) -> PageNumbering {
        PageNumbering::new((1..=n).collect())
    }

    #[test]
    fn empty_range_means_the_whole_document() {
        assert_eq!(sequential(3).resolve("").unwrap(), vec![0, 1, 2]);
        assert_eq!(sequential(3).resolve("   ").unwrap(), vec![0, 1, 2]);
    }

    #[test]
    fn single_pages_and_spans_resolve() {
        let n = sequential(10);
        assert_eq!(n.resolve("3").unwrap(), vec![2]);
        assert_eq!(n.resolve("2-4").unwrap(), vec![1, 2, 3]);
        assert_eq!(n.resolve("1,5-6,9").unwrap(), vec![0, 4, 5, 8]);
    }

    #[test]
    fn open_ended_spans_reach_the_ends() {
        let n = sequential(5);
        assert_eq!(n.resolve("-2").unwrap(), vec![0, 1]);
        assert_eq!(n.resolve("4-").unwrap(), vec![3, 4]);
    }

    #[test]
    fn overlapping_tokens_are_deduplicated_and_sorted() {
        assert_eq!(sequential(6).resolve("4,1-2,2-5").unwrap(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn ranges_follow_the_documents_own_numbering() {
        // Front matter numbered i–iii then a body restarting at 1: asking for
        // page 1 must give the page *printed* 1, the fourth sheet, not the
        // first.
        let n = PageNumbering::new(vec![1, 2, 3, 1, 2, 3]);
        assert!(!n.matches_physical_order());
        assert_eq!(
            n.resolve("1").unwrap(),
            vec![0, 3],
            "both pages carrying the number 1 must be included"
        );
    }

    #[test]
    fn sequential_numbering_is_recognised_as_physical() {
        assert!(sequential(4).matches_physical_order());
    }

    #[test]
    fn garbage_and_backwards_ranges_are_rejected() {
        let n = sequential(5);
        assert!(n.resolve("banana").is_err());
        assert!(n.resolve("4-2").is_err());
        assert!(n.resolve("99").is_err(), "a range matching nothing is an error, not a no-op");
    }

    #[test]
    fn ranges_on_an_empty_document_are_rejected() {
        assert!(PageNumbering::new(Vec::new()).resolve("1").is_err());
    }

    #[test]
    fn physical_ranges_collapse_runs() {
        assert_eq!(physical_ranges_string(&[0, 1, 2, 6]), "1-3,7");
        assert_eq!(physical_ranges_string(&[4]), "5");
        assert_eq!(physical_ranges_string(&[]), "");
        assert_eq!(physical_ranges_string(&[0, 2, 4]), "1,3,5");
    }

    // ── Imposition ───────────────────────────────────────────────────────────

    #[test]
    fn off_puts_one_page_on_each_sheet() {
        let sides = Imposition::Off.arrange(&[0, 1, 2]);
        assert_eq!(sides, vec![vec![Some(0)], vec![Some(1)], vec![Some(2)]]);
    }

    #[test]
    fn n_up_fills_in_reading_order_and_pads_the_last_sheet() {
        assert_eq!(
            Imposition::TwoUp.arrange(&[0, 1, 2]),
            vec![vec![Some(0), Some(1)], vec![Some(2), None]]
        );
        assert_eq!(
            Imposition::FourUp.arrange(&[0, 1, 2, 3, 4]),
            vec![
                vec![Some(0), Some(1), Some(2), Some(3)],
                vec![Some(4), None, None, None],
            ]
        );
    }

    #[test]
    fn booklet_orders_a_single_sheet() {
        // Four pages, one folded sheet: front carries 4 and 1, back 2 and 3.
        assert_eq!(
            Imposition::Booklet.arrange(&[0, 1, 2, 3]),
            vec![vec![Some(3), Some(0)], vec![Some(1), Some(2)]]
        );
    }

    #[test]
    fn booklet_pads_to_whole_sheets() {
        // Six pages need two folded sheets, i.e. eight slots — the two blanks
        // belong at the end of the document, not scattered through it.
        let sides = Imposition::Booklet.arrange(&[0, 1, 2, 3, 4, 5]);
        assert_eq!(sides.len(), 4, "two sheets, two sides each");
        assert_eq!(
            sides,
            vec![
                vec![None, Some(0)],
                vec![Some(1), None],
                vec![Some(5), Some(2)],
                vec![Some(3), Some(4)],
            ]
        );
    }

    #[test]
    fn booklet_reads_in_order_once_folded() {
        // The real invariant: gather the sheets, fold the stack, and reading
        // the leaves front to back must give the pages in order. Leaf k of the
        // folded booklet is slot k of the concatenated sides.
        for count in [4usize, 8, 12, 20] {
            let pages: Vec<usize> = (0..count).collect();
            let sides = Imposition::Booklet.arrange(&pages);
            let mut leaves: Vec<Option<usize>> = vec![None; count];
            let total_sheets = count / 4;
            for sheet in 0..total_sheets {
                let front = &sides[sheet * 2];
                let back = &sides[sheet * 2 + 1];
                // Folded, the front's right half is leaf 2i and its left half
                // the second-to-last remaining leaf; the back sits between.
                leaves[sheet * 2] = front[1];
                leaves[sheet * 2 + 1] = back[0];
                leaves[count - 2 - sheet * 2] = back[1];
                leaves[count - 1 - sheet * 2] = front[0];
            }
            let read: Vec<usize> = leaves.into_iter().flatten().collect();
            assert_eq!(read, pages, "booklet of {count} pages must fold into reading order");
        }
    }

    #[test]
    fn empty_page_list_imposes_to_nothing() {
        assert!(Imposition::Booklet.arrange(&[]).is_empty());
        assert!(Imposition::TwoUp.arrange(&[]).is_empty());
    }

    #[test]
    fn config_keys_round_trip() {
        for imp in Imposition::ALL {
            assert_eq!(Imposition::from_config_key(imp.config_key()), imp);
        }
    }

    #[test]
    fn an_unknown_config_key_falls_back_rather_than_failing() {
        assert_eq!(Imposition::from_config_key("nine-up"), Imposition::Off);
        assert_eq!(Imposition::from_config_key(""), Imposition::Off);
    }

    #[test]
    fn slots_and_grids_agree() {
        for imp in [Imposition::Off, Imposition::TwoUp, Imposition::FourUp, Imposition::Booklet] {
            let (cols, rows) = imp.grid();
            assert_eq!(cols * rows, imp.slots_per_side(), "{imp:?} grid must fill its slots");
        }
    }
}

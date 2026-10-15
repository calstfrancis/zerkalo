//! The text of the rendered preview, as positions on its pages, so it can be
//! selected and copied even though the pages are shown as pictures.
//!
//! Read straight from the compiled document's frames: each text item becomes a
//! run with the x position of every character boundary, in the order Typst
//! lays the document out, which is reading order.

use typst::layout::{Frame, FrameItem, Point, Transform};
use typst_layout::PagedDocument;

/// Where a selection end sits: a run, and a character boundary within it.
pub type Pos = (usize, usize);

pub struct Run {
    pub page: usize,
    text: Vec<char>,
    baseline: f64,
    size: f64,
    /// x of each character boundary in pt, one more than there are characters.
    edges: Vec<f64>,
}

impl Run {
    fn top(&self) -> f64 {
        self.baseline - self.size * 0.8
    }

    fn bottom(&self) -> f64 {
        self.baseline + self.size * 0.25
    }

    fn start_x(&self) -> f64 {
        self.edges[0]
    }

    fn end_x(&self) -> f64 {
        self.edges[self.text.len()]
    }
}

#[derive(Default)]
pub struct TextLayer {
    runs: Vec<Run>,
}

impl TextLayer {
    pub fn extract(doc: &PagedDocument) -> Self {
        let mut runs = Vec::new();
        for (page, p) in doc.pages().iter().enumerate() {
            collect(&p.frame, Transform::identity(), page, &mut runs);
        }
        Self { runs }
    }

    /// The closest character boundary to `(x, y)` pt on `page`: the run whose
    /// line is nearest vertically, then nearest across, so a drag that strays
    /// off the text still extends the selection to the nearest word.
    pub fn hit(&self, page: usize, x: f64, y: f64) -> Option<Pos> {
        let (idx, run) = self
            .runs
            .iter()
            .enumerate()
            .filter(|(_, r)| r.page == page && !r.text.is_empty())
            .min_by(|(_, a), (_, b)| {
                let score = |r: &Run| {
                    let dy = (r.top() - y).max(y - r.bottom()).max(0.0);
                    let dx = (r.start_x() - x).max(x - r.end_x()).max(0.0);
                    (dy, dx)
                };
                let (a, b) = (score(a), score(b));
                // Lines closer than a point apart count as the same line.
                if (a.0 - b.0).abs() < 0.5 {
                    a.1.total_cmp(&b.1)
                } else {
                    a.0.total_cmp(&b.0)
                }
            })?;
        let char_idx = run
            .edges
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (*a - x).abs().total_cmp(&(*b - x).abs()))
            .map_or(0, |(i, _)| i);
        Some((idx, char_idx))
    }

    pub fn is_over_text(&self, page: usize, x: f64, y: f64) -> bool {
        self.runs.iter().any(|r| {
            r.page == page
                && !r.text.is_empty()
                && y >= r.top()
                && y <= r.bottom()
                && x >= r.start_x()
                && x <= r.end_x()
        })
    }

    /// The whole line (every run on the baseline) around `pos`.
    pub fn line_around(&self, pos: Pos) -> (Pos, Pos) {
        let run = &self.runs[pos.0];
        let same_line =
            |r: &Run| r.page == run.page && (r.baseline - run.baseline).abs() < run.size * 0.5;
        let mut first = pos.0;
        while first > 0 && same_line(&self.runs[first - 1]) {
            first -= 1;
        }
        let mut last = pos.0;
        while last + 1 < self.runs.len() && same_line(&self.runs[last + 1]) {
            last += 1;
        }
        ((first, 0), (last, self.runs[last].text.len()))
    }

    pub fn everything(&self) -> Option<(Pos, Pos)> {
        let last = self.runs.iter().rposition(|r| !r.text.is_empty())?;
        let first = self.runs.iter().position(|r| !r.text.is_empty())?;
        Some(((first, 0), (last, self.runs[last].text.len())))
    }

    /// The text between two ends in either order, with the spaces and line
    /// breaks the page shows between runs.
    pub fn text_between(&self, a: Pos, b: Pos) -> String {
        let (from, to) = if a <= b { (a, b) } else { (b, a) };
        let mut out = String::new();
        let mut prev: Option<&Run> = None;
        for idx in from.0..=to.0.min(self.runs.len().saturating_sub(1)) {
            let run = &self.runs[idx];
            let start = if idx == from.0 { from.1 } else { 0 };
            let end = if idx == to.0 { to.1 } else { run.text.len() };
            if start >= end {
                continue;
            }
            if let Some(p) = prev {
                let sep = separator(p, run);
                if sep.starts_with('\n') {
                    out.truncate(out.trim_end_matches(' ').len());
                }
                out.push_str(sep);
            }
            out.extend(&run.text[start..end]);
            prev = Some(run);
        }
        out
    }

    /// The boxes to shade for a selection, in pt: `(page, x0, y0, x1, y1)`.
    pub fn rects(&self, a: Pos, b: Pos) -> Vec<(usize, f64, f64, f64, f64)> {
        let (from, to) = if a <= b { (a, b) } else { (b, a) };
        let mut out = Vec::new();
        for idx in from.0..=to.0.min(self.runs.len().saturating_sub(1)) {
            let run = &self.runs[idx];
            let start = if idx == from.0 { from.1 } else { 0 };
            let end = if idx == to.0 { to.1 } else { run.text.len() };
            if start < end {
                out.push((
                    run.page,
                    run.edges[start],
                    run.top(),
                    run.edges[end],
                    run.bottom(),
                ));
            }
        }
        out
    }
}

fn separator(prev: &Run, next: &Run) -> &'static str {
    if prev.page != next.page {
        return "\n\n";
    }
    let dy = next.baseline - prev.baseline;
    if dy.abs() < next.size * 0.5 {
        if next.start_x() - prev.end_x() > next.size * 0.12 {
            " "
        } else {
            ""
        }
    } else if dy > next.size * 1.7 {
        "\n\n"
    } else {
        "\n"
    }
}

fn collect(frame: &Frame, ts: Transform, page: usize, runs: &mut Vec<Run>) {
    for (pos, item) in frame.items() {
        match item {
            FrameItem::Group(group) => {
                let inner = ts
                    .pre_concat(Transform::translate(pos.x, pos.y))
                    .pre_concat(group.transform);
                collect(&group.frame, inner, page, runs);
            }
            FrameItem::Text(item) => {
                let origin = Point::new(pos.x, pos.y).transform(ts);
                let chars: Vec<char> = item.text.chars().collect();
                if chars.is_empty() {
                    continue;
                }
                let mut byte_to_char = vec![0usize; item.text.len() + 1];
                for (ci, (bi, c)) in item.text.char_indices().enumerate() {
                    for k in 0..c.len_utf8() {
                        byte_to_char[bi + k] = ci;
                    }
                }
                byte_to_char[item.text.len()] = chars.len();

                let mut starts = vec![f64::NAN; chars.len()];
                let mut ends = vec![f64::NAN; chars.len()];
                let mut x = origin.x.to_pt();
                for glyph in &item.glyphs {
                    let advance = glyph.x_advance.at(item.size).to_pt();
                    let range = glyph.range();
                    let first = byte_to_char[range.start.min(item.text.len())];
                    let last = byte_to_char[range.end.min(item.text.len())].max(first + 1);
                    let count = (last - first).min(chars.len() - first).max(1);
                    for k in 0..count {
                        starts[first + k] = x + advance * k as f64 / count as f64;
                        ends[first + k] = x + advance * (k + 1) as f64 / count as f64;
                    }
                    x += advance;
                }
                // Characters no glyph claims sit where the previous one ended.
                let mut edges = Vec::with_capacity(chars.len() + 1);
                let mut at = origin.x.to_pt();
                for i in 0..chars.len() {
                    if !starts[i].is_nan() {
                        at = starts[i];
                    }
                    edges.push(at);
                    if !ends[i].is_nan() {
                        at = ends[i];
                    }
                }
                edges.push(at);
                runs.push(Run {
                    page,
                    text: chars,
                    baseline: origin.y.to_pt(),
                    size: item.size.to_pt(),
                    edges,
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, x: f64, baseline: f64) -> Run {
        let chars: Vec<char> = text.chars().collect();
        let edges = (0..=chars.len()).map(|i| x + i as f64 * 5.0).collect();
        Run {
            page: 0,
            text: chars,
            baseline,
            size: 10.0,
            edges,
        }
    }

    fn layer() -> TextLayer {
        TextLayer {
            runs: vec![
                run("Hello ", 10.0, 20.0),
                run("world", 40.0, 20.0),
                run("Second line", 10.0, 32.0),
                run("New paragraph", 10.0, 60.0),
            ],
        }
    }

    #[test]
    fn a_drag_across_runs_copies_what_the_page_shows() {
        let l = layer();
        assert_eq!(l.text_between((0, 0), (1, 5)), "Hello world");
        assert_eq!(l.text_between((0, 0), (2, 6)), "Hello world\nSecond");
        assert_eq!(
            l.text_between((3, 3), (0, 2)),
            "llo world\nSecond line\n\nNew"
        );
    }

    #[test]
    fn a_point_resolves_to_the_nearest_character_boundary() {
        let l = layer();
        assert_eq!(l.hit(0, 21.0, 18.0), Some((0, 2)));
        // Below all text on the page: the last line, end of it.
        assert_eq!(l.hit(0, 500.0, 700.0), Some((3, 13)));
        // Right of the first line: the end of its last run, not the next line.
        assert_eq!(l.hit(0, 500.0, 19.0), Some((1, 5)));
        assert!(l.hit(1, 0.0, 0.0).is_none());
    }

    #[test]
    fn a_triple_click_takes_the_whole_line() {
        let l = layer();
        let (a, b) = l.line_around((1, 2));
        assert_eq!(l.text_between(a, b), "Hello world");
        assert_eq!(
            l.text_between(l.everything().unwrap().0, l.everything().unwrap().1)
                .lines()
                .count(),
            4
        );
    }

    #[test]
    fn only_text_itself_counts_as_over_text() {
        let l = layer();
        assert!(l.is_over_text(0, 15.0, 18.0));
        assert!(!l.is_over_text(0, 15.0, 45.0));
    }
}

# Citation & Formatting Style Audit — 2026-10-28

Audit of every citation style Zerkalo offers. It checks what Zerkalo actually
**renders**, not just what the source says. Each style was generated through the
real `generate_typst_template` with the dialog's own defaults (US Letter,
"Normal" margins, 12 pt, "1.5 Lines", bottom-centre page number). The output
was compiled with **Typst 0.15.1**, the version Zerkalo links, against a
two-entry test `.bib`, then read from PDF text and rendered pages.

Status: **fixed in 0.54.0 "True Margin" (2026-10-28)** except the items listed
under "Still open" below. The findings are kept as written, so each can be
checked against the fix. The code now lives in `src/style_spec.rs` (one
definition per style), `src/ui/template_dialog/headings.rs` (heading rules) and
`src/ui/template_dialog/generate.rs`.

## Still open after 0.54.0

- SBL's first text page should carry its number at the bottom centre and later
  pages at the top right; every style takes one page-number position.
- SBL and Turabian footnote entries are 10 pt with a full-size number and
  period, but the half-inch first-line indent of a note does not apply.
- SBL still uses the Turabian 8th edition note style: it keeps the place of
  publication but writes `12, no. 3 (2019)` for articles where SBL writes
  `12 (2019)`. The official SBL style file does not render in the citation
  engine.
- Chicago and Turabian notes and bibliography entries are not single-spaced
  with a blank line between them in the author-date styles (they are in the
  note styles).
- The quick style switcher in the header bar does not move margins, page
  numbering or the MLA header; it rewrites headings, paragraph rule, language,
  bibliography style and title. The template dialog sets everything.
- Upstream, in hayagriva's built-in styles: GOST "Сс."/"Pp.", MLA stray comma
  before a page, ASA doubled full stop, NLM/Vancouver authors missing (worked
  around by using the Taylor & Francis NLM style).

Where the code lives:
- `src/ui/template_dialog/mod.rs` — `CITATION_STYLES`, `set_style` (per-style defaults)
- `src/ui/template_dialog/generate.rs` — `generate_typst_template`, `generate_title_page`, `heading_styles`, `bib_style`, `inject_heading_numbering`, `header_block`
- `src/ui/template_dialog/sidecar.rs` — `bib_title_for_style`
- `src/ui/template_dialog/parsing.rs` — `update_page_settings_for_style`, `mandated_heading_numbering`
- `src/styles.rs` — `STYLES`, the older style-block path for non-template documents
- `src/ui/citation_panel.rs` — page-locator insertion

---

## A. Systemic bugs (affect every style)

These matter most: no style can be correct until they're fixed.

### A1. Every heading renders bold, and H1/H2 render oversized — CONFIRMED
Typst applies the heading's built-in `text(weight: "bold", size: 1.4em / 1.2em)`
*before* a custom `show heading: it => …` rule runs, so `it.body` inherits it.
Measured inside Zerkalo's own rules: H1 = bold 16.8 pt, H2 = bold 14.4 pt,
H3 = bold 12 pt.
- Every "plain"/"not bold" heading in the spec tables is actually bold: SBL
  primary and 2nd-level, Turabian H2, Chicago H2/H3, ASA H1–H3, MLA, IEEE H3.
- Every style requires headings at body-text size. SBL is explicit: "Chapter
  titles … the same type as the text font. Do not use bold styling except for
  certain levels of subheadings" (Student Supplement §2.4).
- **Fix:** start every rule body with `#set text(size: 1em, weight: "regular")`.
  Better, use `show heading: set text(size: 1em, weight: "regular")` once, then
  add bold/italic per level explicitly.

### A2. Bibliography / outline headings show a stale section number — CONFIRMED
`inject_heading_numbering` prefixes `counter(heading).display(fmt)`
unconditionally. Unnumbered headings therefore print the last section's number:
IEEE rendered **"I.A.1.1. BIBLIOGRAPHY"**, GOST **"1.1.1.1. BIBLIOGRAPHY"**, and
Vancouver **"1.1.1.1. Bibliography"**. The same happens to `#outline()`'s title.
- **Fix:** guard the prefix with `if it.numbering != none`. The comment says
  `it.numbering` isn't available in non-PDF export; re-check that against 0.15,
  because `heading.numbering` is a plain settable field. Or match on
  `it.outlined`/the bibliography's own heading.

### A3. Freshly generated documents get the wrong bibliography title — CONFIRMED
`generate_typst_template` emits `#bibliography(path, style: …)` with **no
`title:`**, so every new document says **"Bibliography"**. Examples: APA
(needs "References"), MLA ("Works Cited"), IEEE/Vancouver ("References"), and
Harvard Cite Them Right ("Reference list"). `bib_title_for_style` is applied
only on *Apply to current*, via `sidecar.rs:387`, so the title changes the
first time the user re-applies settings.
- **Fix:** pass `bib_title_for_style` in the generator too. Also give GOST
  "Список использованных источников" (GOST 7.32-2017), and decide whether
  SBL/Turabian/Chicago N-B keep "Bibliography" (correct).
- The title also needs the A1 fix: APA/MLA/Chicago want it centred, plain or
  bold per style, at body size. MLA currently gets it flush-left.

### A4. Layout defaults don't follow the chosen style at all
Choosing a style changes only the headings, the title block and the CSL file
(plus heading numbering for IEEE/GOST/Vancouver). Paper, margins, font size,
line spacing, indent, justification and page-number position stay at the
dialog's generic defaults. So the out-of-the-box document violates almost every
style:

| Setting | Dialog default | APA 7 | MLA 9 | Chicago 18 / Turabian 9 | SBL (Student Supp.) |
|---|---|---|---|---|---|
| Margins | 1" top/bottom, **1.25" sides** | 1" all | 1" all | ≥1" all | 1" all |
| Justification | **justified** | ragged right | ragged right | ragged right | "Do not justify the right margin" |
| First-line indent | **1em (≈⅙")** | 0.5" | 0.5" | 0.5" | 0.5" |
| Indent after heading | **none** (Typst default) | indent all | indent all | indent all | flush left (§2.5) ✔ |
| Gap between paragraphs | **2× leading** | none | none | none | none |
| Page number | **bottom centre** | top right | top right with surname | top right or bottom centre | p.1 bottom centre, then top right |

- **Fix:** have `FormWidgets::set_style` (which already sets numbering for
  IEEE/GOST/Vancouver) also preselect margins, spacing, indent, justification
  and page-number position. The user can still override any of them. Add a
  per-style `StyleDefaults` table in one place. Don't keep scattering `match
  style_key` arms across `generate.rs`, `parsing.rs` and `mod.rs`.
- Needs two generator changes: a `justify` flag, and
  `first-line-indent: (amount: 0.5in, all: true)` for APA/MLA/Chicago (indent
  the first paragraph after a heading). SBL keeps `all: false`.

### A5. Line-spacing presets are mislabelled — MEASURED
Typst's `leading` is the gap between lines, not the line pitch. Measured
baseline-to-baseline pitch at 12 pt (Liberation Serif, metric-identical to
Times New Roman):

| Preset | leading | pitch | What it really is |
|---|---|---|---|
| Single | 0.65em | 15.7 pt | ≈ Word "single" (13.8 pt) — fine |
| "1.5 Lines" (dialog default) | 1.32em | 23.7 pt | ≈ **true double** (typewriter 24 pt) |
| "Double" | 2em | 31.9 pt | ≈ **2.3×** — ~15% looser than Word's double (27.6 pt) |
| `styles.rs` "double-spaced" styles | 1em | 19.9 pt | ≈ **1.5 spacing**, not double |

- **Fix:** relabel or retune. Matching Word, which is what instructors compare
  against: 1.5 ≈ `1.07em`, Double ≈ `1.65em` at 12 pt. GOST "полуторный" at
  14 pt ≈ `1.1em` (currently 0.85em = 21.1 pt, should be ≈24.2 pt). Keep
  `LEGACY_SPACING` mappings so old documents round-trip.
- Paragraph `spacing` should equal `leading` for every first-line-indent style
  (A4). `paragraph_spacing_for_leading` doubles it, which leaves a visible
  blank half-line between paragraphs that APA/MLA/Chicago don't allow. Its
  comment says equal spacing made breaks invisible. It does without an indent,
  but with a correct 0.5" indent it's exactly what the manuals specify.

### A6. Heading blocks crowd or overlap the text — CONFIRMED visually
Fixed `above: 1em / below: 0.2em–0em` ignore the document's line spacing. In a
double-spaced document headings sit tighter than body lines. IEEE/ASA/Vancouver
H3 (`below: 0em`) and SBL/GOST H3–H4 visibly collide with the next line.
- **Fix:** derive spacing from `par.leading` (e.g.
  `above: par.leading * 2, below: par.leading`). For the "run-in" levels (APA
  L4/L5, ASA L3, IEEE L3), make them genuinely run-in: render the heading
  inline at the start of the paragraph, not as a block with zero spacing.

### A7. Page-locator text is style-blind — CONFIRMED
The citation panel always inserts `@key[p. 12]`. Typst passes the supplement
through literally, so:
- Chicago notes: `…(Example Press, 2020), p. 12.` should be `…, 12.`
- Chicago A-D: `(Smith and Lee 2020, p. 14)` should be `(Smith and Lee 2020, 14)`
- ASA: `(Doe, Roe, and Poe 2019:p. 50)` should be `(… 2019:50)`
- MLA: `(Smith and Lee, p. 14)` should be `(Smith and Lee 14)`
- APA, Harvard and IEEE are correct *with* "p.".
- **Fix:** take the bare locator from the field and prefix "p."/"pp." only for
  APA, Harvard and IEEE. Also turn a hyphen in ranges into an en dash
  (`12-14` → `12–14`).
- **Upstream (can't fix in Zerkalo):** even a bare `[14]` renders MLA as
  `(Smith and Lee, 14)` with a stray comma. This happens with Typst's built-in
  MLA *and* the current upstream MLA CSL. Worth a hayagriva issue.

### A8. Header option wipes out a top page number
`header_block` sets `page(header: …)`. In Typst a custom header replaces the
automatic top-placed page number, so "Top right" page number plus any header
silently loses the number. APA needs a running head plus a number, and MLA
needs "Surname 1". Needs a combined header that places both.

---

## B. Per-style findings

Rows marked **✔** match the spec. Built-in CSL editions were read from the
`.cbor` metadata in hayagriva 0.10.1.

### SBL — `sbl` → CSL `chicago-notes` (CMOS 18)
Source checked: *Student Supplement for The SBL Handbook of Style, 2nd ed.*
(sbl-site.org, read directly).
- Heading hierarchy ✔: caps centred / bold centred / centred / left bold-italic
  / left plain, matching §2.6 exactly. It only renders wrong because of A1:
  primary and 2nd-level come out bold and enlarged.
- Code comments cite three different sections for the same rules: `styles.rs`
  "§4.1", `generate.rs` "§2.4/§2.6", and the supplement's actual §2.6. Point
  both at Student Supplement §2.6.
- Title page should be **all caps**, centred, 2" top/bottom, ~2" between
  elements (§2.8). Zerkalo uses the generic title page: mixed case, bold 16 pt
  title, italic affiliation.
- Page numbers: p.1 of the text bottom centre, then top right; no number on the
  title page (§2.3). Currently all bottom centre.
- Footnotes: 10 pt acceptable, single-spaced, first line indented 0.5", **full-
  size number followed by a period**, blank line between notes (§2.9). Typst
  defaults give a superscript marker and no indent. Add a `show footnote.entry`
  rule.
- Bibliography: 2" top margin, single-spaced entries with a blank line between
  (§2.11). Currently double-spaced.
- Block quotes: single-spaced, 0.5" indent (§2.2) — `#quote(block: true)` not styled.
- **CSL mismatch:** SBL requires place of publication ("Louisville: Westminster
  John Knox, 1992"). CMOS 18 dropped it, so SBL papers render "(Example Press,
  2020)". The official `society-of-biblical-literature-fullnote-bibliography.csl`
  (SBLHS 2nd ed., updated 2026-08-12) **does not render correctly in hayagriva
  0.10.1** — I tested it and got stray `;`/`,` punctuation. It can't be bundled
  as-is. Options: bundle a trimmed SBL CSL fixed to hayagriva's subset, use
  `turabian-fullnote-8` (keeps place, but it's the 8th edition), or keep
  CMOS 18 and document the gap.

### Chicago Notes-Bibliography — CSL `chicago-notes` = CMOS 18 ✔ (current edition)
- First note full, subsequent notes shortened ✔. Bibliography ✔ apart from A3/A7.
- Headings: CMOS doesn't prescribe a hierarchy; the Turabian-like set is fine once A1 is fixed.

### Chicago Author-Date — CSL `chicago-author-date` = CMOS 18 ✔
- Needs the A7 fix (no "p."). Title "References" ✔ once A3 is fixed.

### Turabian — CSL `chicago-notes` (CMOS 18)
- Turabian **9th ed. (2018) is still the current edition** and tracks CMOS 17.
  I found no announced 10th edition. CMOS 18 output is a defensible choice, but
  it drops place of publication, which Turabian 9 still requires. Same
  trade-off as SBL.
- Headings ✔ vs Turabian A.2.2.4 (centred bold / centred plain / left italic),
  apart from A1.
- Turabian specifics not implemented: notes and bibliography entries
  single-spaced with a blank line between, full-size note numbers in the
  notes, 1" margins, page numbers top right or bottom centre.

### MLA — CSL `mla` = MLA Handbook 9th ✔
- Heading block flush left, double-spaced ✔.
- **Title is not centred** (rendered flush left) — `#align(center)` inside a
  `#block[...]` that holds `set par(first-line-indent: 0pt)`; confirmed on the
  rendered page. Needs investigating.
- Date should be day-month-year ("28 October 2026"); the generator uses `%B %-d, %Y`.
- Header `Surname 1` top right, 0.5" from top, on every page — missing (see A8).
- "Works Cited" centred, plain — currently "Bibliography", flush left (A3, A1).
- No title page ✔. Abstract/keywords are offered — fine if the user opts in.

### APA 7th — CSL `apa` = APA 7 ✔
- Title page: the title must be **12 pt** bold (currently 14 pt), 3–4 lines
  down. There's also a page number top right. The student version has no
  running head; the professional version has a shortened title ≤50 characters
  in caps, flush left, **plus** the page number. Zerkalo always adds the
  running head at 10 pt and no number. Pick one: student by default.
- Abstract on its own page; the body starts on a new page with the full title
  repeated, centred and bold — currently the abstract and body share a page.
  The placeholder `= Introduction` contradicts APA (the intro isn't labelled).
- Abstract: no side inset (currently 8%), "Abstract" bold centred ✔,
  "_Keywords:_" indented 0.5" ✔(italic).
- Headings L1–L3 ✔ apart from A1. **L4/L5 have no rule.** They should be
  indented, run-in, ending with a period: bold for L4, bold italic for L5.
- References: "References" bold centred, hanging 0.5" ✔ (CSL), double-spaced.
- Fonts: APA accepts TNR 12, Georgia 11, Calibri 11, Arial 11, Lucida Sans
  Unicode 10 and Computer Modern 10. Zerkalo's default font list is fine.

### ASA — **CSL `apa` — wrong style**
- `bib_style("asa")` returns `"apa"`, so ASA papers get APA citations:
  `(Smith & Lee, 2020, p. 14)` instead of `(Smith and Lee 2020:14)`. Typst
  ships `american-sociological-association` (ASA 6th/7th). Switch to it. It
  has a cosmetic upstream bug, a doubled period after page ranges (`45–67..`).
- Headings: H1 caps flush left, H2 italic flush left, H3 indented italic run-in
  with a period ✔ in intent. They render bold (A1), and H3 isn't actually
  run-in (A6). I couldn't verify the caps rule against the 7th-edition manual
  itself; secondary guides don't quote it.
- Margins: sources disagree (some 1", some 1.25"). Keep 1" and leave it user-adjustable.

### Harvard — **CSL `chicago-author-date` — wrong style**
- "Harvard" in practice means *Cite Them Right*. Typst ships
  `harvard-cite-them-right`, and upstream is now on the 12th edition. Current
  output `(Smith and Lee 2020, p. 14)` / `2020. Title.` is Chicago, not
  Harvard. CTR gives `(Smith and Lee, 2020, p. 14)` / `(2020) Title.`
  List title: "Reference list".
- Uses APA headings and the APA title page. Harvard specifies no layout, so
  that's acceptable, but the APA running head on the title page is out of
  place for UK coursework.

### IEEE — CSL `ieee` = 2023 guidelines ✔
Per the IEEE conference template: US Letter, 10 pt TNR, two columns with 0.25"
gap, margins 0.75" top / 1" bottom / 0.625" sides, single-spaced.
- The dialog doesn't apply any of it except `columns: 2`. A fresh IEEE
  document is **12 pt, "1.5 Lines" (really double), 1.25" side margins**. The
  older `styles.rs` entry does set 10 pt/0.75".
- **Title block sits inside the left column** — the title wraps into four
  justified, letter-spaced lines. It needs
  `place(top, scope: "parent", float: true)[…]`. The title should be 24 pt,
  not bold and not all caps (currently 18 pt bold caps).
- Numbering `"I.A.1."` makes Typst print **"I.A." / "I.A.1."** for sub-levels.
  IEEE shows only the own-level label: `I.`, `A.`, `1)`. Needs a numbering
  function, e.g. `(..n) => { let n = n.pos(); (if n.len() == 1 { numbering("I.", ..n) } else if n.len() == 2 { numbering("A.", n.last()) } else { numbering("1)", n.last()) }) }`.
- H1 is small caps centred in the template (not bold uppercase). H2 is italic
  (not bold italic). H3 is italic, run-in, indented.
- Abstract should be bold "Abstract—" run-in at 9 pt; keywords "Index Terms—".
- No page numbers on conference papers. "References" unnumbered (A2).

### Vancouver — **CSL `vancouver` is broken upstream: author names are missing**
- Typst 0.15.1's `vancouver` (= `nlm-citation-sequence`, Citing Medicine 2nd
  ed.) renders **no authors at all**:
  `1. A Fictional Study of Examples. Springfield: Example Press; 2020.`
  It also prints `2019 ;12(3)`. Reproduced with Typst's built-in name *and*
  the current upstream `nlm-citation-sequence.csl`, so it's a hayagriva bug.
  **Every Vancouver bibliography Zerkalo produces today is unusable.**
- Interim fix: `springer-vancouver` renders `Smith JA, Lee M. …; 2020.` and
  `Doe C, Roe R, Poe A. … 2019;12:45–67.` correctly, but omits the issue
  number. File upstream.
- ICMJE asks for **double spacing** and 1" margins. It doesn't number IMRaD
  section headings. Zerkalo forces `1.` numbering on Vancouver; drop that
  mandate (`set_style`, `mandated_heading_numbering`).
- The `styles.rs` entry is single-spaced (0.65em) — wrong for ICMJE.

### GOST — CSL `gost-r-705-2008-numeric` (GOST R 7.0.5-2008, CSL dated 2010)
Layout should follow GOST 7.32-2017: A4; margins L30/R15/T20/B20 mm ✔;
≥12 pt (14 pt common) ✔; 1.5 spacing; indent 1.25 cm ✔; page numbers
bottom centre without a dot ✔; title page counted but not numbered ✔.
- Document `lang` is `"en"`, so the reference list title is "Bibliography" and
  the terms are English. Setting `lang: "ru"` gives "Библиография" but also
  `Сс. 45–67` (should be `С. 45–67`, upstream locale bug). For English sources
  GOST wants `P. 45–67`, while hayagriva gives `Pp.`. List title should be
  "СПИСОК ИСПОЛЬЗОВАННЫХ ИСТОЧНИКОВ".
- Heading numbers carry a trailing dot (`1.`, `1.1.`). 7.32-2017 numbers
  sections **without a dot**: `1`, `1.1`.
- Numbered section headings should start at the paragraph indent, bold,
  sentence case, no trailing period. Only structural elements (ВВЕДЕНИЕ,
  ЗАКЛЮЧЕНИЕ, СОДЕРЖАНИЕ, СПИСОК…) are centred caps. Zerkalo centres and
  capitalises every H1.
- 1.5 spacing at 14 pt ≈ `leading: 1.1em` (A5). The dialog default "1.5 Lines"
  is really double.
- Naming: the standard is "GOST R **7.0.5**-2008", not "7.0-5". Comments mix
  it up with 7.32 (the layout standard). Label the style "GOST (7.32 / R 7.0.5)".

### LaTeX Look
Not a citation standard. `bib_style` falls through to `"apa"`, which is
surprising for a "LaTeX" look — LaTeX's default is numeric (`ieee` or
`alphanumeric` would be closer). Low priority.

---

## C. Two style tables drifting apart

`src/styles.rs::STYLES`, the style-block path for non-template documents,
duplicates every style's layout and headings separately from
`template_dialog/generate.rs`. The two already disagree. The dialog's ASA and
Harvard match `styles.rs`, but the older path has IEEE at 10 pt/0.75" while the
dialog has neither. Vancouver is single-spaced in one and unset in the other.
Fixing styles in one place won't fix the other. Collapse both onto one
per-style definition — headings, layout defaults, CSL, bibliography title and
locator prefix — consumed by both paths.

---

## Proposed fix order

1. **Correctness of output (no UI change):** A1 heading weight/size, A2 stale
   numbers, A3 bibliography titles, ASA → `american-sociological-association`,
   Harvard → `harvard-cite-them-right`, Vancouver → `springer-vancouver`
   (interim), IEEE numbering function, MLA title centring, GOST no-dot
   numbering + `lang: "ru"`.
2. **Per-style defaults table (C + A4):** one `StyleDefaults` source of truth,
   applied by `set_style` and the older path; fix the spacing presets (A5) with
   legacy mappings; paragraph spacing = leading; `first-line-indent all`.
3. **Headings & title pages:** spacing from leading (A6), APA L4/L5 run-in, ASA
   and IEEE run-in L3, SBL all-caps title page, APA student title page + body
   restart, IEEE full-width title, GOST structural vs numbered headings.
4. **Notes & references layout:** SBL/Turabian footnote entry style,
   single-spaced bibliography with blank lines (SBL/Turabian), header + page
   number coexistence (A8, MLA "Surname 1", APA).
5. **Citation panel locator (A7)** and en-dash ranges.
6. **Upstream reports (hayagriva):** NLM/Vancouver missing authors; MLA locator
   comma; GOST `Сс.`/`Pp.` plurals; ASA doubled period; SBL CSL punctuation.
   Recheck on each `hayagriva` bump.

Each phase should add a compile test per style asserting rendered text, like
the existing `compile` closure in `template_dialog/mod.rs`'s tests. For
example: the Vancouver bibliography contains "Smith JA"; MLA's contains
"Works Cited"; no unnumbered heading starts with a digit.

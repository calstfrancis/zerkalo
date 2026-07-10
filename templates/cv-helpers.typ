// cv-helpers.typ — renders Skrizhal CV elements in Zerkalo's CV mode.
//
// Made available automatically (no on-disk copy needed) whenever a
// document's cv_elements_path resolves: Zerkalo injects this file's content
// at the virtual path "cv-helpers.typ" next to the document, and the actual
// CV data as a YAML string via `sys.inputs.at("skrizhal-cv-data")` — see
// `effective_cv_elements` in src/ui/app_window.rs. Bring the functions below
// into scope with:
//
//   #import "cv-helpers.typ": cv-entry, cv-section
//
// Categories are canonical Title Case strings ("Ministry Position", not
// "ministry-position") — see skrizhal-core's registry.
//
// Every renderer below takes a `style` string — "modern" | "academic" |
// "classic" | "sidebar" — matching the CV_STYLE constant that "New from
// Template" writes into the document, so `#cv-section(..., style: CV_STYLE)`
// picks up the same per-style formatting the old hand-written #job/#edu/
// #award/#presentation functions used to.

#let cv-data = {
  let raw = sys.inputs.at("skrizhal-cv-data", default: "")
  if raw == "" { (:) } else { yaml(bytes(raw)) }
}

// "2020" -> "2020"; "2020-01/2022-12" -> "2020-01 – 2022-12";
// "2023/" (ongoing) -> "2023 – Present". Matches Skrizhal's stored format
// exactly (a single value, a closed range, or an open-ended range).
#let cv-date-text(date) = {
  if date == none or date == "" { return none }
  // An unquoted single-year date (`date: 2020`) parses as a Typst integer,
  // not a string — unlike skrizhal-core's Rust YAML parsing, which
  // round-trips through text specifically to coerce this leniently (see
  // skrizhal-core's parse_str). Coerce here too so `.split` doesn't fail.
  let date = str(date)
  let parts = date.split("/")
  if parts.len() == 1 {
    parts.at(0)
  } else if parts.at(1) == "" {
    parts.at(0) + " – Present"
  } else {
    parts.at(0) + " – " + parts.at(1)
  }
}

// Accent/muted/dim colors per style — "modern" gets a blue accent, the
// others stay black/gray-scale. Mirrors the CV_STYLE-derived color lets
// "New from Template" used to write directly into the document.
#let cv-palette(style) = (
  accent: if style == "modern" { rgb("#2a5298") } else { black },
  muted: if style == "modern" { rgb("#555555") } else { luma(90) },
  dim: if style == "modern" { rgb("#888888") } else { luma(130) },
)

// Which visual shape a category renders as. Unrecognized categories fall
// back to the generic "job" shape (title / organization / date / bullets),
// which fits most freeform categories fine.
//
// Case-insensitive: Skrizhal's category field is free-text (with a
// suggestion popover, not a hard enum), and skrizhal-core's own registry
// lookup is deliberately case-insensitive so a hand-typed "education"
// still resolves — this needs to match that leniency, or a
// differently-cased category silently renders with the wrong shape.
#let cv-shape-for-category(category) = {
  let c = lower(category)
  if c == "education" { "edu" }
  else if c in ("award", "certification") { "award" }
  else if c in ("publication", "presentation") { "presentation" }
  else if c == "language skill" { "tag" }
  else { "job" }
}

// Bullet list from a Skrizhal `description` field, or `none` if empty —
// shared by every shape below so each just asks for "the description block".
#let cv-desc-block(entry) = {
  let items = entry.at("description", default: ())
  if items.len() == 0 { none } else { list(..items.map(d => [#d])) }
}

// title / organization / date / bullets — Employment, Ministry Position,
// Volunteer, Project, Service, Committee Appointment, and anything unmapped.
#let cv-render-job(style, p, title, org, years, desc) = {
  if style == "modern" {
    grid(columns: (1fr, auto),
      [*#title*#if org != none [ #h(0.3em)#text(fill: p.accent, size: 9.5pt)[#org]]],
      text(size: 9pt, fill: p.dim, style: "italic")[#years],
    )
  } else if style == "academic" {
    grid(columns: (1fr, auto),
      [*#title*#if org != none [ #h(0.3em)#text(style: "italic")[#org]]],
      text(style: "italic", fill: p.muted)[#years],
    )
  } else if style == "sidebar" {
    if org != none { [*#title* --- #org] } else { [*#title*] }
    linebreak()
    text(style: "italic")[#years]
  } else {
    grid(columns: (1fr, auto),
      [*#title*#if org != none [ #h(0.25em)#text(fill: p.muted)[—]#h(0.25em)#org]],
      text(fill: p.muted, style: "italic")[#years],
    )
  }
  v(0.2em)
  desc
  v(0.5em)
}

// degree / institution / date / optional note — Education.
#let cv-render-edu(style, p, degree, institution, years, note) = {
  if style == "modern" {
    grid(columns: (1fr, auto),
      [*#degree*#if institution != none [ #h(0.3em)#text(fill: p.accent, size: 9.5pt)[#institution]]],
      text(size: 9pt, fill: p.dim, style: "italic")[#years],
    )
  } else if style == "academic" {
    grid(columns: (1fr, auto),
      [*#degree*#if institution != none [ #h(0.3em)#text(style: "italic")[#institution]]],
      text(style: "italic", fill: p.muted)[#years],
    )
  } else if style == "sidebar" {
    // Title / Organization / Date, each its own line — an optional note
    // (from the entry's description) comes after, so it never disrupts
    // that three-line rhythm.
    [*#degree*]
    linebreak()
    if institution != none { [#institution]; linebreak() }
    [#years]
  } else {
    grid(columns: (1fr, auto),
      [*#degree*#if institution != none [ #h(0.25em)#text(fill: p.muted)[—]#h(0.25em)#institution]],
      text(fill: p.muted, style: "italic")[#years],
    )
  }
  if note != none { v(0.15em); note }
  v(0.45em)
}

// title / awarding org / date / optional description — Award, Certification.
#let cv-render-award(style, p, title, org, years, desc) = {
  if style == "modern" {
    grid(columns: (1fr, auto),
      [*#title*#if org != none [ #h(0.3em)#text(fill: p.accent, size: 9.5pt)[#org]]],
      text(size: 9pt, fill: p.dim, style: "italic")[#years],
    )
  } else if style == "academic" {
    grid(columns: (1fr, auto),
      [*#title*#if org != none [ #h(0.3em)#text(style: "italic")[#org]]],
      text(style: "italic", fill: p.muted)[#years],
    )
  } else if style == "sidebar" {
    // Title / Organization / Date, each its own line — matches the layout
    // used for education entries.
    [*#title*]
    linebreak()
    if org != none { [#org]; linebreak() }
    [#years]
  } else {
    grid(columns: (1fr, auto),
      [*#title*#if org != none [ #h(0.25em)#text(fill: p.muted)[—]#h(0.25em)#org]],
      text(fill: p.muted, style: "italic")[#years],
    )
  }
  if desc != none { v(0.15em); desc }
  v(0.45em)
}

// role / venue / title / date — Publication, Presentation. Role comes from
// an `extra` field (e.g. `role: Panelist` in the YAML) since it isn't one of
// Skrizhal's modeled fields; defaults to "Presenter".
#let cv-render-presentation(style, p, role, venue, title, years) = {
  if style == "sidebar" {
    [*#role* #h(0.25em)#venue, #text(style: "italic")["#title"]]
    linebreak()
    text(style: "italic")[#years]
  } else {
    grid(columns: (1fr, auto),
      [*#role* #h(0.25em)#venue, #text(style: "italic")["#title"]],
      text(fill: p.muted, style: "italic")[#years],
    )
  }
  v(0.35em)
}

// Renders one CV entry (a raw Skrizhal dict, as found in `cv-data`),
// dispatching on its category to the shape renderers above.
#let cv-entry-render(entry, style: "modern") = {
  let p = cv-palette(style)
  let category = entry.at("category", default: "")
  let shape = cv-shape-for-category(category)
  let title = entry.at("title", default: "")
  let org = entry.at("organization", default: none)
  let years = cv-date-text(entry.at("date", default: none))
  let desc = cv-desc-block(entry)

  if shape == "edu" {
    cv-render-edu(style, p, title, org, years, desc)
  } else if shape == "award" {
    cv-render-award(style, p, title, org, years, desc)
  } else if shape == "presentation" {
    let role = entry.at("role", default: "Presenter")
    let venue = if org == none { "" } else { org }
    cv-render-presentation(style, p, role, venue, title, years)
  } else {
    cv-render-job(style, p, title, org, years, desc)
  }
}

// Renders a single entry by key, dispatching on category. Unknown keys show
// a visible red error in the compiled document rather than failing silently
// (a renamed or deleted Skrizhal entry should be obvious at a glance).
#let cv-entry(key, data: cv-data, style: "modern") = {
  if key not in data {
    return text(fill: red)[Unknown CV entry: #key]
  }
  cv-entry-render(data.at(key), style: style)
}

// Comma/dot-joined line of entry titles (sidebar style: real bullets) — for
// tag-shaped categories like Language Skill, where each entry is a single
// short line rather than a dated card.
#let cv-tags-line(style, items) = {
  if items.len() == 0 {
    text(style: "italic", fill: luma(150))[No entries yet.]
  } else if style == "sidebar" {
    list(..items.map(item => [#item]))
  } else {
    text(fill: luma(110))[#items.join("  ·  ")]
    v(0.15em)
  }
}

// Renders every entry matching `category` and/or `tag` (both optional —
// omitting both renders everything), most-recent-first by date. `category`
// accepts either a single category name or an array of them (e.g. combining
// Employment and Ministry Position into one "Experience" section, sorted
// together rather than in separate blocks). `mode: "tags"` renders a single
// comma/bullet line of titles instead of full entry cards — for tag-shaped
// categories like Language Skill.
#let cv-section(category: none, tag: none, data: cv-data, style: "modern", mode: "entries") = {
  // Lower-cased once so an entry's differently-cased category (see
  // cv-shape-for-category) doesn't get silently excluded from the section
  // it actually belongs in.
  let categories = if category == none { none }
    else if type(category) == array { category.map(lower) }
    else { (lower(category),) }
  let keys = data.keys().filter(k => {
    let entry = data.at(k)
    let entry-category = entry.at("category", default: none)
    let category-ok = categories == none or (entry-category != none and lower(entry-category) in categories)
    let entry-tags = entry.at("tags", default: ())
    let tag-ok = tag == none or tag in entry-tags
    category-ok and tag-ok
  })
  let sorted = keys.sorted(key: k => {
    let d = data.at(k).at("date", default: "")
    let end = d.split("/").last()
    if end == "" { "9999" } else { end }
  }).rev()

  if mode == "tags" {
    cv-tags-line(style, sorted.map(k => data.at(k).at("title", default: k)))
  } else if sorted.len() == 0 {
    text(style: "italic", fill: luma(150))[No entries yet.]
  } else {
    for k in sorted {
      cv-entry-render(data.at(k), style: style)
    }
  }
}

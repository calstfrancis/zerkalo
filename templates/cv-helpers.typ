// cv-helpers.typ — renders Skrizhal CV elements in Zerkalo's CV mode.
//
// Made available automatically (no on-disk copy needed) whenever a
// document's cv_elements_path resolves: Zerkalo injects this file's content
// at the virtual path "cv-helpers.typ" next to the document, and the actual
// CV data as a YAML string via `sys.inputs.at("skrizhal-cv-data")` — see
// `effective_cv_elements` in src/ui/app_window.rs. Bring the two functions
// below into scope with:
//
//   #import "cv-helpers.typ": cv-entry, cv-section
//
// Categories are canonical Title Case strings ("Ministry Position", not
// "ministry-position") — see skrizhal-core's registry.

#let cv-data = {
  let raw = sys.inputs.at("skrizhal-cv-data", default: "")
  if raw == "" { (:) } else { yaml(bytes(raw)) }
}

// "2020" -> "2020"; "2020-01/2022-12" -> "2020-01 – 2022-12";
// "2023/" (ongoing) -> "2023 – Present". Matches Skrizhal's stored format
// exactly (a single value, a closed range, or an open-ended range).
#let cv-date-text(date) = {
  if date == none or date == "" { return none }
  let parts = date.split("/")
  if parts.len() == 1 {
    parts.at(0)
  } else if parts.at(1) == "" {
    parts.at(0) + " – Present"
  } else {
    parts.at(0) + " – " + parts.at(1)
  }
}

// Shared rendering for any category without its own block below: bold
// title, dim organization/location/date line, bullet description. This is
// the seam for category-specific layouts later (e.g. publications wanting
// a citation-style line) without touching cv-entry's call sites.
#let cv-entry-default(entry) = {
  block(above: 0.6em, below: 0.6em)[
    #text(weight: "bold")[#entry.at("title", default: "")]
    #if entry.at("organization", default: none) != none [ --- #entry.organization]
    #linebreak()
    #text(size: 0.9em, fill: gray)[
      #if entry.at("location", default: none) != none [#entry.location #h(1em)]
      #cv-date-text(entry.at("date", default: none))
    ]
    #{
      let desc = entry.at("description", default: none)
      if desc != none {
        let items = if type(desc) == array { desc } else { (desc,) }
        list(..items)
      }
    }
  ]
}

// Renders a single entry by citation key, dispatching on category.
#let cv-entry(key, data: cv-data) = {
  if key not in data {
    return text(fill: red)[Unknown CV entry: #key]
  }
  cv-entry-default(data.at(key))
}

// Renders every entry matching `category` and/or `tag` (both optional —
// omitting both renders everything), most-recent-first by date.
#let cv-section(category: none, tag: none, data: cv-data) = {
  let keys = data.keys().filter(k => {
    let entry = data.at(k)
    let category-ok = category == none or entry.at("category", default: none) == category
    let entry-tags = entry.at("tags", default: ())
    let tag-ok = tag == none or tag in entry-tags
    category-ok and tag-ok
  })
  let sorted = keys.sorted(key: k => {
    let d = data.at(k).at("date", default: "")
    let end = d.split("/").last()
    if end == "" { "9999" } else { end }
  })
  for k in sorted.rev() {
    cv-entry(k, data: data)
  }
}

// Verbatim helpers — a written-up conversation set as a numbered script.
// Part of the template block above; Zerkalo rewrites this when you update the
// template settings, so write your own code below the block, not in it.
#let verbatim-meta = state("verbatim-meta", (:))
#let verbatim-columns = state("verbatim-columns", true)
#let verbatim-line = counter("verbatim-line")
#let verbatim-muted = luma(95)

#set page(header: context {
  let m = verbatim-meta.get()
  if counter(page).get().first() > 1 and m != (:) {
    set text(size: 9pt, fill: verbatim-muted)
    [#m.at("no", default: "") · #m.at("author", default: "") #h(1fr) #m.at("confidential", default: "")]
  }
})

#show heading.where(level: 1): it => block(above: 1.4em, below: 0.7em)[
  #set text(size: 14pt, weight: "bold", style: "normal")
  #align(left, it.body)
  #v(-0.5em)
  #line(length: 100%, stroke: 0.5pt + luma(150))
]
#show heading.where(level: 2): it => block(above: 1.1em, below: 0.5em)[
  #set text(size: 11.5pt, weight: "bold", style: "italic")
  #align(left, it.body)
]

#let verbatim-box(content) = block(
  inset: (x: 0.9em, y: 0.7em), radius: 3pt, width: 100%,
  stroke: 0.5pt + luma(180), fill: luma(248), content)

// One line of the conversation. `who` is the speaker's label ("P", "R", "Dtr"),
// any short text; the same label always means the same person. `feel` is what
// you felt or noticed then; `note` is analysis you added afterwards.
#let say(who, body, feel: none, note: none) = {
  verbatim-line.step()
  context {
    let margin(content, label) = if content == none { [] } else {
      text(size: 9.5pt, fill: verbatim-muted)[#text(weight: "bold")[#label] #content]
    }
    let number = text(size: 9pt, fill: verbatim-muted, verbatim-line.display())
    let name = text(weight: "bold", smallcaps(who))
    if verbatim-columns.get() {
      block(breakable: false, above: 0.9em, below: 0.9em, grid(
        columns: (1.6em, 3.4em, 1.6fr, 0.7fr, 0.7fr), column-gutter: 0.6em,
        number, name, body, margin(feel, "Felt"), margin(note, "Note")))
    } else {
      block(breakable: false, above: 0.7em, below: 0.7em, grid(
        columns: (1.6em, 3.4em, 1fr), column-gutter: 0.6em,
        number, name,
        [#body#if feel != none { footnote[Felt: #feel] }#if note != none { footnote[Note: #note] }]))
    }
  }
}

// A pause, silence or nonverbal moment, set as a stage direction.
#let stage(body) = block(above: 0.5em, below: 0.5em, inset: (left: 6.2em))[
  #text(style: "italic", fill: verbatim-muted)[\[#body\]]
]

// A break in the scene.
#let beat(body) = block(above: 1.2em, below: 1.2em, width: 100%)[
  #align(center, text(size: 9.5pt, fill: verbatim-muted, style: "italic")[— #body —])
]

// The legend of who is who: #speakers((P: "Chaplain (me)", R: "Ms. R., patient"))
#let speakers(cast) = verbatim-box(grid(
  columns: (auto, 1fr), column-gutter: 1em, row-gutter: 0.45em,
  ..cast.pairs().map(((k, v)) => (text(weight: "bold", smallcaps(k)), v)).flatten()))

// Title, details and confidentiality note. Also sets the running header.
#let verbatim-title(no: "Verbatim", unit: "", author: "", date: "", setting: "", other: "", length: "", confidential: "Confidential — use pseudonyms; destroy after the group session") = {
  verbatim-meta.update((no: no, author: author, confidential: confidential))
  align(center)[
    #text(size: 18pt, weight: "bold")[#no]
    #if unit != "" [#v(0.2em) #text(size: 11pt, fill: verbatim-muted)[#unit]]
  ]
  v(0.8em)
  verbatim-box[
    #grid(columns: (auto, 1fr), column-gutter: 1em, row-gutter: 0.45em,
      ..(("Written by", author), ("Date", date), ("Setting", setting), ("Other people", other), ("Length", length))
        .filter(r => r.at(1) != "")
        .map(r => (text(weight: "bold")[#r.at(0)], r.at(1))).flatten())
    #v(0.3em)
    #text(size: 9.5pt, fill: verbatim-muted, style: "italic")[#confidential]
  ]
}

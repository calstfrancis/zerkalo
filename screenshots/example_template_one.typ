// ZERKALO-TEMPLATE-BEGIN
// Created with Zerkalo · Chicago (Notes-Bib) style
// @zerkalo-style: chicago-notes
// @zerkalo-version: 0.16.0

#import "@preview/droplet:0.3.1": dropcap
#import "@preview/marginalia:0.3.1" as marginalia: note, notefigure, wideblock
#show: marginalia.setup.with()
#let dropcap = dropcap.with(font: "Goudy Initialen", height: 4, fill: rgb("#1e3a6e"))

#set page(
  paper: "us-letter",
  margin: (top: 1in, bottom: 1in, left: 1.25in, right: 1.25in),
  numbering: "1",
  number-align: bottom + center,
)

#set text(font: "EB Garamond", size: 12pt, lang: "en")
#set par(leading: 0.65em, spacing: 1.2em, first-line-indent: 1em, justify: true)

// Chicago (Notes-Bibliography) heading styles
#show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(weight: "bold")[#it.body]]
]
#show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[
  #set par(first-line-indent: 0pt)
  #align(center)[#text(style: "italic")[#it.body]]
]
#show heading.where(level: 3): it => block(width: 100%, above: 0.6em, below: 0.2em)[
  #set par(first-line-indent: 0pt)
  #text(style: "italic")[#it.body]
]

// ZERKALO-TEMPLATE-END

// ── Title block ─────────────────────────────────────────────────────
// Edit these variables to update the title page:
#let doc-title = "Example Template One"
#let doc-subtitle = "Chicago"
#let doc-author = "Каликс Бригид Мари Паламас Феликс Святой Франциск"
#let doc-affil = "Atlantic School of Theology"
#let doc-course = ""
#let doc-professor = ""
#let doc-date = "June 9, 2026"

#page(header: none, footer: none, numbering: none)[
  #set align(center)
  #v(1fr)
  #text(size: 16pt, weight: "bold")[#doc-title]
  #if doc-subtitle != "" [\ #text(size: 13pt, style: "italic")[#doc-subtitle]]
  #v(2fr)
  #if doc-author != "" [#doc-author]
  #if doc-affil != "" [\ #text(style: "italic")[#doc-affil]]
  #if doc-course != "" [\ #doc-course]
  #if doc-professor != "" [\ #doc-professor]
  #if doc-date != "" [\ #doc-date]
  #v(1fr)
]

#counter(page).update(1)
#pagebreak()

// ── Document body – DO NOT DELETE or Zerkalo template system will break
// ── Document body ───────────────────────────────────────────────────

= Привет

#dropcap[
  *#lorem(40)*
]
#columns(2)[
#lorem(30)@balthasarGloryLordTheological2009[p 59]

= This is a Heading 
#note[#lorem(15)]
#lorem(50)@bassGroundedFindingGod2015[pp 288-304]

#lorem(50)
== This is a Subheading
#lorem(30)

#lorem(25)

#lorem(50)
=== OMG, a Subsubheading
#lorem(50)
@allenSocialGospelCanada1975[89]
@sennProtestantSpiritualTraditions1986
#lorem(25)@blandNewChristianityReligion1973[212]
]
#pagebreak()

// ── Bibliography ────────────────────────────────────────────────────
#bibliography("citations.bib", style: "chicago-notes")

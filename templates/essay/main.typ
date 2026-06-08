#set document(title: "__NAME__", author: "Author Name")
#set page(margin: 1in)
#set text(font: "Linux Libertine", size: 12pt)
#set par(leading: 0.65em, justify: true)

#align(center)[
  #text(size: 16pt, weight: "bold")[__NAME__]
  \v(0.4em)
  Author Name
  \v(0.2em)
  #datetime.today().display()
]

#v(1em)

= Introduction

Your essay begins here.

= Conclusion

Conclusion goes here.

#bibliography("bibliography.bib")

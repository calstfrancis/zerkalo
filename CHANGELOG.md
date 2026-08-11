# Changelog

All notable changes to Zerkalo are recorded here.  
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.22.0] "Steady Hand" — Backups that happen without being asked

### Added

- **Error and notice dialogs now have a Copy button** next to OK, so a raw error message —
  a git failure, an export failure, anything shown in one of these boxes — can be pasted
  into a bug report or a chat instead of retyped by hand from a screenshot. `AdwMessageDialog`'s
  body text isn't selectable, so without this there was no way to get the text out at all.
- **Backups now happen on their own.** Once a backup location is set up, Zerkalo saves and
  sends a version automatically every so often while you write, and once more on the way
  out if anything's still unsent — so backing up no longer depends on remembering the sync
  button. It's quiet by design: failures show as a small toast rather than a popup, so an
  offline moment doesn't interrupt you, and it never blocks quitting for more than a few
  seconds.

### Changed

- **Plain language throughout setup, sync, and backup screens.** "Repository," "remote,"
  "commit," and "clone" — terms that meant nothing to a non-technical user — are now
  described in terms of what they do ("online copy," "backup location," "save a version").
  Covers the setup wizard's existing-copy path, the fallback sync dialog, the backup
  locations manager, header tooltips, the command palette, and sync failure/conflict
  messages.
- **The Help window's tabs are now a proper libadwaita view switcher** (matching Settings)
  instead of old-style GTK notebook tabs, with an icon per tab and more breathing room
  around headings and code blocks.
- **Help/FAQ/Cheatsheet formatting redone for clarity.** A first pass added a colored band
  behind section headings and a grey box around code — it read as clunky and more technical,
  not less, so it's gone: hierarchy now comes from weight, scale, and whitespace alone, with
  no color anywhere in the panel. (A follow-up attempt to also render the Cheatsheet's markup
  examples with real bold/italic/heading formatting didn't work reliably — reverted; the
  syntax reference stays as plain code for now.)

### Fixed

- **Sync could fail outright if the system's git was configured to sign commits.** Zerkalo's
  auto-backup and manual Sync both run `git commit` in the background, with no terminal
  attached — if `commit.gpgsign` is on, git tries to launch an interactive prompt to unlock
  the signing key, which can't work with no terminal to prompt on, and the whole sync failed
  with a raw gpg/pinentry error ("Inappropriate ioctl for device"). Zerkalo's own commits now
  explicitly skip signing; this doesn't touch the signing setting for anything else on the
  system, including commits made by hand.
- **The in-app changelog window only ever showed the first line of every bullet.** Every
  entry in `CHANGELOG.md` is hand-wrapped across several lines, but the window's parser only
  recognised a line starting with `## [`, `### `, or `- ` — every wrapped continuation line
  matched none of those and was silently dropped, so each bullet read as a bold fragment
  cut off mid-sentence. Continuation lines are now folded back into the bullet they belong
  to before it's rendered.
- **Section headers in the changelog window sat almost flush against the text below them.**
  10px above, 2px below — now 8px below, closer to matching.
- **Typst keywords lit up red inside ordinary prose.** The syntax highlighter matched bare
  words like `in`, `else`, `for`, `if`, `set` and `while` anywhere in the document, including
  plain writing with no code in sight. It turned out to not even help real code: `#if`,
  `#for`, `#let` and the rest were already being caught by the function-call rule, which
  starts matching one character earlier at the `#` and always wins — so the keyword rule
  never highlighted actual Typst code, only English words that happened to match. Removed.
- **The flatpak build only ever shipped a scalable SVG icon.** `install.sh`, the non-flatpak
  path, has long rasterised the same icon to PNG at 16–256px with `rsvg-convert`, because a
  launcher that doesn't invoke an SVG loader shows a blank or generic icon for an SVG-only
  app. The flatpak manifest never did the same, so the app icon showed inconsistently for
  flatpak installs depending on the launcher. It now rasterises the same sizes at build time.
- **The "Document body" and "Chapters" markers read as a bare warning with no explanation.**
  "DO NOT DELETE or Zerkalo template system will break" didn't say what shouldn't be
  deleted — the marker line itself, not the writing below it. Reworded to say so plainly.

---

## [0.21.0] "New Ground" — Nothing to install, nothing to type, and a window that explains itself

### Added

- **Save your own templates.** Set the template dialog up how you want it, press the save
  button beside "Your Templates", and give it a name — it joins the gallery under the
  built-in presets, previews like them, and starts a document exactly the same way next
  time. The title, date, abstract and keywords are deliberately left out, so a saved
  template can't stamp one document's front matter onto the next; your name, affiliation
  and the CV contact rows are kept, since a personal template is precisely where those
  belong. One file per template in `~/.local/share/zerkalo/templates/`, and a corrupt or
  hand-edited one is skipped rather than taking the gallery down with it.

- **Setting up is three screens with one decision each.** It was one long page of five
  sections, each with its own Apply button — seven separate actions in an order nothing
  announced, the first of which asked for a git name and email. Now: what this is for, sign in,
  confirm a name, done. Everything behind that last button — making the folder a repository,
  creating the repository on GitHub, linking it, the first save and upload — runs on its own
  with each step ticked off as it finishes, and any failure says which step and what to do.
- **Signing in supplies your name and email, so you're never asked for them.** They come from
  the GitHub account, using the address GitHub guarantees will attribute your work to you —
  not the public email field, which is empty for anyone with email privacy on and silently
  attributes every version to nobody. A typo here used to be permanent and invisible.
- **git is bundled.** The GNOME runtime it's built on has none, so the flatpak — the main way
  Zerkalo is installed — used to reach the host's git, meaning "install git in a terminal" was
  a prerequisite for saving your work. Nothing needs installing now.
- **No account needed.** The same screen offers backing up to a folder or drive — a synced
  Nextcloud or pCloud folder, a USB stick — or pasting the address of a repository you already
  have. Declining entirely is a plain option, and once declined you aren't asked again.
- **The repository is named after your work, not the program** — the work folder's name with
  `-docs` after it, so the default folder gives `zerkalo-docs`. Folder names that GitHub would
  reject (spaces, brackets) are converted rather than sent and refused.
- **A Tools window** (☰ → Tools) lists what's bundled and what's optional, replacing the last
  step of setup.

- **F1 labels everything on screen.** Press it and each panel and control gets a bubble saying
  what it's for, drawn over the running window rather than replacing it — the program stays
  visible underneath, because the point is to explain the thing you are looking at while you
  look at it. Bubbles are tied to real widgets, so hiding a panel or resizing the window moves
  them with it, and anything not on screen simply isn't labelled. Escape, another F1, or a
  click anywhere puts them away. Rebindable as `help_overlay` in `keybindings.toml`.

### Changed

- **Zerkalo no longer opens with a modal alert listing `sudo` commands.** That was the first
  thing a new user saw, about tools that are now bundled anyway. Missing optional tools are
  logged and shown in Tools; only a missing git — which in the flatpak cannot happen —
  still says anything, as a dismissible message.
- **Document fonts moved to Settings → Editor.** They were a step in setup, putting a font
  choice between a first-time user and getting started, for a setting whose defaults are
  already right nearly always.

### Fixed

- **The first upload to a brand-new repository goes through.** Sync pulls before it pushes,
  and on a repository that has no commits yet that pull fails — there is no branch there to
  pull from. Zerkalo treated any failed pull as an interrupted rebase, tried to abort a rebase
  that had never started, and reported the resulting "no rebase in progress" as a warning that
  the repository might be mid-rebase — then skipped the push, so the setup that had just
  finished successfully ended with nothing uploaded. A rebase is now only aborted when one is
  genuinely in progress, and a pull that fails purely because the remote branch doesn't exist
  yet carries on to the push that creates it.
- **"Double" line spacing is now actually double.** Typst's `leading` is the gap between
  lines, not a multiplier, so the old values had to be measured rather than assumed: Double
  was rendering at about 1.4× single spacing and "1.5 Lines" at about 1.2×. Every style
  Zerkalo offers — APA, MLA, Chicago, Turabian — requires true double spacing for
  submission, so documents set to it were out of spec. Documents written with the old
  values still open on the right setting instead of silently reverting to single.
- **Paragraphs are marked once, not twice.** Generated documents set a first-line indent
  *and* a fixed 1.2 em gap between paragraphs; academic manuscript style uses the indent
  alone, and on a double-spaced document the extra gap also broke the even line grid.
- **MLA documents keep their paragraph indents.** The MLA heading block turned off
  first-line indentation for the whole document rather than for itself — leaving the one
  style that most insists on indented paragraphs as the only one generated without them.
- **APA 7th no longer prints "Running head:"**, a label the 7th edition removed.
- **Executive paper size produced a document that wouldn't compile at all** — Typst calls
  it `us-executive`, and the template wrote `executive`.
- **An abstract fits on small paper.** It was inset a fixed inch on each side regardless of
  page size, which on A5 with wide margins left a column a few characters wide.
- **Changing the document font or size no longer rewrites the whole document.** The two
  pickers in the format bar used to regenerate the entire preamble from the settings file
  saved beside the document — so on a document without one (copied without its
  `.zerkalo.toml`, written before those existed, or corrupt) picking a font silently reset
  paper size, margins, citation style, title page and metadata to defaults. On a `.typ` file
  Zerkalo didn't create, it replaced the whole file with a starter template, with no
  confirmation and no backup. Both now edit only the one line that holds the value, and say
  so plainly when a document has no Zerkalo template block to edit.
- **Update Template Settings reads the document, not just its settings file.** Font, size,
  paper, margins, line spacing, page numbers, running header, packages, languages and heading
  numbering are all read back from the document itself, so settings you changed by hand, or
  that were never recorded, survive pressing Apply. Font size in particular had no reader at
  all: re-opening the dialog on a 14 pt document and applying reset it to 12 pt.
- **Every Apply takes a snapshot first**, so a template change that regenerates a title page
  you'd customised can be recovered from Browse Snapshots…. Applying to a document with no
  body marker — the case that replaces the whole file — also saves a `.typ.bak` next to it,
  and the confirmation says so instead of telling you to make your own backup.
- **Settings that can't be applied say so.** Applying a non-CV template to a CV was refused
  silently, which read as a dead Apply button.
- **Documents are written atomically.** Template writes went through a truncate-then-fill
  that could leave a `.typ` empty or half-written if Zerkalo died mid-save; they now write a
  temporary file and rename it into place. A document that can't be created — read-only
  folder, full disk — reports the error instead of closing the dialog as if it had worked.
- **The format bar shows the current document's font and size.** It kept the previous tab's
  values after switching tabs, and updated itself even when the change hadn't been applied.
- **A new repository starts on `main`.** Setup used to leave the branch to git's own default,
  which on many systems is `master` — so the first push created a second, unrelated branch
  next to the `main` GitHub had made.

### Import

- **Word, OpenDocument and Markdown files are converted by Zerkalo itself, with nothing to
  install.** These three formats are a ZIP of XML, a ZIP of XML, and text — so they no longer
  go through pandoc, which in the flatpak means a tool that has to be installed outside the
  sandbox and which most people won't have. LaTeX, EPUB, RTF and HTML still hand off to pandoc.
  Headings, bold and italic, lists (including nested ones), tables, links, block quotes, code
  blocks and embedded images all convert; images travel with the document.
  "Paste as Document" reads its clipboard text the same way, so it too needs no pandoc.
- **Anything a conversion couldn't carry across is said out loud** in the preview rather than
  quietly dropped — raw HTML in Markdown, and Word citations that come from a reference manager
  and can't be read at all.
- **Importing no longer writes into the folder your source file lives in.** The conversion ran
  with its output aimed straight at that folder, so the `.typ` and an extracted-images folder
  appeared beside your original *before* the preview asked whether you wanted them. Conversion
  now happens in a private working folder and nothing lands anywhere else until you press
  Import. Three consequences: importing from a read-only or shared location works instead of
  failing outright; cancelling an import no longer leaves a half-converted file behind; and
  closing the preview window with its close button cleans up, where before only the Discard
  button did.
- **Extracted images resolve.** With conversion moved out of the source folder, pandoc writes
  absolute image paths — and Typst reads a `/`-rooted path as relative to the project, not the
  filesystem, so those never load. They're rewritten to sit beside the document.
- **A large or noisy import can no longer hang.** Both output streams were captured and left
  unread until the conversion finished, so a document producing more warnings than a pipe holds
  would block forever with an "Importing…" toast and no way out. Streams are now drained as
  they're written. "Paste as Document" had a worse form of this — it fed the text in on the
  interface thread, so a large paste could freeze the whole window; it no longer runs a
  process at all.
- **Missing pandoc is detected before the conversion starts, and says how to fix it.** The only
  check was whether the process could be launched, which inside the flatpak tests
  `flatpak-spawn` rather than pandoc — so for the app's main distribution a missing pandoc
  produced a raw shell error. The instructions now also say that pandoc goes on your computer,
  not into Zerkalo, which is what running it outside the sandbox requires.
- **Too old a pandoc is reported up front**, by version, rather than surfacing mid-conversion
  as "unknown writer".
- **A destination that can't be written to is reported.** Both the single-file and folder
  imports discarded the result of writing the file, so an unwritable destination looked exactly
  like a successful import — in a batch it was even counted as one.
- **Import failures are explained in plain language** — the wrong format for the file,
  permission problems, and pandoc's own words when there's nothing better to say.
- **Working folders left behind by a crash are cleared out at startup.**

---

## [0.20.0] "Plain Sight" — A printer that prints, a window that reads as one thing, and errors that say what and where

### Added
- **A print system worth the name.** Ctrl+P opens a sheet showing what will actually be printed — the document, its page count, its real paper size — then hands off to the system dialog with everything already set. Page ranges are in the document's *own* numbering, so typing `12` gets you the page with 12 printed on it however the document numbers itself. Two or four pages a sheet and fold-and-staple booklets are imposed on the PDF itself, so ordering works on every printer and the output stays vector. Copies, two-sided, colour and layout are remembered between runs, with a proof, a finished copy and a booklet offered as starting points. "Print PDF" used to compile a file into a cache folder and open it in another application; it now goes to the printer.
- **Typst packages download on first use.** `#import "@preview/…"` previously only worked if the package happened to already be cached, and failed naming an internal path if not.
- **Typst warnings reach the error panel.** The compiler had been returning them all along and Zerkalo discarded every one.
- **A shared stylesheet across the whole suite**, so Zerkalo, Rubric and the rest describe a section, a row and a surface the same way from one file.
- **A save button in the header**, and Print reachable from the header and the command palette.

### Changed
- **Error messages are written in plain language.** The compiler's wording used to come first, with an explanation bolted underneath, so you had to get past "unknown variable: my-helper" before reaching anything you could act on. Now the plain sentence *is* the message — *Zerkalo doesn't know what "my-helper" means* — followed by what to do about it, and Typst's own suggestion where it has one. The exact compiler text stays under "Technical detail", and is what the copy button puts on the clipboard.
- **The window reads as one thing.** The header is down from twelve controls to four and the preview bar from ten to five; what reports a mode moved to the status bar, compile controls moved beside the editor they compile. Every window and dialog — Library, Settings, Help, Export, Font Management, the wizards — now sits on the same surface as the main window instead of taking whatever the toolkit gave it. Sidebar panels announce themselves with a dot and small capitals, their rows grouped into cards, and the header bar, status bar, sidebar and preview each sit on their own surface rather than one undifferentiated sheet.
- **The Library joins that design**, with single-line document cards, per-category colour cues, and tags set quietly after the title.
- **Icons are drawn from Adwaita whatever the desktop uses.** Under KDE they had been coming from Breeze — the right names, a different family from the interface around them. Colour scheme, accent and font still follow the system.
- **The menu is regrouped and says what it means.** Import sits with New and Open; Writing Stats leaves the settings block; menu shortcut labels come from your keybindings rather than being hardcoded; Keyboard Shortcuts and What's New are reachable from it at all; About is a proper about window; Ctrl+, opens Settings.
- **The preview canvas follows the colour scheme** instead of being a fixed light grey slab under a dark window.

### Removed
- **The notes panel and the Plan panel**, and the right-hand sidebar they were the last occupants of.

### Fixed
- **Compile errors point at the line they're actually on.** Every error and warning reported line 1, whatever had gone wrong and wherever it was — so the source line quoted beside each one was always the first line of the document. Typst attaches two kinds of source position to a diagnostic and Zerkalo decoded only the kind that almost never occurs, so no error ever carried a location at all. Errors inside an included file now name that file, clicking one jumps to the right place when the root file lives in a subfolder, and the line quoted is the one that was compiled rather than the one last saved to disk.
- **Memory grew for the whole time Zerkalo was open** — roughly 24 MB per thousand compiles of a three-line document, and far more for a real one.
- **Zerkalo wrote your document to disk every 30 seconds whether you asked or not**, which also meant crash recovery could never trigger. Saving is now crash-safe, going through the same write-then-rename path settings always used.
- **Settings changed in one dialog were silently reverted by another**, and a single bad line in `config.toml` discarded every setting and then overwrote the original.
- **Typing stalled on every word boundary with autocorrect on**, and lagged badly on long documents — four separate pieces of work were running far more often than they needed to.
- **Settings that did nothing now do something:** the Output folder was never read, and the Word count goal was saved and never applied.
- **Nine of the seventeen command-palette entries had no handler at all** and silently did nothing.
- **Update Template Settings behaved differently from the menu and the header**, the header route losing your locked author, affiliation and bibliography.
- **Closing Settings with Escape or the close button** left a previewed theme and font applied over settings that were never saved, and mistyped paths were accepted without a word.
- **Printing a CV produced nothing at all**, printing used the last saved version rather than what was on screen, failures were discarded silently, and the dialog opened on the desktop's default paper whatever size the document was.
- **The preview redrew every page on every frame**, and every compile PNG-encoded each page only to decode it straight back.
- **Every category in the library sidebar was the same blue.**
- **Choosing a spelling suggestion threw the editor to the top of the document.**

### Internal
- **The screenshot script can see your installed fonts.** It redirects `XDG_DATA_HOME` for isolation, which is also where fontconfig looks for user fonts — so the demo document was captured with substitutes for EB Garamond and Goudy Initialen. Harmless while warnings were being discarded; the moment they reached the panel it put three spurious font warnings across the release screenshot.
- **The suite goes from 269 tests to 385**, and the two largest files were broken up: the main window's constructor from 4,299 lines to roughly 2,000 across twelve files, and the template dialog's from 1,247 to 229.

---

## [0.20.0-dev13] — Errors that tell you where, and what to do

### Fixed

- **Compile errors point at the line they're actually on.** Every error and warning in
  Zerkalo reported line 1, whatever had gone wrong and wherever it was. Typst attaches two
  different kinds of source position to a diagnostic, and Zerkalo only decoded the kind that
  almost never occurs — so no error ever carried a location at all, and the panel's fallback
  put them all at the top of the file. This also means the source line quoted beside each
  error was, until now, always the first line of the document.
- **Errors in an included or imported file name that file**, instead of being attributed to
  the document you happen to have open.
- **Clicking an error jumps to the right file** when the root file lives in a subfolder. The
  location was resolved against the project folder while the compiler had reported it relative
  to the root file's own folder, so the two disagreed and the jump went nowhere.
- **Typst's own suggestions are shown.** Diagnostics often carry a hint — frequently the most
  useful sentence available, e.g. that `my-helper` was read as `my - helper` — and the parser
  recognised no such line, so every one was silently discarded.
- **The source line shown beside an error is the one that was compiled.** It was read from the
  file on disk while compiling runs against the unsaved buffer, so with unsaved edits the panel
  quoted a line the compiler never saw.
- **A folder name containing a colon no longer truncates the path** in a reported location.

### Changed

- **Error messages are written in plain language.** The compiler's wording used to come first,
  with an explanation bolted underneath, so you had to get past "unknown variable: my-helper"
  before reaching anything you could act on. Now the plain sentence *is* the message —
  "Zerkalo doesn't know what "my-helper" means" — followed by what to do about it. The exact
  compiler text stays available under "Technical detail", and is what the copy button puts on
  the clipboard, so it can still be searched for or pasted into a forum.
- **Messages name the thing that's wrong** where Typst tells us what it is: the missing file,
  the unrecognised option, the label nothing matches.
- **Warnings from the language server read the same as warnings from the compiler**, rather
  than being phrased differently depending on which one noticed the problem.

---

## [0.20.0-dev12] — The menu means what it says

### Fixed — menu and settings audit

- **The Output folder setting works.** Setting it saved to config and nothing ever read it;
  PDFs went on landing in the temporary folder. A per-project output folder still wins.
- **The Word count goal setting works.** It was saved but never applied — the only way to get
  the status-bar progress ring was a `// @zerkalo-goal:` comment in the document. The setting
  now applies immediately and acts as the fallback for documents without their own goal.
- **Every command in the palette does something.** Nine of the seventeen — New File, Open File,
  Export, Git Sync, Settings, Toggle Preview, Toggle Sidebar, New from Template and Focus Mode —
  had no handler and silently did nothing; Project Outline was a deliberate no-op. Each now runs
  the same code its menu row runs, so the two can't drift apart again.
- **The GOST Type B font toggle is remembered** between launches like every other toggle in the
  menu, and says so when the font isn't installed rather than appearing to do nothing.
- **Update Template Settings behaves the same from the menu and the header.** They were two
  copies of the same 110 lines and had drifted: the header button ignored the saved advanced-panel
  state, never passed the bibliography, and dropped the locked author and affiliation.
- **Menu rows that need an open document are greyed out** instead of being clickable and doing
  nothing — Update Template Settings, Repair Template Markers, Save, Save As, Browse Snapshots
  and Export for Web.
- **Abandoning Settings no longer leaves its live preview applied.** Closing the window with
  Escape, Alt+F4 or the close button kept the previewed theme, font and contrast on screen over
  an unchanged config; only the Cancel button reverted.
- **Paths in Settings are checked when you save.** A mistyped folder or a missing bib, CSL or
  Skrizhal file was accepted silently and only surfaced later as work going nowhere. Missing
  folders are offered for creation; missing files are reported.
- **Experimental mode actually gates the experiment.** Ctrl+Shift+I opened the hidden Import
  dialog and Ctrl+Shift+V ran Paste as Document regardless of the setting.
- **Changing the CV elements file takes effect immediately** instead of at the next launch, and
  the restart notice now also covers the output folder, not just the work folder.

### Changed

- **The hamburger is regrouped.** Import moves up beside New and Open — bringing a document in
  belongs with creating and opening one, not with exporting. Writing Stats leaves the settings
  block, being a report rather than a setting. The GOST and Autocorrect toggles are fenced with
  separators and set in Title Case so they stop reading as stray rows.
- **Menu shortcut labels come from your keybindings**, so rebinding in `keybindings.toml`
  relabels the menu instead of leaving it advertising the old key.
- **Keyboard Shortcuts is in the menu.** The window that lists your actual bindings was only
  reachable by pressing Ctrl+Shift+H, which you had to already know.
- **What's New is in the menu**, so the release notes are reachable after the upgrade that
  showed them.
- **About is a proper about window** — clickable repository and issue links, the licence, and
  the release name.
- **Settings uses the platform's page switcher** rather than plain notebook tabs, gains a
  Keyboard Shortcuts row pointing at `keybindings.toml`, has Ctrl+, as its shortcut, and greys
  out Simultaneous imports until experimental mode is on.
- **"Font Management…" is now "Document Fonts…"**, with tooltips distinguishing it from the
  editor font in Settings.
- **Every confirmation and notice in the app is drawn the same way.** Delete, Move to Trash,
  Restore Snapshot, unsaved-changes and the rest came from two different toolkits, so dialogs
  that ought to look like siblings didn't — most visibly on the destructive ones, where looking
  trustworthy matters most. Destructive buttons are now marked as such throughout, and the two
  separate copies of "Move to trash?" are one.

---

## [0.20.0-dev11] — The Library joins the design; the outline takes you there

### Changed

- **The Library is drawn in the suite's shared design.** Documents are single-line cards
  grouped under dotted small-caps headings — Pinned first, then Documents — each carrying one
  coloured cue for its category, its tags set quietly after the title, and the date and word
  count aligned right. The sidebar, list, and status bar sit on the same surfaces as every
  other window, and the header keeps one bordered control.
- **A tag's dot in the sidebar is its own colour**, not a red-to-blue ramp by how often it was
  used. The colour a tag was given is the only thing it should signal.
- **All Documents and Untagged have their own icons.** All Documents shared the recent-files
  icon with Recently Opened, and Untagged wore a close button's ×.

### Fixed

- **Clicking a heading in the outline moves the editor to it.** The jump was only wired to row
  activation, which a single click does not always raise, and it scrolled through a call that
  silently does nothing before the view has measured its lines.

---

## [0.20.0-dev10] — Help back where you can reach it; notes removed

### Changed

- **Help is a button under the preview again**, beside the page and zoom readouts, rather than
  tucked into the overflow with fit-width and open-in-a-window. Reaching a cheatsheet should
  not itself need looking up.
- **Autocorrect moved to the menu**, beside the interface-font switch. It is something you set
  once rather than a state worth a permanent word in the status bar.

### Removed

- **The notes panel is gone**, and with it the right-hand sidebar it was the last occupant of
  and the toggle that opened it — a button that opens an empty column is worse than no button.

---

## [0.20.0-dev9] — Every window and dialog joins the same design

### Changed

- **The header bar of every window and dialog now sits on the same surface as the main
  window's, with a line under it** — the Library, Settings, Help, Export, Font Management, the
  setup wizard, the template dialog and the rest. They were each taking whatever the toolkit
  gave them, so a window opened from Zerkalo did not look like Zerkalo.
- **The changelog window announces Added, Changed and Fixed the way sections are announced
  everywhere else** — a dot, then the name in letterspaced small capitals.
- **The compile-errors list is set like the rest of the suite:** each run of errors is one
  rounded card with hairlines between the rows, and a file name grouping errors beneath it is
  a section header rather than a dim caption.

---

## [0.20.0-dev8] — Two rows off the sidebar; Template joins the header

### Changed

- **Update Template is now simply "Template", in the header bar.** It was a full-width row
  above the sidebar panels — the only thing in that column that was not a panel — and it
  belongs with the other document-level actions.
- **The heading-depth filter is a button in the Outline header** showing the level in force,
  rather than a row of four chips above the outline. The Outline/Symbols switch moved into that
  header too, so the sidebar is now its two panels and nothing else.

---

## [0.20.0-dev7] — A quieter preview bar

### Changed

- **The bar under the preview is down from ten controls to five.** It now reads as where you
  are, how large the page is, and how the last compile went; fit-width, fit-page, the
  cheatsheet and open-in-a-window are behind a single button at its end. It also sits on the
  same surface as the rest of the window's chrome, with a line above it.

### Fixed

- **Refreshing the screenshots failed whenever Zerkalo was already open.** The throwaway copy
  used for capturing handed over to the running one and quietly exited, so nothing was ever
  drawn on the isolated display and the run gave up claiming the app never rendered — while
  opening a stray window in the real instance. It is given its own private session now.

---

## [0.20.0-dev6] — The sidebar gets the suite's cards; the preview canvas follows the theme

### Changed

- **The Outline and Citations lists are set the way the service order is in Rubric:** rows
  grouped into one rounded card with hairlines between them, and a quiet wash marking the
  current row rather than a filled accent bar. Both were loose rows with no grouping and no
  separators.
- **The preview canvas follows the colour scheme.** The ground the pages sit on was a fixed
  light grey, so in a dark window the preview sat on a pale slab. It is resolved as the pane is
  drawn, so switching light and dark is picked up straight away.

---

## [0.20.0-dev5] — A far quieter window: four controls in the header, one line of words below

### Changed

- **The header bar is down from twelve controls to four.** It had six text buttons and six
  icons in no particular grouping. What reports a mode — Simple, focus, Library, notes — now
  sits in the status bar with the other mode words; Git is a word there too, rather than an
  icon; compile mode and compile-now moved next to the editor they compile; and Print is
  reached from the menu, which already had it. The header keeps the sidebar toggle, the
  document title, Save, Preview and the menu.
- **The Outline and Citations panels announce themselves the way the rest of the suite does** —
  a coloured dot, the name in small capitals, and a count beside it.
- **The sidebar and the status bar sit on their own surface**, so the window reads as panels
  rather than as one sheet with lines ruled across it. This did not work in the previous build:
  a surface applied to a container is painted over by the lists and scrollers inside it, which
  is now handled once in the shared stylesheet for every app.

### Removed

- **The Plan panel is gone.** Its toggle is not: it was the only way to open the right-hand
  sidebar, which still holds Notes, so the button became the Notes toggle.

---

## [0.20.0-dev4] — A shared look across the suite, distinct surfaces, and icons that match the interface

### Added

- **Zerkalo now draws on a stylesheet shared across the whole suite.** Rubric, Zerkalo and the
  rest describe a section, a row and a surface the same way, from one file, so a change to the
  look lands everywhere at once instead of drifting app by app.

### Changed

- **The header bar, status bar, sidebar panels and preview each sit on their own surface.**
  Every part of the window used to be the same near-white, so it read as one sheet with lines
  ruled across it.

### Fixed

- **Icons were coming from the desktop's icon theme.** Under KDE that meant Breeze — the right
  names, but drawings from a different family than the interface around them, so "save" was a
  floppy disk rather than a download arrow. They are drawn from Adwaita now whatever the desktop
  uses. Colour scheme, accent colour and font still follow the system.

---

## [0.20.0-dev3] — A print system worth the name, a leak closed, saving you control, and category colours restored

### Fixed
- **Every category in the library sidebar was the same blue.** Categories are meant to take a colour derived from their name until you pick one deliberately, so they stay distinguishable at a glance — but the database gave each one that blue the moment it was created, so the code choosing a per-name colour never ran. Categories you have not coloured yourself now get their own colour again, in the sidebar and on document rows, and existing libraries pick it up on first open. Setting a category on a document no longer quietly locks in whatever colour the dialog happened to be showing; the colour is saved only when you choose one.

### Internal
- **The two largest untested modules now have tests, and the two largest files have been broken up.** The document library, the spell checker's text scanner and the editor's own helpers had no coverage at all between them; the suite goes from 269 tests to 371. The library's trash, restore and permanent-delete paths move real files around and were the most exposed — the case that matters most is restoring a document when something new has since taken its original path, which must land beside the newer file rather than overwrite it.
- **`app_window.rs` and `template_dialog.rs` are no longer single enormous files.** The main window's constructor was 4,299 lines and is now 2,000, split across twelve files by what each part actually does — menus, panels, citations, the file tree, startup. The template dialog's constructor went from 1,247 lines to 229, and its three "read the form" paths, which each cloned 35 widgets and repeated the same 70-line block, now share one. No behaviour was changed; every step was checked against a running build. Full account in `REFACTOR-PLAN.md`.

### Added
- **Typst packages are downloaded when a document first imports one.** `#import "@preview/…"` previously only worked if the package already happened to be in `~/.cache/typst/packages`; otherwise it failed with `file not found` naming an internal cache path. Packages — and their own dependencies — are now fetched on first use, the same way `typst-cli` does it, into the same shared cache. `@local` packages resolve too.
- **Typst warnings now reach the error panel.** The compiler returned them all along and Zerkalo discarded every one, so deprecations and unused imports were invisible even though the panel already knew how to display warnings. A document that compiles cleanly but warns now says so, without the banner or toast reserved for real errors.
- **Ctrl+P opens a print sheet before the printer's own dialog.** It shows what will actually be printed — the document, its page count, its real paper size — and offers the things only Zerkalo can know, then hands off to the system dialog with everything already set. It opens immediately with a spinner rather than making you wait on a toast, and can be abandoned mid-compile.
- **Page ranges in the document's own numbering.** Typst documents routinely disagree with their own page order — roman front matter, `counter(page)` resets, appendices restarting at 1 — and every print dialog counts physical sheets instead. Type `12` and you get the page with 12 printed on it. Where the two differ, the sheet says so; where they agree, it doesn't clutter the dialog saying it.
- **Two and four pages a sheet, and fold-and-staple booklets.** Imposition is done on the PDF itself rather than left to the printer driver, so booklet ordering works on every printer, output stays vector, and the preview shows the real first sheet — for a booklet, the last page sitting next to the first, with the fold marked.
- **Print settings are remembered.** Copies, two-sided, colour and layout persist between runs; the portal keeps nothing of its own, so every print used to start from the desktop defaults. Three starting points are offered above the individual controls — a proof, a finished copy, and a booklet.
- **Print is reachable from the header and the command palette**, not just the hamburger menu and the pop-out preview.
- **A save button in the header**, to the right of the git sync button. Same action as ≡ → Save, snapshot included.

### Changed
- **"Print PDF" is now "Print…", and actually prints.** It used to compile a PDF into `~/.cache/zerkalo/`, open it in whatever application owns PDFs, and leave you to print from there. It now goes to the printer. The compiled PDF is sent as-is, so text prints as vector at the printer's own resolution rather than being flattened to an image. On a desktop with no print portal, Zerkalo falls back to GTK's print dialog with pages rendered one at a time as the printer asks for them.
- **Printing the same document twice no longer compiles it twice.** The compiled document is kept until it's edited, so reopening the print sheet, adjusting a setting, or printing a second copy is immediate instead of costing another full compile. Cancelling mid-compile keeps the result too — the work is already paid for, so the next print gets it for free.

### Fixed
- **Memory grew for the whole time Zerkalo was open.** Typst caches compiled work in a process-wide store that nothing was clearing, so every recompile — and there is one behind every pause in typing — added to it permanently. Measured at roughly 24 MB per thousand compiles of a *three-line* document, and much more for a real one; a long writing session leaked steadily. The cache is now retired on the same cadence `typst-cli` uses, and a test fails if that is ever removed.
- **Zerkalo wrote your document to disk every 30 seconds whether you asked or not.** A timer saved every modified file, cleared its modified marker, and deleted the crash-recovery copy that had just been written — which meant the idle autosave never had anything left to save, and the recovery offer on reopening could never trigger, because the file on disk was always current. The timer is gone. Files change on disk when you save them, and crash recovery works.
- **Saving a document was not crash-safe.** Settings were written atomically but documents were not, so an interruption mid-write could truncate the file being saved. Every document write now goes through the same write-then-rename path, with the data flushed to disk before the rename and the file's permissions preserved.
- **Settings changed in one dialog were silently reverted by another.** The main window kept its own copy of the config while the export dialog, template dialog and setup wizard each re-read, edited and rewrote the file independently — so changing the default fonts through the wizard and then toggling anything in the main window put the fonts back. There is now one live config that every part of the app shares.
- **A single bad line in `config.toml` silently discarded every setting.** An unparseable file fell back to defaults everywhere without a word, and the next save wrote those defaults over the original. The file is now copied to a timestamped backup and the parse error reported, before defaults are used.
- **Typing stalled on every word boundary with autocorrect on.** Each space, full stop, comma, semicolon, colon, `!` and `?` started a `hunspell` process and waited for it to exit on the UI thread. Spell suggestions — autocorrect and both right-click menus — now run on a worker thread; the menu opens straight away and fills itself in.
- **A word added to one project's dictionary stayed accepted in every project opened afterwards.** Project dictionaries accumulated across switches instead of replacing one another.
- **Crash-recovery copies were kept forever**, including for files deleted long ago, and lived in `~/.config` beside the settings rather than with other regenerable state. They now live under `~/.local/state/zerkalo/` — existing ones are moved there on next launch — and are retired after 30 days.
- **Printing a CV produced nothing at all.** It compiled with no sys inputs, but CV entries reach the document through `skrizhal-cv-data`, so a CV document couldn't compile — and the error was discarded, leaving the button apparently dead. Printing and PDF export now both compile with exactly the inputs the preview uses. Export was silently affected by the same bug.
- **Printing used the last saved version of the document**, not what was on screen. Unsaved changes are now written first, as the PDF export already did.
- **Printing failed silently.** Compile errors and write failures were both discarded. Failures now appear in the error panel, rather than a button that appears to do nothing for several seconds.
- **Repeatedly pressing Ctrl+P started a compile per press**, each racing the others and writing the same file. Only one runs at a time now.
- **The print dialog opened on the desktop's default paper whatever the document was.** Neither print path told the system the document's actual page size, so anything that wasn't A4 or Letter — an A5 booklet, a custom-size card — was silently scaled or clipped. Both paths now send the document's real size and orientation. A document that mixes page sizes says so, instead of quietly printing them all at the first page's size.
- **The fallback print path rasterised at a fixed 300 dpi**, downsampling every 600 dpi printer and making a large-format page an enormous bitmap. It now uses the resolution the printer actually reports, within sane bounds.
- **Inside the Flatpak, the fallback print path could find no printers at all** — it needs a CUPS socket the sandbox wasn't granting. Granted now, so the fallback works where it's reached.
- **Printed documents accumulated in `~/.cache/zerkalo/`** under a name derived only from the file stem, so any two projects with a `main.typ` overwrote each other's. Nothing is written to disk to print any more.
- **The preview redrew every page of the document on every frame.** Scrolling, zooming or resizing repainted all pages — five fills and a scaled blit each — however little of the document was actually on screen, so a long document scrolled progressively worse. Only the pages touching the visible area are painted now.
- **Every compile PNG-encoded each page only to decode it straight back.** The compile thread compressed each rendered page to PNG and the main thread immediately decompressed all of them again; the bytes never left the app. Pages now go straight from the renderer to the screen as raw pixels, which drops the compression work from each compile and — because the decode ran on the main thread — removes a freeze after every recompile that got longer the more pages the document had.
- **Editing during a slow compile started a second compile on top of it.** Typst can't be interrupted mid-compile, and each edit spawned another one regardless, so on a document slow enough to compile that several could stack up and compete with the interface for processor time. A request arriving mid-compile now waits and runs once, when the current one finishes.
- **Typing lagged badly on long documents, and got worse the longer the document and the more tabs were open.** Four separate pieces of work were running far more often than they needed to. Whenever the document had any compile error or warning — which, mid-edit, is nearly always — every keystroke re-applied the error squiggles across *every open tab*, sweeping each buffer end to end to clear the old marks first. That now runs once 250 ms after you stop typing, and only on the tab you actually edited. The status bar's section word count re-read the document three times over — once per line, through the text widget — every time the cursor changed line, so holding an arrow key meant one full scan per line; it now reads the buffer once and waits for the cursor to settle. The comment highlighting re-tagged the whole document every time typing paused, even when no comment had moved; it now compares the comment spans first and does nothing when they're unchanged.
- **Choosing a spelling suggestion threw the editor to the top of the document**, the same GTK scroll-to-mark animation behind the paste jump fixed in 0.19.0 — dismissing the suggestion popover hands focus back to the editor and GTK animates the viewport away. Your place is now held through it, for both the right-click menu and Alt+Enter.

---

## [0.19.0] "Quiet Silver" — Inline autocomplete, a status bar that stays out of the way, and a viewport that stays put

### Changed
- **Autocomplete suggests inline instead of covering your text.** Typing `#` used to throw a large list over the document. Now the best match appears as dim ghost text right after the cursor, previewing what will actually be inserted — `#fig` shows `ure(image(""), caption: [Caption text])` — and Tab accepts it. A compact ranked list joins in only once you've typed two characters: a third of the old width, single-line rows, matched letters in bold, and a count when it's showing a slice of the matches. The ghost is drawn over the view, never inserted into the document, so it can't be saved, counted or sent to the language server by accident.
- **Suggestions say what they are and which key takes them.** The status bar names the current suggestion and leads with its signature — `figure(image(""), caption: […])` — falling back to prose where there isn't one, with the live keys after it. Arrowing through the list re-describes each entry there, so the explanation never covers what you're writing. A bare `#` says what to do next, and says when only built-in snippets are available because no language server is running.
- **Completions are found by the name you'd actually type, and learn from you.** They were matched on their display title, so `#pagebreak` found nothing. Matching is now on the Typst name, ranked prefix-first, and `#break` still finds `pagebreak`. The name you choose for a prefix is remembered per project and offered first next time, and names already used in the document outrank ones that aren't.
- **Citations and CV entries behave the same way.** `@` and `!` got the inline suggestion, the status-line description and the same two-character gate. A bare `@` used to drop the whole bibliography over the text.
- **Escape means "not for this word".** It dismissed a suggestion and the next keystroke brought it straight back; now it stays quiet until you move on. It also no longer deletes what you typed — Escape with the list open used to erase back to the `#`.
- **Controls moved to where they belong.** The status bar runs the full width of the window now, under the sidebar as well as the editor, and gives its left half to the suggestion hint with every standing control packed to the far right. Simple Mode and Focus moved up to the header beside Library; the compile-mode toggle (auto / on save / manual) beside the compile buttons it describes; the root-file controls beside the document title; and the GOST Type B font switch — a once-in-a-while setting, not a status — into the ≡ menu.
- **Project controls can be put away.** A single-file document has no root to choose, so the root controls and the "main.typ detected" banner were taking up space with no way to close them. They now carry a dismiss button that shuts them for that project and keeps the banner from returning, remembered in the project's own config. The "project" toggle stays in the header, so one click brings them back.

### Added
- **`#cv-profile("name")` renders a whole CV profile** — every section, in the profile's own order, each with its heading — replacing a hand-assembled run of `#cv-section` calls. Profiles are built in Skrizhal's "CV Profiles" dialog and stored in the same CV elements file; unlike a plain category/tag filter they also carry section order and explicit keep/drop lists, so a one-off exception ("drop that job from *this* CV") doesn't need a single-use tag invented for it.

### Fixed
- **Zerkalo crashed outright when resizing the editor/preview split, dragging the sidebar edge, or toggling the sidebar.** As the formatting toolbar narrowed, controls were moved into its overflow menu, but the bookkeeping that remembers where each control belongs was only ever seeded with one entry per zone instead of one per collapsible group. The first collapse exhausted it, and the moment the bar widened again the app aborted. Controls also return in their original order rather than a shuffled one.
- **Copying or pasting threw the editor to the top of the document.** Two separate causes. Copying — most reliably from the right-click menu — left the editor's idea of "where you were" pointing at the cursor rather than the viewport; the saved position now follows every scroll, and copy and cut pin the viewport outright since neither should move it. Pasting was GTK itself: it animates the viewport to the top after a paste, over a dozen eased frames, with focus never leaving the editor and the cursor still mid-document. Your place is now held through that animation, so nothing moves. A paste landing off-screen still scrolls to where the text went.
- **The right-click menu covered the spelling suggestions.** The spell popover opened on right-click and GtkTextView's own context menu opened on top of it, hiding the suggestions the right-click was for. Zerkalo now claims that click when it has suggestions to show.
- **GitHub sync could send your sign-in token to non-GitHub remotes** — a backup remote pointing at any HTTPS host had the token injected into its URL during sync. Token use is now scoped to github.com only, and passed via a scoped git config header instead of the URL, so it no longer appears in process listings either.
- **Cancelling "Sign in with GitHub" didn't actually cancel it** — closing the dialog left the background approval check running; if you approved the code afterward, the account still got silently connected.
- **Renaming a citation key could silently overwrite a different, already-existing key**, deleting it with no warning. Renaming now refuses and shows an error instead.
- **Hyphenated citation keys** (e.g. `smith-2020`, a normal BibTeX convention) **weren't renamed in the document text**, only in the bibliography file, leaving dangling citations behind.
- **Snapshot/version history could mix together unrelated files** that happened to share a name in different folders, or unrelated projects whose folders happened to share a name — each now gets its own history.
- **A CV elements file containing profiles left the `!` autocomplete and the CV Elements panel completely empty**, with no error shown: one unrecognized block failed the whole parse. Reserved blocks are now filtered out first, so an unrecognized block costs you that block rather than every entry in the file.
- **`#cv-section` would have rendered Skrizhal's profile block as if it were a CV entry** — reserved (`_`-prefixed) keys are now skipped.
- **A CV's style selector could show the wrong style** when reopening "Update Template Settings," depending on unrelated internal list ordering.
- **Dragging to reorder files in the sidebar could show a "rejected" bounce-back** even though the reorder actually succeeded.
- **Exporting to Word/HTML/EPUB/etc. could silently produce broken output** for documents missing certain internal template markers, with no warning that something went wrong.
- **Autocomplete could get misaligned** on lines containing emoji or certain rare symbols before the cursor.
- **Quick-fixes for compile errors** added an extra blank line above the inserted text, and could silently convert Windows-style (CRLF) line endings to Unix-style (LF) for the whole file.
- **"Add to Project" and "New Document" dialogs treated Cancel/Escape as confirming**, creating the thing you'd just declined.
- **Clicking Cancel mid-compile could clear real compile errors and show a false "Compiled successfully" toast.**
- **A malicious or corrupted project's `.zerkalo/config.toml` could point the compiler at arbitrary files outside the project** — root paths are now confined to the project directory.
- **Replace All could silently corrupt file contents when the replacement text contained a `$`.**
- **Restoring a document from Trash could mark it "restored" in the library even though the file was never actually moved back.**

## [0.18.0] "True Type" — Default fonts, CV template fixes, onboarding polish

### Added
- **Default Fonts step in onboarding** — pick a default sans and serif font in Setup & Onboarding; new documents and template previews use them until you choose something else per-document.
- **Soft-locked default fonts in Font Management** — disabling a font that's set as your default sans or serif is blocked with a warning explaining you need to choose a replacement first; "Disable All" now skips them too.
- **Descriptions in the CV style switcher** — the in-document format-bar CV style dropdown (Modern/Academic/Classic/Two-Column) now shows a one-line description for each style, matching the New from Template gallery.

### Changed
- The formatting toolbar above the editor now collapses lower-priority controls (size, font, CV style, figure, table, line numbers, pagebreak, headings) into a trailing "more" menu as the pane narrows, instead of forcing the editor to overflow underneath the sidebar.
- In CV mode, the New from Template Metadata group now shows CV-relevant fields (Email, Location, Phone, Links/Website) instead of academic-paper fields (Subtitle, Course, Professor) that were silently ignored for CVs.
- **Polished Setup & Onboarding**: an at-a-glance "X of 5 sections set up" progress summary, section icons and "Optional" badges, auto-scroll to the first unfinished section on reopen, a copy button on install-hint commands, live re-checking of missing tools when the window regains focus (no more manual re-clicking "Verify" after installing something), required tools (git) now read more urgently than optional ones (hunspell, Skrizhal), the Default Fonts step previews each option in its own font and defaults to a common font instead of whatever sorts alphabetically first, and "Account & Sync" / "Editor Preferences" subheadings separate the five sections into two groups.

### Fixed
- **Switching CV style away from Two-Column kept the old two-column layout** — the in-document style switcher only changed the `CV_STYLE` label, never regenerating the document body, so a résumé switched from Two-Column to Modern/Academic/Classic (or back) kept rendering with the wrong column structure.
- **CV — Two-Column's Award entries ran title and organization together on one line** ("Organization — **Title**") while Education entries already stacked title / organization / date on separate lines. Award now uses the same three-line, left-aligned layout as Education, in both current documents and documents created before the Skrizhal rewrite.
- **A single-line description on any CV entry (Employment, Education, Award, etc.) failed to compile in every CV style**, not just Two-Column — Skrizhal's schema deliberately allows a one-line `description` to be written as a bare scalar instead of a list, but the shared `cv-desc-block` Typst helper assumed it was always an array and called `.map()` on it, crashing with "type string has no method `map`". Now normalizes a scalar description to a single-item list before rendering.
- **Editing metadata (Email/Location/Website/etc.) via "Update Template Settings" on an existing CV crashed with "unknown variable: section"** — the dialog tracked which template kind (Academic/Book/CV/Letter) to regenerate in a separate piece of state from the CV Mode toggle, and only the toggle got restored when reopening the dialog on an existing document; the template-kind state silently stayed at its Academic default, so Apply regenerated an Academic preamble (which never defines `#section`) onto a preserved CV body that still called it. Now correctly restores the template kind too, from the document's sidecar or its `@zerkalo-kind` marker.
- **Setup & Onboarding could open 2-3x wider than its intended 640px** — long unwrapped content (status labels showing a full repository URL, the "Unknown distro" install hint joining three package-manager commands) forced GTK to size the window to fit the widest child's natural width. Now clamped and wrapped the same way `WelcomeWindow` already was.
- **"Update Template Settings" could still regenerate a CV as Academic, crashing with "unknown variable: section"**, on a document whose sidecar had drifted to the wrong kind on an older Zerkalo version — the dev3 fix above only covered the common case, not an already-corrupted sidecar that kept perpetuating itself. The dialog now cross-checks the document's actual body content (does it call `#cv-section(...)`?) and trusts that over a stale sidecar/marker, and `apply_body_splice` itself now refuses to combine a CV body with a non-CV preamble instead of silently writing a document that won't compile.
- **Re-picking a CV style in "Update Template Settings" didn't actually change the layout** — Apply regenerates a fresh preamble and splices it onto the existing body to avoid clobbering hand edits, but the splice always preserved the old body verbatim, so switching e.g. Two-Column back to Modern kept the old two-column grid. `apply_body_splice` now regenerates the body when the CV style crosses the Two-Column ↔ single-column boundary, in either direction.
- **CV — Two-Column had no visible description in "Update Template Settings"** — its style picker there is the same "Style" dropdown used for citation styles (SBL/APA/MLA/…) elsewhere in the dialog, so in CV mode it was still showing citation names and a citation-related subtitle instead of CV style names. It now shows Modern/Academic/Classic/Two-Column with a live description matching the current selection, and correctly restores which CV style a reopened document is actually using (it was previously reading the wrong marker and silently defaulting to Modern).

---

## [0.17.0] "Clear Glass" — Sign in with GitHub, CV Mode, reliable font rendering

### Added
- **Sign in with GitHub (device flow)** — the setup wizard and Settings → GitHub Sync now offer a "Sign in with GitHub" button instead of requiring a hand-generated Personal Access Token. Approve the sign-in with a short code at github.com/login/device; the resulting token is stored in the system keyring instead of plaintext config.
- **Create & Link a repository from within Zerkalo** — the setup wizard can create a new GitHub repository (name + public/private) via the GitHub API and link it as `origin` in one step, instead of requiring you to create the repo manually on github.com first and paste its URL back in. Pasting an existing repo's URL remains available as a fallback.
- Settings → GitHub Sync shows connection status ("Connected as `<username>`") with Sign in / Disconnect actions.
- **CV Mode toggle** in the New from Template / Update Template Settings dialog — switch it on to show only the CV templates, hide Sections/Packages (irrelevant to a résumé), and reveal a prominent, explained Skrizhal CV Elements file picker. Auto-enables itself when reopening template settings on a document that's already a CV.
- **Skrizhal now appears in onboarding** — added to the setup wizard's Tools checklist alongside git/pandoc/hunspell, with an explanation of what it does and install instructions, instead of being mentioned nowhere until you stumbled into Settings → Extras.
- **Academic Letter preset now generates an actual letter** — date, recipient block, salutation, and a signed closing — instead of reusing the generic essay title-page-and-Introduction body it shared with every other academic preset.
- **CV — Two-Column now has a Profile section** — a full-width professional-summary paragraph above the sidebar/main-column split, editable via the `cv-summary` variable.

### Changed
- GitHub tokens are now stored in the system keyring (via the `keyring` crate) rather than in plaintext in `config.toml`. Existing plaintext tokens are migrated into the keyring automatically on first load after upgrading.
- Reworked the Help/Cheatsheet/FAQ reference panel's visual hierarchy — larger, accent-colored headings, tighter section spacing, and inline `code` highlighting for key names, shortcuts, and function calls (including the `!` CV autocomplete trigger, which was previously easy to miss in the CV/Résumé Helper Reference).
- **CV Mode toggle** in New from Template is now a compact "CV" label + switch in the header bar instead of a full-width bar with the label and switch pinned to opposite ends of the window. The Skrizhal CV Elements file picker moved out of its own cramped bar and into the Template tab's left column, alongside the preset list.
- **New from Template presets now set a running header**, giving each preset a more distinct look instead of all defaulting to none: Research Article (APA) shows the title (its running head), GOST Technical Report and Book / Long-form show the current section/chapter, and LaTeX Look shows Author · Title.
- Removed the old static Blank/Essay/Journal-Thesis/Theological-Journal template system (`src/templates.rs`'s `BuiltinTemplate`/`UserTemplate`/`AnyTemplate`, and their `templates/` files) — it was never wired into the UI and had been fully superseded by the New from Template preset gallery.

### Fixed
- **CV preset previews in New from Template never actually rendered** — CV — Modern/Academic/Classic/Two-Column all `#import "cv-helpers.typ"`, but the gallery's preview renderer didn't provide it (it relied on a file at `/tmp/cv-helpers.typ` that nothing ever wrote), so every CV preset silently failed to compile and showed a blank preview pane.
- **Template previews (and CV documents defaulting to "Linux Libertine") rendered in the wrong font** — Zerkalo never enabled the embedded-Typst-compiler's `embed-fonts` feature, so none of Typst's bundled fonts (Libertinus Serif, New Computer Modern, DejaVu Sans Mono) were actually available, and the requested font — "Times New Roman" or "Linux Libertine", neither an exact match to anything on disk — silently fell back to whatever font Typst's FontBook picked for an unknown family, usually a mono font. Now ships with `embed-fonts` enabled and points previews/CV defaults at "Libertinus Serif", which is guaranteed to render correctly regardless of what fonts the host system has installed.

---

## [0.16.1] "Even Column" — CV — Two-Column template, Skrizhal integration

### Added
- **CV — Two-Column template** — a new résumé preset with an Education/Skills/Interests/Awards sidebar beside an Experience/Presentations/Extracurricular main column, selectable from New from Template alongside CV — Modern/Academic/Classic.
- **New CV helper functions**, available in every CV style: `#mylink(url, label)` for underlined clickable links, `#taglist(items)` for a plain list without a category label (Interests, etc.), and `#presentation(role, venue, title, years)` for talks/publications entries.
- The status-bar CV style switcher gained a fourth "Two-Column" option.
- **Skrizhal CV element integration** — when a document is in CV mode, the citation panel connects to a [Skrizhal](https://github.com/calstfrancis/skrizhal) CV-element YAML database instead of a bibliography: the panel header and search placeholder swap to CV Elements, and clicking or double-clicking an entry inserts `#cv-entry("key")` at the cursor.
- **"Skrizhal" launch button** — in CV mode, the citation panel's file-name label is replaced with a button that launches the installed Skrizhal flatpak directly to edit the CV-element database, with a toast if it isn't installed.
- **`!` autocomplete for CV entries** — mirrors the existing `@` citation autocomplete: type `!` followed by a key, title, organization, or tag fragment to get a filtered popup, with Tab/Enter to insert and arrow keys to navigate.
- New `cv_elements_path` setting (global and per-project override, like `bib_path`) points Zerkalo at a Skrizhal YAML file; the file is watched and reloaded automatically on change, and is editable directly from Settings → Extras.

### Changed
- **CV — Two-Column template now matches a plain hand-written CV's formatting exactly**: no rule under section headings (native heading style instead), dates stack below the title/company line (beside it only for modern/academic/classic), Skills/Software/Interests render as real bullet lists, and awards omit the dash entirely when there's no separate awarding body to name (`#award(title, none, years)`). The header layout, spacing, and link colour (plain blue) now mirror a plain reference document precisely, so importing an existing hand-written CV into this template is a seamless, near-lossless conversion.
- Find bar: the current search match is now highlighted with a bright background tag (in addition to the selection), and scrolls to center reliably via a buffer mark instead of occasionally no-op'ing on unvalidated line heights.
- Changelog window: entries now show the version and title on separate rows (so long titles wrap instead of eliding) and tag the currently-running version with a "Current" badge.
- Settings dialog is now resizable, with a larger default size to fit the new CV Elements row.
- Release screenshots moved from tracked root-level `zerkalo.png` / `packaging/zerkalo-screenshot.png` to a `screenshots/` directory, attached to GitHub Releases automatically via CI.

### Fixed
- **CV — Two-Column (and other sidebar-style CVs): Education entries with a differently-cased
  category** (e.g. Skrizhal's free-text category field saved as `education` instead of
  `Education`) **rendered degree and school crammed onto one line** instead of the intended
  degree / school / dates stacked layout — the category dispatch in `cv-helpers.typ` matched
  case-sensitively, so anything not spelled exactly `Education` silently fell back to the generic
  job-shape renderer. Category matching (both which shape an entry renders as, and which
  `#cv-section` it's included in) is now case-insensitive throughout, matching skrizhal-core's own
  case-insensitive category lookup.
- **CV — Two-Column header spacing** — the gap between the name and the contact line was noticeably tighter than a plain hand-written CV's, because the blank line that produces that gap in a hand-written document falls back to Typst's default paragraph spacing, which this template overrides to stay compact everywhere else. Calibrated the header's own spacing instead of touching the shared paragraph spacing (which would have loosened the gap between dates and bullet points throughout the rest of the CV).
- Backward compatibility: CV documents created before the Skrizhal `#cv-section` rewrite (which called `#job`/`#edu`/`#skill`/`#award`/`#presentation` directly) keep compiling after a template settings change — the legacy helper functions are re-injected into the regenerated preamble when detected.
- `cv-helpers.typ` is now injected into every CV document's preview compile unconditionally, not just when a Skrizhal file is configured, since CV templates unconditionally `#import` it — previously this failed to compile for any CV document that hadn't been pointed at a Skrizhal file yet.

---

## [0.16.0] "Open Harbor" — universal document import, readable errors, visual polish

### Added
- **Universal document import** — LaTeX, Word, Markdown, OpenDocument Text, HTML, EPUB, RTF, and PDF, all via ☰ → Import, drag-and-drop, or Paste as Document (Ctrl+Shift+V and Ctrl+Shift+I). Batch/folder import runs several conversions at once (concurrency configurable in Settings), with a preview-before-commit dialog (destination choice, conversion-quality summary, surfaced pandoc warnings) and an Import History with retry, single/batch undo, search, duplicate-import warnings, and Zotero/Mendeley/EndNote citation-field detection for DOCX.
- **Dropcap color picker** and the **Marginalia package** (margin notes, wide-blocks, captioned margin figures), plus substantial in-app descriptions for every optional package in the Packages tab.
- **"LaTeX Look" template preset**, new "LaTeX" and "Ross" margin presets, and custom page size/margin/font-size fields in New from Template / Update Template Settings.
- **Double-click a word in the preview** to jump to it in the source, complementing the existing Ctrl+Click paragraph jump.

### Fixed
- **Error messages are now genuinely readable** — the hover Fix It suggestion was silently broken (matching a fabricated string instead of the real diagnostic), the panel's Fix button only ever worked for one error type, and error-line highlighting could land on the wrong file or never appear on background tabs. All three are fixed, plus plain-language explanations for common Typst mistakes (missing/unexpected arguments, type mismatches) and a poisoned compiler-cache lock that could break compilation for the rest of a session.
- **Visual inconsistencies** — mismatched status-bar separators, an off-center SIMPLE toggle, a hardcoded goal-ring color, and a plain-label library empty state are all fixed or polished; diff colors and uncolored tag/category chips that were unreadable or indistinguishable are fixed.
- **The in-app "What's New" dialog had gone stale for several builds** and never mentioned any of the above — now current, and will be kept current going forward.

### Changed
- Consolidated all app-wide CSS into a single file; LSP status and compile-mode colors now resolve live from the active theme instead of hardcoded values.

---

## [0.15.1-dev7] — Import system: batch, drag-drop, history, and more

### Added
- **HTML and EPUB import** — join LaTeX/Word/Markdown/ODT in the ☰ → Import picker, via the same shared pandoc pipeline.
- **Drag-and-drop document import** — drop a `.tex`/`.docx`/`.md`/`.odt`/`.html`/`.epub`/`.pdf` file onto the editor to import it directly, no picker dialog needed.
- **Import Folder…** — convert every matching file in a folder in one go (format + destination chosen once, applied to all; each file still gets its own entry in Import History).
- **Import progress and cancellation** — importing now shows an "Importing…" toast with a Cancel button that actually kills the pandoc process, instead of leaving the UI with no feedback until the file opens.
- **Import preview dialog** — after conversion, a read-only preview of the generated Typst appears before anything is written, with a choice of destination ("This project" or next to the source file) and Import/Discard buttons — previously the file was written and opened immediately with no way to review or redirect it.
- **Bibliography auto-detect after import** — if no bibliography is configured yet and a `.bib`/`.yaml`/`.yml` file sits next to the just-imported document, a toast offers to set it.
- **Import History** — a small persisted log (`src/import_log.rs`) of past import attempts, reached via a clock icon in the Import dialog; shows source, format, timestamp, success/failure, and a "reveal in file manager" action for successful imports whose output still exists.
- **Smarter PDF import** — short, isolated lines in `pdftotext` output are now promoted to `== Heading`s instead of the whole document coming through as an undifferentiated wall of text.
- **RTF import** — one more format in the same pandoc pipeline.
- **Paste as Document** — paste plain text (e.g. copied prose) directly into a new Typst document via the same markdown pandoc path, without saving a file first.
- **Multi-select in the single-file import picker** — pick several files at once from a format row; more than one routes through the same sequential batch pipeline "Import Folder…" uses.
- **"Include subfolders" for folder import** — an optional recursive scan, skipping hidden directories and any `_media` folder pandoc itself generated.
- **Bounded-parallel batch import** — folder/multi-file import now runs up to 2 pandoc conversions at once (previously strictly one at a time), with a single progress toast updated as each file finishes.
- **Retry, delete, and clear actions in Import History** — a failed import can be retried without re-picking the file/format; individual entries or the whole history can be removed.
- **Conversion-quality summary in the preview dialog** — a line above the preview text reporting word/heading/image/citation count, plus an equation count flagging math that may need manual review.
- **Undo action on successful import** — the post-import toast includes an "Undo" button that closes the tab and deletes the just-written file.
- **Citation-without-bibliography nudge** — if a converted document cites sources but no `.bib` was found nearby, a toast suggests exporting a Zotero/Mendeley/EndNote library to `.bib` (this is a nudge, not extraction — proprietary citation-manager field codes in DOCX/ODT aren't something pandoc's CLI can export on its own).

---

## [0.15.1-dev6] — Robust document import (LaTeX, Word, Markdown, ODT)

### Added
- **Markdown (.md) and OpenDocument Text (.odt) import** — ☰ → Import now covers LaTeX, Word, Markdown, and ODT via pandoc, all sharing one code path instead of duplicated per-format handlers.

### Fixed
- **Document import (LaTeX/DOCX/Markdown/ODT) froze the UI** — the pandoc subprocess ran synchronously on the GTK main thread; it now runs on a background thread with the app remaining responsive during conversion.
- **Embedded images in DOCX/ODT were silently dropped on import** — pandoc is now run with `--extract-media`, and invoked with the input file's directory as its working directory using bare relative filenames so the generated `#image(...)` paths are relative (verified: passing absolute paths made pandoc emit OS-absolute image paths, which Typst's root-relative path resolution can't follow — a real bug caught by manually running the exact pandoc invocation before shipping it).
- **Re-importing a file could silently overwrite an existing same-named document** — the output path now gets a `(1)`, `(2)`, ... suffix on collision, matching the existing "Untitled 2.typ" convention used elsewhere.
- **An old pandoc without Typst-writer support produced a raw, confusing error** — common failure signatures (e.g. "unknown writer: typst") are now translated into a plain-language message.

---

## [0.15.1-dev5] — Visual polish pass (status bar, goal ring, library empty state)

### Fixed
- **Status bar had two inconsistent separator styles** (a real `Separator` next to plain "│" labels) and one toggle's label sat a few pixels off from its siblings — unified to one style, aligned margins.
- **The "SIMPLE" status-bar toggle wasn't actually centered** despite the code intending it to be — a spacer was missing on one side; it now centers correctly in the free space.
- **The word-count goal ring used hardcoded colors** that never matched the user's accent color — now pulled from the active theme.
- **Library's empty state was two plain labels with no icon** — replaced with a proper `AdwStatusPage`, matching libadwaita convention used everywhere else in the app.

### Improved
- Welcome window's ASCII layout diagram is slightly smaller, reducing overflow risk on narrower windows.

---

## [0.15.1-dev4] — Marginalia package, error-handling reliability pass

### Added
- **Marginalia package** — new "Marginalia" toggle in the Packages tab adds configurable margin notes (`#note[...]`), wide-blocks that spill into the margin (`#wideblock[...]`), and captioned margin figures (`#notefigure(...)`)
- **Substantial package descriptions** — every entry in the Packages tab (Droplet, Codly, Showybox, Gentle Clues, Tablex, Marginalia) now explains what the package does and the basic syntax to use it, instead of a one-line label; descriptions can now wrap to multiple lines

### Fixed
- **Hover-over-error tooltip never showed real fix suggestions** — it was matching a fabricated placeholder string ("Error on line N in file.typ") against the fix-pattern list instead of the actual compiler/LSP message, so the "Fix It" suggestion almost never appeared. It now reads the real diagnostic message.
- **Error panel's "Fix" button only ever appeared for one error type, and always applied a blind whole-document fix** — it's now available for every error pattern that has a real targeted fix (missing closing brace/bracket/paren, unknown variable, unclosed delimiters) and applies the fix at the correct line instead of guessing across the whole file.
- **LSP-reported errors skipped the plain-language explanations** that compiler errors already got (e.g. "the bibliography key was not found — check that…"); both sources are now enriched consistently.
- **Error-line highlight could apply to the wrong file, or never appear on background tabs** — the subtle red paragraph highlight behind an error line was applied only to whichever tab happened to be active at compile time, using line numbers pooled across every open file. A multi-file project could highlight the wrong line in the wrong tab, and switching to a background tab with an error never showed the highlight at all. It's now applied per-file across every open tab, matching how the gutter dot already worked, and LSP-only diagnostics now get the same highlight compiler errors did.
- **A poisoned compiler cache lock could break compilation for the rest of the session** — `ZerkaloWorld`'s source/file caches used `Mutex::lock().unwrap()`, so any unrelated panic while a lock was held would turn every future compile into an immediate crash instead of a normal error. The locks now recover from poisoning (the cached data can't be left in a half-written state, so this is safe).

### Improved
- **More error messages now get plain-language explanations** — "missing argument", "unexpected argument", and "expected X, found Y" type-mismatch errors (some of the most common mistakes for newer Typst users) now explain what went wrong and how to fix it, matching the treatment already given to bibliography, font, and package errors.

---

## [0.15.1-dev3] — dropcap color picker

### Added
- **Dropcap color picker** — the Droplet package settings now include a Color dropdown (Ink Black, Vermilion Red, Lapis Blue, Illuminated Gold, Verdigris Green) so the decorative first letter can match an illuminated-manuscript palette; the choice round-trips through the sidecar and is parsed back from the document's `#let dropcap = dropcap.with(...)` line

---

## [0.15.1-dev2] — LaTeX Look template, custom page settings

### Added
- **"LaTeX Look" template preset** — Computer Modern typography (body + code), tight leading and paragraph spacing, wide margins, and increased heading spacing, matching the classic LaTeX document look
- **Two new margin presets** — "LaTeX (1.75\" all)" and "Ross (1.25\" / 33% right)"; Ross's right margin is a true 1/3-page-width relative length, so it stays correct at any paper size
- **Custom page size, margins, and font size** — the Paper Size, Margins, and Font Size dropdowns in "New from Template" / "Update Template Settings" now each have a "Custom…" option revealing precise width/height (mm), margin (in), and font size (pt) fields; custom values round-trip correctly through the per-document sidecar

---

## [0.15.1-dev1] — preview word-jump, theming and UI-consistency pass

### Added
- **Double-click a word in the preview to jump to it in the source** — reads PDF word bounding boxes to find the clicked word and selects it in the editor, complementing the existing Ctrl+Click paragraph jump

### Fixed
- **Diff colors in commit history and file snapshots were unreadable in light mode** — they were hardcoded for dark backgrounds only
- **Categories/tags with no assigned color all rendered as the same hardcoded blue** — they now get a distinct, stable palette color instead
- **The preview area showed two redundant, stacked zoom/page toolbars** — removed the pane's internal duplicate controls; the floating zoom indicator now flashes for toolbar-button clicks too, not just keyboard shortcuts
- Non-resizable "New Bibliography Entry" and "Link to GitHub" dialogs had no default height and could size themselves awkwardly
- Library selection highlight was visually identical to the keyboard focus ring

### Changed
- Consolidated all static app-wide CSS, previously scattered across four files, into a single `ui/styles.rs`
- LSP status indicator and compile-mode colors now resolve live from the active theme instead of hardcoded/branched hex values
- Minor consistency polish across panels: matching page-transition animations, dialog tooltips, separators, margins, and swatch sizes

---

## [0.15.0] "Steady Hand" — citation management, data-loss fixes, correctness pass

### Added
- **Hayagriva YAML bibliography support** — `.yaml`/`.yml` bibliography files are now recognized alongside `.bib`, in the bib-file picker, project auto-detect, and citation completion/browsing (read-only for now — adding new entries still requires a `.bib` file)
- **Custom CSL style support** — a new "Custom (CSL file)" entry in the style switcher lets you point at any `.csl` file (configured in Settings) instead of being limited to the 8 built-in styles
- **Export cited-only bibliography** — new button in the reference manager exports only the `.bib` entries actually cited in the current document
- **Citation key rename** — rename a key from the reference manager and it updates the `.bib` file plus every `@key`/`#cite(<key>)` occurrence across the whole project
- **Click-to-insert in completion popups** — clicking a row in the citation (`@`) or LSP (`#`) completion popup now inserts it immediately, matching Tab/Enter

### Changed
- Replaced the hand-rolled BibTeX parser with the `biblatex` crate, fixing edge cases with `@string` macros and nested braces

### Fixed
- **Replace All in Simple Mode wiped document preamble**, permanently destroying `#set`/`#show`/bibliography declarations
- **Close-tab Save dialog saved the wrong tab**, silently losing a background tab's unsaved changes
- **Update Template Settings dialog overwrote edits made while it was open**
- **Compiler panicked on non-ASCII documents with errors** when a diagnostic span landed inside a multibyte codepoint
- **Bold/italic toggle corrupted selection on non-ASCII text** (accented letters, Cyrillic, CJK)
- **Autocorrect could corrupt text on fast typing**
- **Preview scroll signal accumulated O(N) closures across compiles**, degrading scroll performance and growing memory over long sessions
- **Upgrading users got wrong word-wrap and compile-on-save defaults**
- **Failed `git rebase --abort` was silently discarded**, leaving the repo in mid-rebase state with no indication
- **LSP reader broke on servers sending multiple headers**, silently losing diagnostics and completions
- **Preview pane jumped around while typing** — scroll-by-fraction drifted as document height changed, and cursor-to-preview sync could jump to the wrong page; sync removed, scroll-by-fraction fixed
- **Library "created" dates were always the import timestamp** — the filesystem-creation-time correction pass existed but was never invoked
- **Snapshot writes could leave a corrupt file on crash or power loss** — now staged to a temp file and renamed into place atomically
- **Switching heading style didn't apply mandated numbering** for styles that require it (IEEE, GOST 7.32, Vancouver) unless numbering was already explicitly configured

### Removed
- **Citation hover preview** — removed; it was more distracting than useful

---

## [0.14.4-dev3] — completion click-to-insert, preview jump fixes

### Added
- **Click-to-insert in completion popups** — clicking a row in the citation (`@`) or LSP (`#`) completion popup now inserts it immediately, matching Tab/Enter (previously only double-click or keyboard confirmed a selection)

### Removed
- **Citation hover preview** — removed; it was more distracting than useful

### Fixed
- **Preview pane jumped around while typing** — two causes, both fixed: (1) after every auto-recompile, scroll position was restored by *fraction*, but the document's total height changes as you type, so the viewport visibly drifted even though nothing was scrolled; (2) a cursor-to-preview sync feature scrolled the preview to match the editor cursor's heading on every keystroke, which could jump to the wrong page (e.g. the bibliography) — this sync has been removed. The preview now only moves via its own scrollbar or the page-navigation buttons.
- **Library "created" dates were always the import timestamp** — a filesystem-creation-time correction pass existed but was never invoked, so every document in the library showed its import date as "created" regardless of when the file actually existed on disk; now runs on startup after the initial library scan
- **Snapshot writes could leave a corrupt file on crash or power loss** — snapshots were written directly to their final path; a write is now staged to a temp file and renamed into place atomically
- **Switching heading style didn't apply mandated numbering** — styles that require specific heading numbering (IEEE, GOST 7.32, Vancouver) only got it if the document already had numbering explicitly configured; switching from a style with no numbering now correctly applies the mandated format

## [0.14.4-dev2] — citation management improvements

### Added
- **Hayagriva YAML bibliography support** — `.yaml`/`.yml` bibliography files are now recognized alongside `.bib`, in the bib-file picker, project auto-detect, and citation completion/browsing (read-only for now — adding new entries still requires a `.bib` file)
- **Custom CSL style support** — a new "Custom (CSL file)" entry in the style switcher lets you point at any `.csl` file (configured in Settings) instead of being limited to the 8 built-in styles; Typst's own bibliography engine renders it
- **Citation hover preview** — hovering `@key` or `<key>` inside `#cite(...)` shows the formatted citation and title in a popover
- **Export cited-only bibliography** — new button in the reference manager exports only the `.bib` entries actually cited in the current document
- **Citation key rename** — rename a key from the reference manager and it updates the `.bib` file plus every `@key`/`#cite(<key>)` occurrence across the whole project, not just open tabs

### Changed
- Replaced the hand-rolled BibTeX parser with the `biblatex` crate, fixing edge cases with `@string` macros and nested braces that the old regex-based parser could mishandle

## [0.14.4-rc1] — correctness fixes

### Fixed
- **Replace All in Simple Mode wiped document preamble** — regex/whole-word Replace All read the buffer with `include_hidden_chars=false`, which silently dropped the preamble; after the replacement the full buffer was deleted and reinserted without it, permanently destroying all `#set`, `#show`, and bibliography declarations
- **Close-tab Save dialog saved the wrong tab** — when closing a background tab via right-click → Close → Save, the Save action called `save_current()` which saved whatever tab was focused in the foreground; the background tab's unsaved changes were silently lost
- **Template dialog overwrote edits made while dialog was open** — the "Update Template Settings" dialog is non-modal; body edits typed after opening it were discarded on Apply because the apply callback used a content snapshot taken at dialog-open time rather than reading the buffer fresh
- **Compiler panicked on non-ASCII documents with errors** — diagnostic span byte offsets from Typst could land inside a multibyte UTF-8 codepoint; `&text[..offset]` then panicked with "byte index N is not a char boundary"; the offset is now clamped to the nearest char boundary before slicing
- **Bold/italic toggle corrupted selection on non-ASCII text** — Ctrl+B/I used the byte length of the selected text as a GTK character offset; any selection containing multibyte characters (accented letters, Cyrillic, CJK) landed the post-toggle selection one or more positions too far
- **Autocorrect could corrupt text on fast typing** — the idle callback for autocorrect captured `TextIter` objects which are invalidated by any subsequent buffer edit; now captures char offsets and validates the word still matches before applying
- **Preview scroll signal accumulated O(N) closures across compiles** — `connect_value_changed` was reconnected on every successful compile without disconnecting the previous handler; after a long session with auto-preview, scrolling degraded and memory grew proportionally
- **Upgrading users got wrong word-wrap and compile-on-save defaults** — `editor_word_wrap` had `#[serde(default)]` (→ false) but `Config::default()` set it to true; `compile_on_save` had the opposite mismatch; upgrading users silently got word wrap disabled and compile-on-save enabled
- **Failed git rebase --abort was silently discarded** — if `git pull --rebase` hit a conflict and `rebase --abort` also failed, the error was dropped with `let _ = ...`; the repository would remain in mid-rebase state with no indication; the failure is now surfaced in the sync error log with a manual recovery hint
- **LSP reader broke on servers sending multiple headers** — the reader consumed exactly one line after Content-Length as the blank separator; a Content-Type header between Content-Length and the blank line offset every subsequent message body, silently losing all diagnostics and completions for the session

---

## [0.14.3] "Firm Ground" — undo button reliability

### Fixed
- **Undo button greys out incorrectly with multiple tabs** — the undo/redo buttons were connected to every open buffer's `notify::can-undo` signal, so a background tab's transient state (e.g. during autocorrect or any internal GtkSourceView operation) could disable the buttons even when the active tab had a full undo history. The handlers now guard against this: notifications from background tabs are ignored, and only the currently active tab's buffer drives button sensitivity.

---

## [0.14.2] "Burnished Folio" — CV/Résumé support, library PDF export, UI polish

### Added
- **CV/Résumé templates** — three new presets in the "New from Template" dialog: Modern, Academic, and Classic, each with a fully distinct visual identity throughout
- **CV helper functions** — `#section`, `#job`, `#edu`, `#skill`, `#award` helpers embedded in every CV document; all adapt automatically to the active style
- **CV snippets** — the `#` completion popup shows CV-specific completions (`#job`, `#edu`, `#skill`, `#section`, `#award`) when editing a CV file
- **CV-aware format bar** — the academic Style button is replaced by a CV Style button when a CV file is open; one-click switching between Modern, Academic, and Classic rewrites the document's style variable in place
- **CV awards section** — `#award(title, org, years, desc: none)` helper for awards and honours entries
- **CV-aware cheatsheet** — the embedded `?` reference panel and Help popup switch to a CV helper reference when editing a CV file
- **Library PDF export** — the Export button in the library compiles the document to PDF using the embedded Typst compiler

### Fixed
- **CV style switching wiped preamble** — switching CV style via the format bar button would erase the entire document preamble; caused by reading the buffer with `include_hidden_chars=false`, which silently dropped the preamble hidden by simple mode's invisible tag
- **Cheatsheet horizontal drift** — code blocks in the help/cheatsheet panel could drift left unpredictably; disabled horizontal scrolling entirely and switched to `WordChar` wrap mode

### Changed
- **CV templates redesigned** — all three styles differ in every visual element:
  - *Modern*: 26pt letter-tracked name, accent-blue contact row and company names, colored section bars with spaced uppercase labels, two-column skills grid, 1.5pt accent rule below header
  - *Academic*: smallcaps name, two-line centered contact, smallcaps+uppercase section headers with 1pt rule, italic company names
  - *Classic*: bold-italic section headings, em dash separators in job/edu entries, italic skill categories, thin rule below header
- **Hamburger menu reorganized** — cleaner groups: New/Open (with Browse Documents), Current Document (Update Template Settings + Repair Markers), Save/Version, Export/Share, App Settings, Help; "Update Template Settings…" restored (was wired but never visible)

---

## [0.14.2-dev6] — Cheatsheet horizontal scroll fix; CV awards; hamburger menu cleanup

### Added
- **CV awards section** — `#award(title, org, years, desc: none)` helper added to generated CV templates; Awards & Honours placeholder section included; `#award` appears in the `#` completion popup when editing a CV file
- **CV-aware cheatsheet** — the embedded `?` reference panel and the Help popup both switch to a CV-specific cheatsheet (helper reference, style switching, personal details variables) when the active document is a CV
- **Hamburger menu reorganized** — cleaner groupings: New/Open together (including Browse Documents), a dedicated "Current document" group (Update Template Settings + Repair Markers), Save/Version, Export/Share, App settings, Help; "Update Template Settings…" restored to the menu (was wired but never shown)

### Fixed
- **Cheatsheet horizontal scroll** — code blocks in the cheatsheet/help panel could drift left unpredictably; disabled horizontal scrolling entirely on the panel (`PolicyType::Never`) and switched wrap mode to `WordChar` so no horizontal scroll range can exist

---

## [0.14.2-dev5] — CV/Résumé template and CV-aware editor

### Added
- **CV/Résumé templates** — three new presets in the "New from Template" dialog: CV — Modern (clean two-column layout), CV — Academic (page numbers, conservative margins), and CV — Classic (traditional single-column)
- **CV helper functions** — generated CV files include `#section`, `#job`, `#edu`, and `#skill` Typst helpers; style-conditional rendering adapts to the chosen CV_STYLE variable
- **CV snippets** — when editing a CV file, the `#` completion popup offers `#job`, `#edu`, `#skill`, and `#section` instead of the usual academic snippets
- **CV-aware format bar** — a "CV Style" button appears in the format bar when a CV file is open, replacing the academic citation options; one-click switching between Modern, Academic, and Classic rewrites the `CV_STYLE` variable in the document in-place
- **CV style persistence** — CV style is detected from the `// @zerkalo-cv-style:` comment marker embedded in generated files; label in the format bar reflects the active style

---

## [0.14.2-dev4] — Dropcap height setting

### Added
- **Dropcap height** — a "Height" selector in the template settings Droplet section lets you choose how many lines tall the drop capital should be (2–6 lines; default 3); emits `height: N` in the generated `dropcap.with(…)` call
- **Dropcap sub-panel** — the Droplet entry in template settings is now an expandable row; the Font and Height options slide open underneath it when Droplet is enabled, and collapse when disabled

### Changed
- **Dropcap font** — now a dropdown of the document's enabled fonts (plus "(use body font)") instead of a free-text entry row

---

## [0.14.2-dev3] — Dropcap font setting

### Added
- **Dropcap font** — when the Droplet package is enabled in template settings, a "Dropcap Font" entry row appears underneath it; setting a font name emits `#let dropcap = dropcap.with(font: "…")` in the template, so all `#dropcap[…]` calls in the document use that font automatically

---

## [0.14.2-dev2] — Bug fixes and format bar improvements

### Added
- **Line number toggle** — `#` button in the format bar lets users show line numbers even in Simple Mode
- **Font and size labels** — the Font and Size buttons in the format bar now display the current font name and size from the open document
- **LSP completion kinds** — completion popup now shows readable kind labels (Function, Method, Variable, Keyword, Snippet…) in caption style instead of cryptic abbreviations
- **Compile on template apply** — "Update Template Settings → Apply" now triggers a compile immediately

### Fixed
- **Snapshot restore** — the Restore button in Browse Snapshots now actually replaces the buffer content; previously it silently did nothing because the file was already open
- **Horizontal editor movement** — removed the per-frame tick callback that was fighting with GTK layout and causing the view to jitter horizontally
- **Status bar compile toggle** — the On Save / Auto / Manual toggle now uses the correct font size and weight (was previously created with `Button::with_label`, causing it to appear larger and always bold)
- **Welcome window** — release name updated to "Quiet Brass" and "What's New" bullets reflect current features

---

## [0.14.2-dev1] — More visual polish

### Added
- **Session delta color** — the session word-count counter turns green when positive
- **LSP status dot** — a single colored dot replaces the text glyph: green = ready, amber = loading/indexing, red = error, grey = unknown
- **Find bar match size** — the "N of M" counter in the find bar now renders at caption size
- **Format bar fade** — the format bar fades in on text selection (120ms opacity transition)
- **Error banner shake** — when a new compile error fires with the banner already visible, it shakes briefly to signal updated content
- **Preview page indicator** — "Page X / Y" label in the preview bottom bar, hidden when single-page
- **Breadcrumb bar class** — breadcrumb bar receives CSS class for future styling
- **Tag chip click-filter** — clicking a tag chip in the doc list sets the sidebar filter to that tag and selects the matching sidebar row; brief accent flash confirms the action
- **Command palette match highlighting** — matched query characters are bolded in palette results and subtitles
- **Tab drag CSS** — the dragged tab gets `opacity: 0.7` + accent background during reorder

---

## [0.14.1] "Quiet Brass" — Library status bar, compile mode toggle, visual polish

### Added
- **Library status bar** — doc/project count and last-opened info moved from sidebar to a bottom status bar on the right panel; "Compact" text button (bold when active) also lives there
- **Compile mode status bar toggle** — click "on save / auto / manual" in the editor status bar to cycle compile modes; label turns amber in manual, green in auto; syncs with Settings
- **Sidebar count badges** — each filter row in the library sidebar shows a pill badge with the matching document count; hidden when zero
- **Doc list empty state** — an empty filter or search now shows a "No documents / Nothing here yet" centred placeholder instead of a blank list; subtitle updates to "Try a different search" when searching
- **Pinned docs divider** — a separator row appears between pinned and unpinned documents in the library list
- **Compile pulse animation** — the compile button pulses its opacity during compilation to indicate active work
- **Preview zoom step buttons** — `−` and `+` flank the zoom percentage label in the preview bottom bar for mouse-free zoom
- **Goal ring celebration** — when the word count crosses the goal threshold, the ring flashes bright green with a thicker stroke for 900 ms
- **Tab overflow fade** — a right-edge gradient on the tab header hints at scrollable overflow
- **CSS: compile mode colour** — "manual" label is amber, "auto" is success-green

### Changed
- **Tab transitions** — extended to `all 150 ms ease` on the notebook stack child

---

## [0.14.1-dev3] — Visual polish and save behaviour fixes

### Added
- **Preview zoom OSD** — a percentage chip fades in near the preview bottom-right on zoom in/out/reset, disappearing after 1.5 s
- **Word count goal ring** — the flat progress bar is replaced by a small circular Cairo arc; teal while in progress, green at 100 %
- **Typewriter scroll crosshair** — a faint 1 px horizontal line sits at the 45 % typewriter anchor while typing, fading 800 ms after keystrokes stop
- **Compile error line gutter** — lines containing compile errors receive a faint red paragraph background; cleared on next successful compile
- **Status bar micro-separators** — thin `│` glyphs divide the word-count, session-delta, and version segments in the status bar

### Changed
- **Tab transitions** — switching notebook pages now fades at 120 ms ease instead of snapping
- **Tab hover** — inactive tabs show a subtle background tint on hover
- **Sidebar pane handle** — hover accent is more visible (0.45 alpha) with a slightly wider handle
- **Find bar reveal** — transition extended to 250 ms for a smoother slide
- **Library card selection** — selected cards show a 2 px accent `outline` ring in addition to the background tint
- **Default spell language** — new installs default to `en_CA` instead of `en_US`

### Fixed
- **Unsaved dot clears after git push and auto-save** — `save_all_modified` now fires the title-bar modified callback so the `·` disappears correctly
- **Compile saves first** — the Preview toggle and Recompile button now call `save_current()` before triggering compilation

---

## [0.14.1-dev2] — Editor polish and library improvements

### Added
- **Paragraph focus mode** — new menu item (Ctrl+Shift+D) dims all paragraphs except the one containing the cursor, reducing visual noise during writing
- **User-defined snippets** — Settings → Editor → Snippets tab; add trigger/body pairs that expand on Tab keypress in the editor
- **Compile status shows timing and page count** — status bar now reads "Compiled in X.Xs · N pages" on success, or "Error · X.Xs" on failure; shows "Compiling…" while in progress
- **Title dirty indicator** — window title gains a bullet (·) when the active file has unsaved changes; cleared on save
- **Word count goal** — Settings → Editor → Word Count Goal; non-zero value shows a progress bar in the editor status bar
- **Recent project word count** — library project view header shows total document count and word count for the project
- **Library empty state** — library shows a proper empty-state page when no documents exist or a search returns no results

### Fixed
- **Style/font/size changes now trigger recompile** — clicking a style button, changing the font family, or adjusting font size from the toolbar now correctly recompiles the preview; previously the snapshot could be stale causing the old version to compile
- **High-contrast mode respects light/dark theme** — high-contrast now applies the correct CSS class for the current color scheme; previously switching themes while high-contrast was enabled could show white-on-white text in light mode
- **Library file creation dates** — documents newly added to the library now record the filesystem creation time rather than the import time; a one-time migration corrects existing records
- **Citation tag format** — "Tag from citation" now generates `Lastname, Firstname` format for all BibTeX author name styles

---

## [0.14.1-dev1] — Reliability and startup polish

### Fixed
- **Undo stack no longer wiped by style changes** — applying a citation style, updating template settings when no body marker exists, or accepting autosave recovery no longer silently destroys the undo history; each operation is now one undoable step (Ctrl+Z takes you back to before it)
- **"File reloaded" toast** — when an external change triggers a file reload (file watcher or git sync), a toast notifies that undo history was cleared rather than leaving Ctrl+Z silently broken
- **Ctrl+Y as redo alias** — Ctrl+Y now redoes alongside the existing Ctrl+Shift+Z
- **Atomic autosave writes** — autosave now writes to a `.tmp` file then renames atomically; a crash mid-write can no longer corrupt the previous good autosave
- **Stale autosaves cleared after manual save** — manually saving (Ctrl+S or Save All) now deletes the corresponding autosave entry, preventing false recovery offers on next launch
- **Recovery dialogs serialized** — if multiple tabs have pending recovery on session restore, dialogs now appear one at a time rather than all at once

### Performance
- **Library DB loaded off the main thread** — opening and scanning the library database no longer blocks the GTK main thread on startup; the window now appears immediately and the library is available in the background within a few hundred milliseconds
- **Typewriter scroll debounced** — rapid line crossings (holding Enter, pasting) no longer trigger multiple recenters; the view settles once after 80 ms of no new line changes
- **Heading-based preview sync debounced** — the preview no longer jumps the instant the cursor crosses a section boundary mid-edit; it waits 200 ms for the cursor to settle before scrolling

### UX
- **Tab switching lands focus in the editor** — switching tabs with the keyboard shortcut now moves focus to the new tab's text view; previously required a mouse click to start typing
- **Tab exits the file sidebar** — pressing Tab while the file tree has focus now jumps directly to the editor instead of cycling through toolbar buttons
- **Command palette returns focus on close** — closing the palette (Escape or selecting a command) now returns keyboard focus to the editor
- **Find bar Escape returns focus** — pressing Escape to close the find bar now returns focus to the editor text view

### Fixed
- **Tab close prompts to save unsaved changes** — closing a modified tab now shows a Save / Discard / Cancel dialog instead of silently discarding changes; applies to the X button, middle-click, and right-click "Close tab"
- **File watcher deduplication** — rapid back-to-back external writes to the same file no longer trigger multiple recompiles within the same 250 ms window
- **Background tabs no longer trigger spurious recompile** — the file watcher's "is this file open?" guard now checks all open tabs, not just the active one
- **Git sync conflict shows a clear message** — a merge conflict during pull now shows "Merge conflict — sync aborted" with guidance, instead of raw git error output
- **Work folder change now shows restart notice** — changing the work directory in Settings now tells the user a restart is needed for the change to take effect

---

## [0.14.0] "Lucid Archive" — Library search, hierarchy, and polish

### Added
- **Search across tags and categories** — the search bar in the library now matches tag names and category names in addition to document titles, across every filter
- **New Category button** — sidebar now has a direct "New Category" button (with colour picker) alongside "New Project" and "Manage Tags"; no longer requires opening a document first
- **Two-rank category hierarchy** — categories can now have a parent, enabling groupings like Semester → Class; create subcategories via right-click "Add Subcategory…"; parent categories show a combined doc count across children
- **Set Parent** — move a leaf/standalone category under a parent via right-click "Set Parent…"
- **Category group filter** — clicking a parent category in the sidebar filters to all documents in that category and its children
- **Notes visible in library** — card view shows the first line of a document's notes below the tags; compact view shows the full notes text as a tooltip on hover

### Changed
- **Bulk tagging is now additive** — "Tag…" in the bulk action bar adds the selected tags to each document without erasing tags they already have
- **Delete blocked on parent categories** — deleting a category with subcategories is blocked (button dimmed with tooltip); remove subcategories first
- **Drag-to-parent blocked** — dragging a document onto a parent category shows a toast; drop onto a specific subcategory instead

### Fixed
- **Saving a file not yet in the library now registers it** — auto-save previously did nothing for files that hadn't been added to the library; it now upserts the document first
- **Recently Opened badge matches the list** — the count badge now uses the same LIMIT 30 as the list, so the number is never larger than what's shown
- **Old categories visible in sidebar** — categories assigned before the categories table was introduced now appear correctly in the sidebar after migration
- **Title extraction could abort early** — a `?` in the `#let doc-title` parsing path caused the whole function to return `None` if the `=` was formatted unexpectedly; both title paths now use identical logic
- **App now quits cleanly when library is open** — the library window was an `adw::ApplicationWindow`, keeping the process alive after the main window closed; changed to `adw::Window`

### Performance
- **Database indexes** — added indexes on category, archived/deleted, last_opened_at, modified_at, and both sides of the tag join table; filter switches are faster on large libraries

## [0.13.12] "Amber Index" — Document Library

### Added

- **Document Library** (`Ctrl+L` or Library button in toolbar) — a floating window that serves as the primary document navigator, backed by SQLite at `~/.local/share/zerkalo/library.sqlite`
- **Filter sidebar** — All Documents, Recently Opened, Untagged, Projects, Categories, Tags; Trash and Archive pinned below the scrollable list and always visible
- **Card view and compact view** — toggle between pleasant card layout (title, chips, word count, date) and a single-line compact layout; word count excludes Typst commands, code blocks, and citation tokens
- **Tags** — create tags with 8 preset colours; tag chips on every card use heat colouring (red = most used, yellow = middle, blue = least used); manage and rename tags in Manage Tags
- **BibTeX author import** — import author last names as tags from a `.bib` file; filtered to entries actually cited in the current document
- **Projects** — group documents into named projects; drag to reorder within a project; right-click to rename or delete
- **Categories** — assign a category and colour to any document; drag a document onto a category in the sidebar to assign it; right-click to rename or delete a category
- **Document templates** — "New Document" picks from `.typ` files in `Templates/` in your work dir; falls back to a blank file if none exist
- **Pinned documents** — "Pin to Top" in the context menu; pinned docs sort to the top with a left accent border
- **Trash / soft delete** — "Move to Trash" sends a file to `~/.local/share/zerkalo/trash/`; Trash filter shows trashed docs with "Restore" and "Permanently Delete…" options
- **Archive** — flag-only archiving; archived docs hidden from All and shown under Archive
- **Bulk operations** — Ctrl+click multi-select; action bar slides up with Archive / Tag / Add to Project / Remove
- **Document titles from source** — library titles default to `#let doc-title = "…"` in the file; fall back to filename
- **Sort control** — Modified / Created / Opened / A→Z dropdown
- **Statistics bar** — total doc count, project count, and last-opened document
- **Export** — "Export…" in the doc context menu copies the file anywhere via a save dialog
- **Auto-registration** — files opened in the editor and `.typ` files added externally are registered automatically
- **Notes field** — "Edit Notes…" in the doc context menu stores a multiline note per document

### Fixed

- **Crash when creating a tag** — borrow conflict in `populate_filter_list()` after `borrow_mut()` in the same expression; fixed by dropping the mutable borrow before re-borrowing
- **BibTeX author parser** — fixed trim order (comma before brace), BibLaTeX extended name format (`family=…`), and double-braced organisation names; no longer matches `authorrunning` as the author field

---

## [0.13.12-dev3] — Fix scroll jump after right-click context menu

### Fixed

- **Editor scrolled back to the selected paragraph after dismissing a right-click context menu** — two root causes: (1) `saved_scroll` was not updated when the mouse left the text view after scrolling, so right-clicking elsewhere after a mouse-wheel scroll restored the stale pre-scroll position; (2) GTK's focus-in `scroll_to_mark` snapped to the old cursor position (the selected paragraph) because the cursor wasn't moved on right-click. Fixed by adding `ptr_ctrl.connect_leave` so `saved_scroll` tracks mouse-wheel scrolling, and by placing the cursor at the right-click position (if not inside the active selection) before the context menu opens, making the focus-in snap a no-op.

---

## [0.13.12-dev2] — Fix context menu scroll jump

### Fixed

- **Right-clicking in the editor and clicking away to dismiss the context menu caused the view to jump** — `focus_ctrl.connect_enter` was reading the current scroll position at the moment focus returned, which could already be GTK's snapped (cursor-aligned) value. Fixed by reading from `saved_scroll`/`saved_hscroll` (set by `connect_leave` and the right-click gesture's idle callback) so the pre-right-click position is always what gets restored.

---

## [0.13.12-dev1] — Fix spell check freeze

### Fixed

- **Clicking "Ignore All", "Add to Dictionary", or "Add to Project Dictionary" in the spell check popover froze the UI** — the tag-removal loop scanned every character in the buffer one at a time. Replaced with `forward_to_tag_toggle` iteration, which skips directly between tagged ranges and is O(k) in the number of misspelled-word markers rather than O(N) in buffer length.

---

## [0.13.11] "Steady Gaze" — Fix horizontal view snap on click

### Fixed

- **Clicking in the editor snaps text against the left edge** (simple mode) or **hides text under line numbers** (regular mode) — GtkSourceView5 maintains its own internal hadjustment (separate from the `ScrolledWindow`'s hadjustment) and sets it to exactly `left_margin` on every cursor movement, scrolling the left margin off screen. Diagnosed via a per-frame `add_tick_callback` that reads `view.visible_rect()` directly (bypassing GTK signal timing). Fixed by detecting `visible_rect.x == left_margin` in the tick and resetting the view's hadjustment to 0 before the frame's layout+draw pass, so the user never sees the snapped position.
- **Typewriter scroll snapping text to the left edge** — `scroll_to_iter` with `xalign=0.0` was placing the cursor at the left edge of the viewport on every line change while typing. Fixed by saving and restoring the horizontal adjustment around the call, so typewriter scroll only moves vertically.

---

## [0.13.11-dev6] — Fix crash when changing style twice (spell poll timer SourceId)

### Fixed

- **Crash (panic "Failed to remove source") when changing style a second time** — the spell-check poll timer uses `glib::timeout_add_local`, which auto-removes the GLib source when the callback returns `ControlFlow::Break`. The `spell_poll_timer` RefCell still held the now-dead `SourceId`. The next `connect_changed` (triggered by the second style change) called `id.remove()` on it, which panicked because the source no longer existed. Fixed by clearing `*pt2.borrow_mut() = None` inside the poll callback before returning `Break`, mirroring the pattern used by all the debounce timers.

---

## [0.13.11-dev5] — Fix borrow held across spell-tag removal in set_spell_enabled

### Fixed

- **Borrow violation in `set_spell_enabled`** — when disabling spell check, `self.state.borrow()` was held while calling `clear_spell_tags()` on each tab's buffer. `clear_spell_tags` calls `remove_tag_by_name`, which fires the `tag-removed` signal and can cascade through GtkSourceView's internal signal handlers back into Zerkalo code that needs a conflicting borrow. Fixed by collecting the buffer list in a scoped block and releasing the `state` borrow before any GTK tag operations.

---

## [0.13.11-dev4] — Fix crash when changing style (borrow held across GTK tag ops)

### Fixed

- **Crash (SIGABRT) when changing the editor style** — three separate borrows were held across GTK operations that cascade through GtkSourceView signals:
  - `apply_simple_mode_tag(&buffer, *self.simple_mode.borrow())` — the `Ref<bool>` temporary lives until the end of the statement (after `apply_simple_mode_tag` returns), spanning `buffer.remove_tag()` / `buffer.apply_tag()` calls that emit signals. Fixed at 8 call sites by extracting the bool before the call: `{ let sm = *self.simple_mode.borrow(); apply_simple_mode_tag(&buffer, sm); }`.
  - `spell_checker` borrow held in `connect_changed` closure while calling `clear_spell_tags()` → `buffer.remove_tag_by_name()`. Fixed by scoping the borrow to extract `enabled: bool`, releasing it before the tag removal call.
  - `self.entries.borrow()` in `BibPopup::show_filtered` held across `append_row`, `select_row`, and `popover.popup()`. Fixed by collecting matched entries into an owned `Vec<BibEntry>` inside a scoped block, releasing the borrow before all GTK widget operations.

---

## [0.13.11-dev3] — Fix crash in LSP poll timer (borrow held across GTK ops)

### Fixed

- **Crash (SIGABRT) when typing while tinymist is running** — the 400ms LSP poll timer held `lsp_client.borrow()` across calls to `mark_diagnostics()` and `show_lsp_completions()`. Both functions call GTK buffer operations (`create_source_mark`, `popover.popup`) that cascade through GtkSourceView signals and re-enter Zerkalo callbacks that try to borrow `lsp_client` again → `BorrowError` panic. Fixed by extracting all LSP data (`poll()`, `poll_completion()`) in a scoped borrow block, releasing the borrow, then performing all GTK operations.

---

## [0.13.11-dev2] — Fix crash when LSP completion popup appears during typing

### Fixed

- **Crash (SIGABRT) when typing in the code editor** — `show_lsp_completions` held `state.borrow()` while calling `lsp_popup.show_items(...)`, which calls `popover.popup()`. That popup show triggers GTK layout, which cascades through GtkSourceView signals and re-enters a Zerkalo handler that tries a conflicting borrow → `BorrowError` panic. The crash manifested when tinymist responded with completions during or shortly after typing. Fixed by collecting all tab data in a scoped borrow block, releasing the borrow, then performing all GTK popup operations.

---

## [0.13.11-dev1] — Professor/course fields, more crash fixes

### Added

- **Professor / Instructor field on the title page** — new field in the template dialog (under Course Code). Persisted in the `.zerkalo.toml` sidecar. Rendered on title pages for all academic styles (MLA, APA, ASA, Harvard, Chicago, Turabian, SBL, Default).

### Fixed

- **Crash (SIGABRT) when editing a freshly created template file** — the undo/redo toolbar buttons held `state.borrow()` while calling `tab.buffer.undo()` / `tab.buffer.redo()`. Those calls fire `changed`, which cascades through GtkSourceView (`source-mark-updated`) and re-enters Zerkalo handlers that try a conflicting borrow → `BorrowError` panic. Fixed by cloning the buffer handle out of the state borrow scope, then calling `undo()`/`redo()` after the borrow is released.
- **Same pattern eliminated** from `apply_simple_mode_to_buffer`, `apply_word_wrap`, `apply_show_whitespace`, `apply_tab_width`, `apply_line_spacing`, and `apply_style_scheme` — all previously held `state.borrow()` while calling GTK buffer/view ops that can emit signals.

---

## [0.13.10] "Steady Quill" — Crash fixes and faster dev builds

### Fixed

- **Crash (SIGABRT) when typing** — two separate RefCell double-borrow crashes eliminated. The `connect_changed` handler, `mark_saved`, and `save_all_modified` called `set_visible` / `update_property` while holding `state.borrow_mut()`; `mark_diagnostics` and `clear_diagnostic_marks` called GTK buffer ops while holding `state.borrow()`. In both cases, GTK fires synchronous signals that cascade through GtkSourceView and re-enter Zerkalo callbacks that try a conflicting borrow, causing a `BorrowError` panic. All fixed by releasing the borrow before any GTK calls.
- **Clicking to dismiss the spell popover no longer jumps the document** — fixed a race between GTK's focus-leave handler and the idle-based scroll restore that was saving and restoring the wrong scroll position.

### Internal

- Flatpak manifest split into `zerkalo-deps` (all cargo deps, cached) and `zerkalo` (app crate only). Dev builds now recompile only the `zerkalo` crate (~30 s vs ~3 min) when `Cargo.lock` is unchanged.

## [0.13.10-dev8] — Fix scroll jump when dismissing spell popover (root cause)

### Fixed

- **Clicking to dismiss the spell popover no longer jumps the document** — the root cause was a race between GTK's focus-leave handler and our idle-based scroll restore. When right-clicking, GTK's button-3 handler snaps the scroll to the cursor before our idle can restore it. `focus_ctrl.connect_leave` fired during that window and saved the snapped (wrong) position into `saved_scroll`. When the popover was dismissed and the view regained focus, `focus_ctrl.connect_enter` restored from that wrong position, causing the jump. The fix: the right-click idle that restores scroll now also writes the correct position back into `saved_scroll`, so `focus_ctrl.connect_enter` always has the right value regardless of when `focus_leave` fired.

## [0.13.10-dev7] — Fix typing delays: move spell check off main thread, cancel stale timers

### Fixed

- **Typing no longer stalls or feels choppy** — the debounced spell check was calling `wait_with_output()` (synchronous hunspell subprocess) directly on the GTK main thread, blocking the event loop for 50–200 ms every 700 ms. The spell check now runs in a background thread (same pattern as `recheck_all_buffers`); the main thread only does a non-blocking `try_recv()` poll every 50 ms.
- **Timer accumulation eliminated** — the word count (300 ms), project word count (5 s), comment highlight (500 ms), and spell check (700 ms) debounce timers each now cancel the previous pending timer before adding a new one (`SourceId.remove()`). Previously, every keystroke added a new timer to the GLib event loop without removing the old ones, so after rapid typing thousands of pending timers had to be iterated on every event loop tick.

## [0.13.10-dev6] — Fix spell menu not appearing (move to connect_pressed)

### Fixed

- **Spell correction popover now reliably appears on right-click** — GtkSourceView processes button-3 internally and may grab the pointer before the release event reaches our gesture, so `connect_released` was silently never firing. The entire spell menu handler is now in `connect_pressed`, which always fires before any widget-level event handling.

## [0.13.10-dev5] — Fix scroll jump when dismissing spell popover

### Fixed

- **Clicking outside the spell popover no longer jumps the editor** — the `connect_closed` handler was forcing the scroll back to the right-click position after dismissal, which conflicted with wherever the user clicked. Popover close now only unparents; scroll position is not touched.
- **Left-click navigation no longer suppressed** — the idle scroll restore is now only queued when the editor is actually gaining focus (no-focus → focused). When the view already has focus, left-click is intentional navigation and the view scrolls to the cursor normally.

## [0.13.10-dev4] — Fix spell correction popover blocked by gesture conflict

### Fixed

- **Spell correction popover now appears on right-click** — the `GestureClick(button=0)` gesture added for scroll-snap suppression was stealing the button-3 sequence (GTK4 processes last-added controllers first), preventing the spell menu gesture's `connect_released` from ever firing. Changed to `button=1` (left-click only) so the right-click spell gesture retains ownership of button-3 sequences.
- **Right-click scroll snap suppression** — the idle scroll restore is now queued directly in the right-click gesture's `connect_pressed`, so right-clicks also suppress the viewport snap even when the editor already has focus.

## [0.13.10-dev3] — Fix right-click scroll jump (already-focused case) and popover position

### Fixed

- **Right-click no longer jumps to the top of the document** — the previous fix only suppressed the snap when the view gained focus; if the view already had focus, GTK's right-click handling could still call `scroll_mark_onscreen` without firing a focus-in event. Every button press now queues an idle scroll restoration unconditionally.
- **Spell correction popover appears at the click position** — the popover rect now uses the viewport-relative coordinates captured at press time rather than `buffer_to_window_coords`, which was computed after the scroll had already snapped and therefore gave the wrong position.

## [0.13.10-dev2] — Fix preview flicker, right-click scroll jump, and spell menu

### Fixed

- **Preview pane shadow no longer flickers** — the auto-fit path now computes the correct zoom before the single redraw instead of doing an intermediate render at the old zoom level followed by a second render with the corrected zoom.
- **Right-clicking in the code area no longer jumps to the top of the document** — the spell-correction popover handler now captures the buffer position and scroll offset at button-press time (before GTK's focus-in handler can snap the viewport to the cursor), so right-clicking anywhere in the document no longer causes the viewport to snap.
- **Spell correction popover now appears at the correct word** — previously, the popover's position and word lookup used the widget coordinates from `connect_released`, which were wrong after the focus-snap; both now use coordinates captured at `connect_pressed`.
- **Mouse text selection jumps reduced further** — `saved_scroll` is now updated on every button press (via a `GestureClick` with button=0), not just on pointer-enter. This ensures the scroll position used for the focus-snap restoration is always current, even when the user scrolls within the editor between enters.

## [0.13.10-dev1] — Fix export log output (capture stdout alongside stderr)

### Fixed

- **Export log now shows the actual error** — `run_command_logged` previously only captured stderr; pandoc (and its Typst subprocess) may write error messages to stdout. Both streams are now captured concurrently so the log pane always reveals what went wrong.

---

## [0.13.9] "True Shore" — Mouse selection, word count, export, and paragraph spacing

### Added

- **Running header** — new "Running Header" row in the template dialog Layout tab. Options: None, Title, Author, Current section (auto-matched to the nearest H1), Title · Author, Title · Section, Author · Section, Author · Title. Saved in the sidecar and restored when reopening template settings.

### Fixed

- **Mouse text selection no longer causes wild viewport jumps** — GTK's built-in focus-in behaviour calls `scroll_mark_onscreen(insert)` when the text view gains keyboard focus, snapping the viewport to the cursor's old position even when the user had scrolled elsewhere. The scroll position is now saved when the pointer enters the editor and restored (via idle) after the focus-in snap fires, so the view stays where the user was reading. Covers both the first-click case and re-entry after clicking away.
- **Scroll margin idle no longer disrupts drag selection** — the cursor-follow idle (keyboard typing only) is skipped when a selection is active or when the cursor is more than one viewport height outside the visible area.
- **Status bar no longer shows "N words selected" when nothing is selected** — when clicking to deselect, GTK moves the `insert` mark before `selection_bound`, creating a momentary ghost selection; a second `connect_mark_set` handler now fires when `selection_bound` catches up and clears the label.
- **Word count (status bar and document statistics) no longer inflates with Typst code, citations, or bibliography** — ZERKALO-STYLE and ZERKALO-TEMPLATE blocks are stripped before counting; structural directives (`#set`, `#show`, `#let`, `#import`, `#include`) are skipped to end-of-line. Document statistics window now uses the same clean count for words, characters, paragraphs, sentences, and reading time.
- **HTML, DOCX, ODT, LaTeX, and EPUB export no longer fail** — heading show rules injected by the template dialog used `it.numbering`, which Typst's non-PDF export pipeline does not support. Counter display is now injected only when heading numbering is enabled, using the format string directly. Existing documents are migrated automatically on open and patched transparently when exported via pandoc.
- **Uniform paragraph spacing in all double-spaced styles** — Chicago, SBL, Turabian, Harvard, MLA, APA, ASA now set `spacing: 1em` to match `leading: 1em`, giving true uniform double-spacing between paragraphs.

---

## [0.13.8] "Still Water" — Template dialog redesign, preview startup fix, release notes

### Fixed

- **Preview always matches the editor on startup** — removed the conflicting second restore path in `build()`; `open_initial_file()` now owns the full session restore, and a deferred `idle_add_local_once` pins the preview to the active editor file after the event loop starts.
- **Release notes window too wide** — title/heading labels use `set_ellipsize(End)` (their minimum width was propagating through `adw::Clamp` and forcing the window wider); bullet labels use `WrapMode::WordChar` and `Align::Fill`; scroll has `hscrollbar_policy(Never)`.
- **Drafting package compile errors** — `@preview/drafting:0.2.2` removed from Packages tab; conflicts with Typst 0.14 `*` imports.
- **Project root used at startup when toggle was off** — `on_page_switch` now gates `configured_root` behind `proj_mode_active`; root from a previous session can no longer hijack the preview.

### Changed

- **Template dialog redesigned** — tab sidebar runs the full window height with no content above it; "Template" is the first tab and contains the starting-template gallery; Quick Settings removed; dialog default width doubled to 1240 px.

---

## [0.13.8-rc4] — Release notes width (partial fix)

### Fixed

- **Release notes window too wide** — wrapped the body in `adw::Clamp` with `maximum_size = 460`. GTK4 labels with `set_wrap(true)` still request their full natural (unwrapped) width, which forces the window to expand; Clamp caps the natural-width request so labels wrap within the window rather than pushing it wider.

---

## [0.13.8-rc3] — Preview startup fix (definitive); release notes wrapping

### Fixed

- **Preview startup mismatch** — removed the conflicting `build()` restore path that was opening `recent_files[0]` separately from the session restore. `open_initial_file()` now handles the sole restore; after the event loop starts, an `idle_add_local_once` explicitly sets the preview root to whatever file the editor is showing and triggers compile. This fires last, after all `on_page_switch` compiles, guaranteeing the preview always matches the editor.
- **Release notes text too wide** — shortcut key column reduced from 22 to 16 chars; description column now wraps with `WrapMode::WordChar` and fills the row width.

---

## [0.13.8-rc2] — Template dialog redesign; preview startup fix; release notes width

### Fixed

- **Preview shows wrong file on startup** — project mode is now off by default; `on_page_switch` only uses the configured root file when the project toggle is actively ON. Previously the root from a prior session would cause the preview to show a stale file even though the editor opened a fresh one.
- **Release notes text too wide** — welcome window reduced to 460 px; all body labels now wrap with `WrapMode::WordChar`; `hscrollbar_policy` is `Never`.

### Changed

- **Template dialog fully redesigned** — the tab sidebar now runs the full window height with no content above it. "Template" is the first tab and contains the starting-template gallery (preset list + live preview). Quick Settings panel removed. All five settings tabs (Document, Layout, Sections, Languages, Packages) remain.
- **Drafting package removed from Packages tab** — `@preview/drafting:0.2.2` exports conflicting names in Typst 0.14. Removed until a compatible version is available.

---

## [0.13.7] — Project controls, cursor sync, build log, Simple Mode polish

### Added

- **Project toggle in status bar** — a "project" toggle (default OFF, left of the SIMPLE button) reveals inline root-file controls: current root filename, "Set…" picker, and "✕" clear. Root controls no longer require the file-tree right-click menu.
- **Bib file picker** — folder-open button in the Citations sidebar header to select a `.bib` file directly; active filename shown in the header.
- **Reverse sync** — moving the cursor in the editor scrolls the preview proportionally to the cursor position (debounced 300 ms).
- **Compile elapsed timer** — the spinner shows "Compiling… Ns" updating every 500 ms while the compiler runs.
- **Build Log panel** — a collapsible section below the error panel shows raw Typst stderr on compile failure.
- **Compile status label** — shows "✓ N pages · X.Xs" on success and "✗ X.Xs" on error.
- **Help toggle** — the preview toolbar now has a labelled "Help" toggle instead of an icon-only `?` button.

### Changed

- **Simple Mode** — the line-number gutter is hidden; text gains a 40 px left margin; the `// ── Document body` marker comments are now hidden alongside the preamble; the format bar auto-enables when Simple Mode is activated.
- **Root-file suggestion banner** — the "main.typ detected — set it as root?" banner above the preview only appears when the project toggle is ON, never on startup.
- **Draft/Final toggle hidden** — removed from the status bar for now (compile-profile functionality preserved internally).
- **Word wrap button** — removed from the breadcrumb toolbar; still configurable in Settings.

### Fixed

- **Crash on cursor move** — `SourceId::remove()` panics in glib 0.18 when the debounce timer has already auto-fired; replaced with generation counter pattern.
- **Right-click no longer jumps to top** — spell-check context menu no longer resets scroll position.
- **What's New text wrapping** — release notes window no longer scrolls horizontally.

---

## [0.13.6] — Search toggle, font dropdown, preview stability

### Added

- **Compile button in header** — new refresh icon button beside the Preview toggle triggers an immediate compile without toggling preview visibility.

### Changed

- **Auto compile is now the default** — new installs start in Auto mode (reverted from manual-only introduced in rc9).
- **Search toggle now bold when active** — the "search" button in the status bar uses bold text when the find bar is open, matching the other toggle buttons; no longer invisible when off.
- **Font dropdown wider** — minimum width increased to 260 px with natural-width propagation so font names are never truncated.

### Fixed

- **Preview scroll jumping** — eliminated a double-scroll-restore race in auto-fit mode that caused the preview to jump to a different position after each recompile.
- **31 compiler warnings resolved** — `#[allow(dead_code)]` / `#[allow(deprecated)]` attributes placed on scaffolded APIs and the GTK 4.10 colour lookup call.

---

## [0.13.6-rc9] — Performance, format bar, and colour polish

### Added

- **Font dropdown uses Font Management list** — the format bar font popover now shows only the fonts enabled in the Font Management dialog, wrapped in a scrollable list.
- **More font sizes** — 18pt, 20pt, and 24pt added to the size dropdown.
- **Table grid cells now visible** — cells in the insert-table picker have a border and background; hovering highlights the full selected range in accent colour.
- **Custom table size input** — a rows × cols entry with Insert button below the grid allows inserting tables of any size.

### Changed

- **Manual compile default** — `manual_compile_only` now defaults to `true`; auto-compile is disabled out of the box for a snappier editing experience.
- **Word count debounced** — word count label updates at 300 ms after the last keystroke instead of on every change; project word count tooltip updates at 5 s (it was reading all project files from disk on every keystroke).
- **Comment highlights debounced** — full-buffer comment scan runs 500 ms after the last change instead of every idle frame.
- **LSP status dot colours theme-aware** — green/red dots adapt to light vs. dark mode instead of using fixed dark-mode hex codes.
- **Comment highlight colour follows accent** — comment paragraph background uses the user's chosen accent colour instead of a hardcoded blue.
- **Current-line highlight** — alpha raised from 0.06 → 0.10 for better visibility.
- **Syntax highlighting scheme** — `kate` (light) and `monokai-extended` (dark) are tried first before Tango/Adwaita, giving better keyword contrast in most themes.
- **Typst citation/label markup colour** — `@citations` and `<labels>` now map to `def:preprocessor` for a more distinctive colour in GtkSourceView themes.

---

## [0.13.6-rc8] — Warning cleanup

### Fixed

- Removed unused `bracket_depth` closure left over from an earlier version of the template body-splice function.
- Removed dead grouping loop (`groups`, `last_file`) from the error panel that was superseded by the single-pass file-header approach.
- Dropped redundant `search_entry` and `search_text` struct fields from `ErrorPanel`; the values are kept alive by the GTK widget hierarchy and closure captures respectively.

---

## [0.13.6-rc7] — Error panel improvements 2

### Added

- **Error panel search filter** — a filter entry above the error list narrows rows in real time by matching against the error message text.
- **Error deduplication** — identical errors (same file, line, and first message line) are collapsed to a single row, eliminating noise from repeated includes.
- **Source context snippet** — each error row shows the offending source line in a monospace dim label so the problem is visible without switching to the editor.
- **File group headers** — when errors span multiple files, a dim filename label separates each file's group, making multi-file projects easier to navigate.
- **Last clean compile timestamp** — when errors clear, a dim "Last clean compile: HH:MM" note appears at the bottom of the panel so the user knows how long they were broken.
- **Save error log button** — a save icon in the panel header writes the current error log to `~/.local/share/zerkalo/error_log.txt` and confirms with a toast.
- **Ctrl+E shortcut** — focuses the first visible error row in the panel and selects it; makes keyboard-only navigation to errors practical.
- **Export-done callback** — app_window shows a toast with the exact saved path after the error log is exported.

### Changed

- **Search bar above the list** — replaces the previous header-only layout; the filter applies immediately as you type.

---

## [0.13.6-rc6] — Error panel improvements

### Added

- **Error breakdown in header** — the error panel header now reads "Compile Errors — 2 errors, 1 warning" instead of a bare count, separating compile from LSP diagnostic sections.
- **Collapsible error list** — a chevron button in the error panel header collapses the list while keeping the header and count visible; expands automatically when new errors arrive.
- **Copy button per error row** — each error row has a clipboard icon that copies the full message and location text to the clipboard.
- **Jump-to-error button per row** — a go-to icon button makes the jump action explicit and discoverable alongside the existing row-activation shortcut (Enter key).
- **Keyboard navigation in error list** — selection mode changed to Browse so rows receive keyboard focus and can be activated with the Enter key.
- **Try-Fix button for syntax errors** — "unexpected end of file" errors show a "Fix" button that counts unmatched brackets/braces and appends the missing closers as a single undoable edit.
- **"Stuck?" trend indicator** — after three consecutive compiles with the same error, the header shows "Stuck?" with a tooltip of common remediation steps.
- **Error count in window title** — the application window title shows "(N errors, M warnings)" while compile errors are present; clears on successful compile.
- **Compile vs LSP section labels** — compile errors use the header "Compile Errors"; LSP diagnostics use "Diagnostics", so the two are never conflated.

### Changed

- **Success toast on error recovery only** — the "Compiled successfully" toast is now suppressed for routine clean compiles; it appears only when recovering from a previous error state, reducing notification noise.
- **Enrichment hints shown inline** — the first line of an enriched error is the title; additional hint lines appear below it in a smaller dim style, replacing the previous single-label approach.

---

## [0.13.6-rc5] — Accessibility pass 2

### Added

- **Focus trap in format bar popovers** — table, font, and size popovers now call `grab_focus()` on open so keyboard users land inside the popup immediately. All three also set `autohide(true)` so Escape dismisses them without a mouse click.
- **Restore focus on popover close** — when any format bar popover closes, focus returns to the editor text view automatically.
- **`aria-pressed` on status bar toggles** — focus, format bar, autocorrect, and simple mode toggles now update `AccessibleState::Pressed` (true/false) so screen readers announce the button's on/off state.
- **Editor `aria-label` and `multiline` role** — each editor view now declares itself as "Document editor" with `Property::MultiLine(true)`, giving AT enough context to describe it as a multi-line editing area.
- **Error row severity labels** — the ✗ and ⚠ severity icons in each error row now carry `Property::Label("Compile error")` / `"Compile warning"` so screen readers pronounce the severity in plain language.
- **Alt+Enter spell suggestions** — pressing Alt+Enter while the cursor is on a misspelled word opens the spell-suggestion popover with focus already inside it, providing a keyboard equivalent to right-click for spell correction.
- **Reduced-motion support** — on startup, Zerkalo checks `gtk_enable_animations()`. When GNOME "Reduce Animations" is enabled, all CSS transitions and animations are suppressed via an application-priority override.
- **Tab accessible label tracks unsaved state** — when a tab becomes modified, its accessible label changes to "filename — unsaved"; on save, it reverts to the bare filename, so screen reader users know about unsaved files without relying on the coloured dot.

---

## [0.13.6-rc4] — Accessibility pass 1

### Added

- **Screen reader labels** — all icon-only buttons (Bold, Italic, table, figure, undo, redo, format bar toggle, focus toggle, autocorrect, simple mode) now have `aria-label` equivalents via `update_property(Property::Label(...))`.
- **Table grid cell labels** — each cell in the 8×8 table picker announces its size (e.g. "3×4 table"); label updates as the user hovers so AT tracks the selection.
- **Preview keyboard navigation** — the preview pane now accepts keyboard focus. `+`/`=` zoom in, `-` zoom out, `0` fit-to-width, `Space`/`Shift+Space` scroll a page down/up.
- **Error live region** — the error panel includes a visually-hidden `AccessibleRole::Status` label. When compile errors appear, it announces the count and first error message to screen readers without requiring focus to move.

### Changed

- **Status bar toggle contrast** — inactive status bar toggles now render at 70 % opacity (up from `dim-label`'s ~40 %) and reach full opacity on hover or focus, improving readability and meeting WCAG AA contrast for interactive controls.

### Fixed

- **Duplicate table grid handler** — removed the redundant pre-ep stub `connect_clicked` on each grid cell that closed the popover but discarded its row/col values. Only the real insertion handler (which uses `ep.active_view_buffer()`) now fires.

---

## [0.13.6-rc3] — Format bar power features and header cleanup

### Added

- **Format bar: font and size dropdowns** — right-aligned dropdowns change the document body font and font size via the sidecar, regenerating the template immediately.
- **Format bar: Insert Table** — hover an 8×8 grid to pick dimensions, click to insert a Typst `#figure(table(...))` block.
- **Format bar: Insert Image** — opens a file dialog; inserts `#figure(image(...))` with width and caption placeholder.
- **Focus toggle** — moved from the header bar to the status bar (same bold/regular toggle formatting as other status bar controls).

### Changed

- **Preview button** — replaced the "suggested-action pill" compile button with a flat "Preview" label that is bold when the preview pane is visible and regular when hidden. Clicking it toggles the pane and triggers compile on show.
- **Sidebar: Structure label removed** — the "Structure" heading above the outline panel has been removed.

### Fixed

- `set_filters` API: wrapped `&filters` in `Some(...)` for gtk4 0.7 compatibility.
- Deprecated `style_context()` call in table grid replaced with `queue_draw()`.

---

## [0.13.6-rc2] — Popup and format bar fixes

### Fixed

- **Citation and LSP completion popups** — double-click and Enter now correctly insert the selected item. Single-click now selects without inserting (natural autocomplete UX). Previously, single-click activated and double-click had no distinct handler.
- **Format bar toggle** — state is now read from the widget, so toggling works correctly when `format_bar_visible = false` is loaded from config.
- **Bold/Italic toolbar buttons** — rewritten to match Ctrl+B/I exactly: restores selection on inner text after unwrapping, places cursor between paired markers when inserting with no selection.
- **Heading buttons** — clicking the same heading level a second time removes the heading (toggle off).
- **Ghost text placeholder** — empty buffers now show a dim hint label (via GTK Overlay) that disappears on the first keystroke.

---

## [0.13.6-rc1] — New-user UX pass

### Added

- **Formatting toolbar** — Bold, Italic, H1, H2, H3, and page-break buttons appear above the editor. The toolbar can be hidden with the new "format bar" toggle in the status bar (default on); the setting persists across sessions.
- **First-run welcome screen** — new installs see a layout diagram and getting-started guide instead of the "What's New" list; returning users see What's New as before.
- **Auto-detect bibliography** — when no `.bib` file is configured, Zerkalo scans the project folder and loads the first `.bib` it finds automatically, showing a toast notification.
- **Template gallery auto-preview** — the first preset is previewed immediately when the gallery opens, so users see what the dialog does without having to click.

### Changed

- **Citation popup** — rows now lead with a bold "Smith et al., 2019" label so authors can scan by name, with title and `@key` below as secondary context.
- **Citation search** — the `@` popup now matches against author name and title in addition to the BibTeX key.

---

## [0.13.5] — 2026-06-09

### Fixed

- **Percent-decode URI**: non-ASCII paths (Cyrillic, spaces, etc.) were silently corrupted when opening files from the file manager — bytes were now collected correctly as `Vec<u8>` before UTF-8 conversion.
- **Autosave key stability**: `path_key` in `auto_save.rs` now uses FNV-1a 64-bit hash instead of `DefaultHasher`, so recovery files survive Rust version upgrades.
- **Compile-stats cache**: stats are now kept in an in-memory `OnceLock<Mutex<>>` and flushed to disk only every 10 compiles, eliminating per-compile file I/O.
- **Writing streak**: streak no longer resets to 0 before the first write of the day — if today has no entries yet the count starts from yesterday, surviving until midnight.
- **Git pull --rebase failure**: if `pull --rebase` fails, `git rebase --abort` is now called to restore a clean state and the remote is skipped instead of pushing a diverged commit.
- **Preview drop shadow**: shadow rectangles were expanding past the page edge on wide viewports — shadow drawing now uses the rendered page width, not `max(page, canvas)`.
- **Progress bar timer leak**: rapid recompiles no longer stack up multiple pulse timers; the previous `SourceId` is cancelled before spawning a new one.
- **Tab switch spurious recompile**: switching back to an already-compiled tab no longer re-triggers compilation when the content hasn't changed (per-file content hash).
- **Idle autosave blocked by errors**: autosave was silently skipped when the last compile produced errors; that guard is removed — autosave always runs on idle.
- **LSP diagnostic flicker**: clearing diagnostics on `did_change` was causing a one-frame flicker; panel now waits for 3 consecutive empty polls (~1.2 s) before hiding the indicator.
- **Multiple startup tool alerts**: missing `tinymist`/`pandoc`/`git` alerts were stacked as separate dialogs — they now appear as one combined alert.
- **Open-dropdown delete**: files are now moved to system trash via `gio::File::trash()` instead of permanently deleted with `std::fs::remove_file`.
- **New document default**: new file placeholder is now `= Title\n\n` (valid Typst heading) instead of `// New document\n\n`.
- **Save As**: dialog now pre-fills `untitled.typ`, restricts the file filter to `.typ`, and auto-appends the extension if the user omits it.
- **Update Template Settings no-op**: menu item now shows an alert when no file is open instead of silently doing nothing.
- **File tree delete**: now asks for confirmation before trashing a file (matching the open-dropdown behaviour).

### Changed

- **Simple Mode first-run dialog removed**: the modal alert explaining Simple Mode is gone; the same information is now in the Welcome window under a "Simple Mode" section, so it's readable at any time without interrupting the writing flow.
- **Welcome → Setup Wizard chaining**: the two startup dialogs no longer run on independent timeouts (which could race). The Setup Wizard now opens only after the Welcome window is dismissed via its "Get Started" button.
- **Welcome window Quick Start text**: corrected "the preview updates as you type" to "Press Ctrl+S to save and compile — the preview updates immediately" (accurate for the default `compile_on_save = true` behaviour).

## [0.13.5-rc3] — 2026-06-09

### Fixed

- **Completion popup arrow-key navigation** now skips hidden (filtered-out) rows correctly.
- **Escape in completion popup** now deletes the typed `#word` back to before the `#`, matching standard editor behaviour.
- **Completion popup appears immediately** when `#` is typed using built-in snippets; LSP results are merged in when they arrive (~150 ms later).
- **Completion popup no longer steals focus or blocks typing** (`autohide: false`; focus stays in the editor).
- **Completion popup no longer covers the cursor** (above/below logic, same as citation popup).

## [0.13.5-rc2] — 2026-06-09

### Added

- **Completion popup client-side filter**: popup now shows all completions when `#` is typed; as you type further letters the list filters and refocuses to the first match — no more replacing the whole list on each keystroke.
- **Numbering format selector**: Sections tab gains a "Numbering Format" ComboRow (Decimal 1.1.1., IEEE Roman I.A.1., Alpha a.a.a.) that appears when "Numbered Headings" is on.
- **Preview Code button**: header bar of the template dialog now has "Preview Code…" — shows the generated Typst preamble in a read-only window before applying.

### Changed

- **IEEE/GOST/Vancouver numbering now user-controlled**: the `#set heading(numbering:)` directive is no longer hardcoded inside the heading style strings; the "Numbered Headings" toggle (which defaults to ON for IEEE, GOST, Vancouver) drives it, so users can now disable IEEE's Roman-numeral numbering via the toggle.
- **Heading numbers now actually render**: custom `#show heading` rules now include `#if it.numbering != none [#context counter(heading).display(it.numbering)#h(0.3em)]` before the body, so turning on numbering actually shows numbers. Fixes GOST and Vancouver too.
- **Outline panel icons**: "Outline" and "Symbols" segmented buttons now use 20 px symbolic icons (`view-list-symbolic` / `input-keyboard-symbolic`) instead of text labels.

## [0.13.5-rc1] — 2026-06-09

### Added

- **Vancouver citation style**: new style option with numbered headings and Vancouver bib output.
- **Font size selector**: template dialog now has a 10/11/12/14 pt selector in the Typography section.
- **Numbered headings toggle**: Sections tab now has a "Numbered Headings" switch (1. 1.1 …).
- **#lorem() word count**: `#lorem(N)` is now counted as N words in section WC (breadcrumb) and outline panel word counts.
- **App screenshot**: added screenshot for GNOME Software / Discover.

### Changed

- **GOST style renamed**: "GOST 7.32" → "GOST R 7.0-5 (numeric)" throughout; bib style correctly uses `gost-r-705-2008-numeric` (was incorrectly falling back to APA).
- **Abstract preservation**: Update Template dialog now reads the abstract the user has typed directly in the `.typ` file; that text wins over the sidecar.
- **Codly package version**: bumped to 1.3.0; showybox to 2.0.4; gentle-clues to 1.2.0; drafting to 0.2.2.
- **Window title priority**: `#let doc-title = "..."` template variable is now checked before the first `= Heading` when setting the window title.

### Fixed

- **Preview no longer scrolls on mouse click**: clicking in the editor no longer jumps the preview; only keyboard navigation (typing) triggers the scroll-to-section.
- **LSP completion popup closes on click-away**: clicking outside the popup now dismisses it (autohide re-enabled).

---

## [0.13.4] — 2026-06-09

### Added

- **Ctrl+B / Ctrl+I**: wrap selection in `*bold*` / `_italic_` Typst markup; pressing again strips the markers.
- **Ctrl+Shift+E**: export PDF directly to the document folder with no dialog. Shows a toast on completion.
- **Section word count**: breadcrumb bar shows `§ N` word count for the heading section under the cursor.
- **Compile progress stripe**: thin pulsing bar at the bottom of the header bar while a compile runs.
- **Citation panel single-click**: clicking a reference in the sidebar inserts `@key` immediately (was double-click).
- **Window title from document**: header shows the Typst `title:` metadata, falling back to first `= Heading`, then filename.

### Improved

- Preview scroll position preserved across recompiles — no more jumping to the top on every compile.
- Preview page shadows: soft drop shadow behind each page on the gray canvas.
- Status bar separators: thin vertical dividers between control groups.
- Modified-dot indicator on tabs now renders in the accent color.
- Citation popup: no longer steals focus while typing a key; positions above/below cursor correctly; suppresses compile errors while the popup is open; shows full bibliography list.
- GOST bibliography style corrected to `gost-r-705-2008-numeric` (the actual bundled identifier).
- Breadcrumb separator changed from `›` to `/`.
- Style button no longer shows a dropdown arrow.
- "Developer mode" renamed to "Experimental mode" in settings.
- Auto-compile no longer halts after typing `@` when no bib matches are found.

---

## [0.13.4-rc8] — 2026-06-09

### Improved

- Section word count (`§ N`) moved from status bar to breadcrumb bar, appearing between the heading path and the word-wrap button.

---

## [0.13.4-rc7] — 2026-06-09

### Added

- **Ctrl+B / Ctrl+I**: wrap selection in `*bold*` / `_italic_` Typst markup; pressing again on an already-wrapped selection strips the markers.
- **Section word count**: status bar shows `§ N` word count for the heading section under the cursor; updates only when the cursor crosses a line boundary.
- **Compile progress stripe**: thin pulsing bar appears at the bottom of the header bar when a compile is running; disappears on completion.

### Improved

- **Preview page shadows**: each page now has a soft three-layer drop shadow against the gray canvas background.
- **Status bar separators**: thin vertical separators group the left controls from the right word-count block.
- **Modified-dot color**: the tab modified indicator (`●`) now renders in the accent color instead of the default foreground.

---

## [0.13.4-rc6] — 2026-06-09

### Added

- **Ctrl+Shift+E**: exports PDF directly to the document's own directory, no dialog. Shows "Exporting PDF…" toast while compiling, then success/error toast.
- **Window title from document**: header title now shows the Typst `title:` metadata field, falling back to the first `= Heading`, then the filename.
- **Citation panel single-click insert**: clicking once in the Citations sidebar now inserts `@key` at the cursor (was double-click).

---

## [0.13.4-rc5] — 2026-06-09

### Fixed

- Auto-compile no longer dies after typing `@`: `bib_active` flag is now cleared unconditionally on dismiss, and set only if the popup actually shows (previously stayed `true` when no bib matches were found, permanently suppressing compile).
- GOST 7.32: corrected bibliography style name to `gost-r-705-2008-numeric` (Typst's actual bundled identifier).

---

## [0.13.4-rc4] — 2026-06-09

### Fixed

- Citation popup: when appearing above the cursor, the bottom of the popup now lands at the cursor line's top edge — the cursor line is fully visible.

---

## [0.13.4-rc3] — 2026-06-09

### Fixed

- Citation popup: no longer steals keyboard focus — keystrokes always register in the editor.
- Citation popup: smart above/below placement — anchors below cursor when in the upper half of the view, above when in the lower half, so it never lands on the line being typed.
- Citation popup: compile and LSP diagnostics are suppressed while the popup is open, preventing error spam from partial `@keys`.
- GOST 7.32: bibliography style changed to `gost-r-7-0-5` — numeric citations with GOST-format entries using `//` article separators.

---

## [0.13.4-rc2] — 2026-06-09

### Changed

- Citation popup: positioned to the right of the cursor so it never overlaps the text being typed.
- Citation popup: shows the full bibliography list instead of capping at 15 entries.
- GOST 7.32 citation style: switched from author-date (APA) to footnote-based (Chicago Notes).
- Breadcrumb heading path: separator changed from `›` to `/`.
- Style button: plain button with no dropdown arrow (popover still works on click).

---

## [0.13.4-rc1] — 2026-06-09

### Changed

- Style dropdown: the dropdown triangle arrow is hidden (button still opens the menu).
- Settings: "Developer mode" renamed to "Experimental mode".
- Preview pane: scroll position is no longer reset after each compile — position is fully user-controlled (scroll, page buttons, arrow keys). Only the first compile auto-fits to width.

---

## [0.13.3] — 2026-06-09

### Changed

- LSP status indicator: running dot (●) is now green, error (✗) is red.
- LSP completion popup: items are now sorted alphabetically.
- LSP completion popup: double-click on a row inserts the completion (Tab and Enter already worked).

---

## [0.13.2] — 2026-06-09

### Fixed

- Simple Mode: preamble stays hidden after Update Template or Style dropdown — `buffer.set_text` was clearing all text tags; simple mode tag is now reapplied after every content replacement.
- Simple Mode: hidden preamble text is no longer silently dropped during compilation, saving, style changes, or spell-check — all content-retrieval calls now use `include_hidden_chars = true` so the invisible front-matter is always preserved.

---

## [0.13.1] — 2026-06-08

### Added

- **Simple Mode** toggle in the status bar (SIMPLE — bold when on, plain when off): hides the Typst front-matter above `// ── Document body` so you see only your document content. Line numbers are unchanged. Edit front-matter via the Update Template button. Defaults to on for new installs with a first-run explainer popup.
- Style dropdown moved from the status bar to the toolbar (breadcrumb bar, right side), next to word-wrap and undo/redo.
- Removed the open-tabs pan-down button from the toolbar (the file dropdown in the title bar handles this).

---

## [0.13.0] — 2026-06-08

### Fixed

- Preview pane: scroll now works immediately after the first compile, without needing to resize the pane first (root cause: content dimensions were set asynchronously, leaving the vadjustment with no scrollable range until the next resize)

---

## [0.12.34] — 2026-06-08

### Fixed

- DOCX/LaTeX import: strip ALL pandoc-generated `#set`, `#show`, and `#let` preamble blocks (previously only a subset was stripped, leaving `#let conf(...)` and `#show terms:` in the body and causing compile errors)
- Multi-line `#let` blocks now tracked with full delimiter depth (parens, brackets, braces) so large template functions are consumed correctly

---

## [0.12.34-rc2] — 2026-06-08

### Fixed

- Import LaTeX and Import DOCX now route pandoc through `flatpak-spawn --host` so they work inside the flatpak sandbox
- Error panel rows now show "Line N · filename:col" instead of "filename:line:col" for quicker scanning

---

## [0.12.34-rc1] — 2026-06-08

### Added

- Export for Web — converts the active Typst file to an HTML fragment via pandoc; footnotes become hover tooltips that respond to light/dark mode toggles (`data-theme`, `.dark`/`.light` classes, and `prefers-color-scheme`)

---

## [0.12.33] — 2026-06-08

### Added

- Delete button (trash icon) beside every file in the open dropdown — asks for confirmation, removes from disk, closes the tab if open, and removes the row from the list

---

## [0.12.33-rc1] — 2026-06-08

### Added / Changed

- Changelog window now renders release notes with proper GTK formatting — version headers, category sub-heads, and formatted bullets instead of raw monospace text
- Template dialog: lock (padlock) buttons on Author and Affiliation fields to save them as defaults for new documents
- Template dialog: date field now shows a tooltip noting it defaults to today if left blank
- Flatpak manifest: sourced from local directory instead of GitHub — test builds no longer require a push
- RC versioning scheme introduced: builds are numbered `X.Y.Z-rcN`; the suffix is removed on release

### Fixed

- Setup wizard widened to 640×620 for better readability
- What's New window updated to reflect 0.12.32 features

---

## [0.12.32] — 2026-06-08

### Added / Fixed

- Right-click on any editor tab shows a context menu with "Close tab" and "Delete file…" (with confirmation dialog)
- Fix: app was re-creating old project folder on every launch (removed unconditional `create_dir_all` of work_dir at startup)
- Author/affiliation lock: fixed compile error (missing fields in settings dialog Config initializer)

---

## [0.12.31] — 2026-06-08

### Fixed / Changed
- **Start maximized** — main window now opens maximized
- **Setup wizard: resizable** — removed non-resizable constraint; window is scrollable and resizable again
- **Setup wizard: bundled tools** — tinymist and pandoc shown as bundled (always ✓); only git and hunspell show install instructions

---

## [0.12.30] — 2026-06-08

### Changed
- **Welcome window: What's New** — updated to reflect 0.12.29 features (single-file workspace, status bar layout, page gaps, session restore, LCS diff, popout maximize)

---

## [0.12.29] — 2026-06-08

### Changed / Fixed
- **Status bar layout** — autocorrect and GOST Type B toggles moved to left side of status bar; Style dropdown and Draft/Final toggle moved from header bar to right side of status bar (Draft shown bold)
- **Preview: page gaps** — pages now separated by a visible 20px gray gap so page boundaries are clear
- **Session restore** — app now opens the last-edited file on startup
- **Snapshot diff** — replaced positional line diff with LCS-based diff; only truly changed lines shown as red/green, context lines shown around each change
- **Setup wizard sizing** — capped at 500×560, non-resizable to prevent oversized dialog
- **Setup wizard tool check** — `git`, `pandoc`, `tinymist` now correctly verified inside flatpak sandbox via `flatpak-spawn --host`; bundled tinymist at `/app/lib/zerkalo/tinymist` detected directly
- **Popout preview: maximize button** — added maximize button to the popout window header
- CLAUDE.md: added rule that GitHub pushes and flatpak publishes only happen on explicit release instruction

---

## [0.12.28] — 2026-06-08

### Fixed / Improved
- **Snapshot diff: color coding** — removed lines shown with red background/text, added lines green; hunk headers blue in git history
- **Snapshot diff: history panel click** — `connect_activate` → `connect_row_selected` (same fix as snapshot list; Single-mode ListBox rows weren't firing activate on single click)
- **Document Statistics popup** — replaced monospace text layout with `adw::PreferencesGroup` / `adw::ActionRow`; removed stale "Project total" row
- **Audit** — no other `connect_activate`-on-Single-mode bugs found in remaining panels

---

## [0.12.27] — 2026-06-08

### Fixed
- **Browse Snapshots: clicking a snapshot did nothing** — row handler was `connect_activate` (fires only on Enter/double-click); replaced with `list_box.connect_row_selected` so single clicks update the diff view and enable Restore

---

## [0.12.26] — 2026-06-08

### Fixed
- **Preview pane size resets on open**: pane position-notify now ignores changes during initial GTK layout (flag set on idle after realize), so the saved split is always restored correctly
- **Git sync uses wrong directory**: sync now derives the git repo root from the active file's path (`git rev-parse --show-toplevel`) instead of `config.work_dir`
- **Remove Open Project Folder / Recent Projects menu items**: these were holdovers from project mode and no longer serve any purpose

---

## [0.12.25] — 2026-06-08

### Changed
- **Single-file workspace**: removed project mode entirely. The active tab is always the compilation root — no root chip, no "Set as Compilation Root", no project config `root_file`. Removes `ProjectModel`, `is_project_mode` flag, New Project wizard, Project Settings dialog, and all related UI

---

## [0.12.24] — 2026-06-08

### Fixed
- **Crash on outline click in project**: `jump_to_line` was called synchronously inside `open_file`'s callback chain, causing reentrancy. Now deferred to idle so all page-switch callbacks complete before scrolling
- **Compile ignores changes in project mode**: `is_project_mode` was checking `EditorPane.project_root` (always set to `work_dir`, never None) instead of whether the user created an explicit project. Now uses a proper `is_project_mode` flag (true when `.zerkalo/config.toml` has `root_file`). In single-file mode, tab switch/save/keystroke all update the compilation root correctly
- **Ctrl+S reset project root**: saving any file was unconditionally calling `preview.set_root_file(path)`, clobbering the project root
- **Root chip "Project Settings…" did nothing**: `list_box.parent()` is GTK's internal `PopoverContent` wrapper, not a `Popover`, so the popdown before presenting the dialog never fired. Now stores `root_popover` directly in `EditorPane` and calls `popdown()` on it
- **Weird grey area in root chip popover**: removed separator `ListBoxRow` and replaced with top margin on the settings row
- **No way to leave project mode**: added "Clear root file" row to the root chip popover; clears the compilation root and returns to single-file mode (root follows active tab)

---

## [0.12.23] — 2026-06-08

### Fixed
- **Root chip now shows "Project Settings…"**: clicking the compilation root chip in the status bar opens a popover that lists candidate root files and includes a "Project Settings…" row at the bottom, so users can change or clear the root file without going through the hamburger menu
- **Outline filenames as tooltip**: in multi-file projects the file name is now shown on hover instead of inline, making the section title fully readable

---

## [0.12.22] — 2026-06-08

### Fixed
- **Preview not updating while typing in project mode**: the debounced on_change handler was calling `set_root_file(active_tab)` on every keystroke, so edits to a non-root file were compiled as if that file were the root. Now skips `set_root_file` when a project root is already set (same fix as the Compile button and tab-switch handler)
- Flatpak runtime bumped to GNOME Platform 50

---

## [0.12.21] — 2026-06-08

### Fixed
- **Crash on file switch in project**: `connect_switch_page` held `state.borrow()` while the page-switch callback called `all_tab_texts()` which tried to borrow state again → double-borrow panic. Fixed by extracting page data and releasing the borrow before firing the callback
- **Compile button resets project root**: clicking Compile (or switching tabs) was overwriting the project's compilation root with whatever file was active. Both now skip `set_root_file` when a project root is already set
- **tinymist not found in flatpak**: binary is at `/app/lib/zerkalo/tinymist` in the flatpak, not `/usr/lib/`; both `lsp.rs` and the startup check now probe both paths
- **history panel git calls broken in flatpak**: now uses `flatpak-spawn --host git` via the shared `host_command()` helper
- **pandoc/pdftotext broken in flatpak**: export dialog and PDF text extraction now use `host_command()` so they reach the host binaries

---

## [0.12.20] — 2026-06-08

### Fixed
- Startup git warning: use `flatpak-spawn --host git` inside the flatpak sandbox so the check passes correctly
- Welcome window "What's New" now lists the 0.12 features (multi-file projects, template build system, New Chapter, Project Settings, cross-file outline) instead of 0.11 era items
- GOST Type B font ships in all three flatpaks (Kopilka, Rubric, Zerkalo) at `/app/share/fonts/gosttypeb.ttf` so fontconfig inside the sandbox finds it

---

## [0.12.19] — 2026-06-08

### Added
- **Template build system**: built-in templates are now real `.typ` files in `templates/` (embedded via `include_str!` at compile time); user templates can be added to `~/.config/zerkalo/templates/<name>/manifest.toml` and appear in the New Project dialog automatically
- **New Chapter**: file tree header has a "New Chapter" button — enter a chapter name, creates `<slug>.typ` with a heading stub and appends `#include "<slug>.typ"` to `main.typ` before `#bibliography` (or at end); opens the new file immediately
- **Project Settings dialog**: "Project Settings…" in the menu opens a per-project settings sheet — change compilation root and bibliography path, saved to `.zerkalo/config.toml`
- **Cross-file outline**: document outline now shows headings from all project `.typ` files, not just the active tab; each heading shows which file it belongs to; clicking jumps to the correct file and line

---

## [0.12.18] — 2026-06-08

### Fixed
- New Project: after creation, the spawned process now receives `main.typ` as a CLI argument so session restore is skipped and the new project opens directly instead of restoring the previous project's documents
- Session restore: only files inside the current work_dir are restored; files from a previous project no longer leak in when the work_dir has changed

### Added
- Flatpak: `--socket=ssh-auth` added to finish-args so SSH git remotes work inside the sandbox

---

## [0.12.17] — 2026-06-08

### Added
- Help window: new "Projects" tab covering the full multi-file workflow (wizard, root, file tree, #include helper, project config, worked example)
- Help window: Overview tab now mentions multi-file projects with a pointer to the Projects tab
- Help window: five new FAQ entries (create a project, root concept, ★ indicator, add a chapter, missing root chip)
- README: Multi-file projects feature table; updated file tree row description

---

## [0.12.16] — 2026-06-08

### Added
- File tree: right-click menu now has "Insert #include" and "Insert #import"; inserts at the cursor with a path relative to the compilation root's directory
- `#import` snippet also adds the file stem as the imported identifier (`#import "ch01.typ": ch01`)

---

## [0.12.15] — 2026-06-08

### Added
- File tree: subdirectory rows are now collapsible — click the folder header to toggle; arrow icon shows expand/collapse state
- File tree: "New Folder" button (folder-new-symbolic) in the panel header creates a subfolder in the project root
- DnD idle-rebuild simplified to use `FileTree::clone()` instead of manual field reconstruction

---

## [0.12.14] — 2026-06-08

### Added
- Status bar: "Root: filename.typ" chip button; clicking it opens a popover listing all candidate root files so you can switch the compilation root without touching the file tree
- `ProjectModel::candidate_roots()` — returns files not imported by any other

---

## [0.12.13] — 2026-06-08

### Added
- File tree: ★ indicator on the current compilation root row
- File tree: right-click context menu now has "Set as Compilation Root" above "Delete"; selecting it writes `root_file` to `.zerkalo/config.toml`, updates the preview, and triggers a recompile
- New Project wizard: four templates (Blank, Essay, Journal/Thesis, Theological Journal); creates project folder, generates starter .typ files, opens the new project directly

---

## [0.12.12] — 2026-06-08

### Fixed
- **Flatpak: git sync now works** — all `git` calls delegate to the host system via `flatpak-spawn --host git`; added `--talk-name=org.freedesktop.Flatpak` to finish-args
- **Flatpak: Typst package cache accessible** — added `--filesystem=~/.cache/typst` so packages installed on the host are found inside the sandbox

---

## [0.12.11] — 2026-06-08

### Changed
- Flatpak: strip debug symbols from zerkalo and tinymist binaries — reduces flatpak size by ~50 MB

---

## [0.12.10] — 2026-06-07

### Changed
- "Backup Remotes" menu item renamed to "Git Remotes"
- Git Remotes dialog now includes a "Primary Remote" section at the top for viewing and editing the origin (GitHub) URL — no longer need the Setup Wizard to change which repo the project syncs with

---

## [0.12.9] — 2026-06-07

### Changed
- Syntax scheme preference order: `solarized-dark` / `tango` first, Adwaita as fallback
- Comment block highlight changed from neutral grey wash to a faint blue tint for visual distinction
- Current-line highlight now uses accent colour (`alpha(@accent_color, 0.06)`) for consistency with the cursor
- Style dropdown shows only the style name, not the document filename

---

## [0.12.8] — 2026-06-07

### Fixed
- Plan panel toggle button now uses `view-list-symbolic` instead of the missing `text-editor-symbolic` icon

---

## [0.12.7] — 2026-06-06

### Added

- **Section Notes panel**: the right sidebar now has two tabs — "Plan" (existing scratchpad) and "Notes" (new). The Notes tab mirrors the document outline; clicking a heading loads that section's planning note in a text area below. Notes are saved as `<filename>.notes.json` alongside the `.typ` file. Keys are preserved by heading text across edits; headings that disappear are garbage-collected from the sidecar. The list and notes update live as you type.

---

## [0.12.6] — 2026-06-06

### Changed

- **Preview toolbar**: removed Copy Text, Jump to Editor, and Watch Mode buttons. Ctrl+Click on the preview for jump-to-source still works.
- **Find bar**: hidden by default; the "search" button in the status bar now turns blue when the bar is open (Ctrl+F or Esc to toggle).

### Fixed

- **Settings hang**: spell recheck after changing languages/enabling spell check now runs hunspell off the GTK main thread, so the UI stays responsive.

---

## [0.12.5] — 2026-06-06

### Fixed

- **tinymist bundled in deb/rpm now detected correctly**: startup availability check now probes `/usr/lib/zerkalo/tinymist` first (matching the LSP launcher logic), so deb/rpm installs no longer show a spurious "Optional: tinymist" alert.
- **RPM spec `%files` section**: removed the broken `%if 0%{?with_tinymist}` conditional — tinymist is always bundled in release packages and must be listed unconditionally to avoid an "installed but unpackaged files" build error.

---

## [0.12.4] — 2026-06-06

### Fixed

- **GitHub token dialog now actually works**: previously the token was saved to disk but the in-memory config was not updated, so the next sync attempt still used no token. Fixed — the dialog now updates the live config immediately.
- **Auto-retry after login**: the dialog button now reads "Save & Sync" and automatically retries the push after saving the token, so the user doesn't need to click the sync button a second time.
- `do_sync` and `show_sync_result` now share the live `current_config` so future auth-failure retries read the correct token.

---

## [0.12.3] — 2026-06-06

### Fixed

- Settings dialog now preserves `active_profile`, `word_count_goal`, `last_export_format`, `recent_searches`, and `auto_save_idle_ms` when saving — previously these were reset to defaults on every Settings save.
- Removed stale `cos_for_watch` variable in file-watcher callback (unused since compile-on-save logic was refactored into the pill).
- Removed dead `default_auto_save_idle_ms_pub` export from `config.rs`.

---

## [0.12.2] — 2026-06-06

### Added

- **GitHub login dialog**: if a push fails with an authentication error, Zerkalo shows a "GitHub Login" dialog prompting for a Personal Access Token (PAT). The token is stored in the local config and injected into HTTPS remote URLs on future syncs — no terminal needed.
- **GitHub token in Settings**: the "General" settings page now has a "GitHub Sync" section where the PAT can be set or updated at any time.
- **Pull before push**: `sync()` now runs `git pull --rebase` before each push so that multi-machine workflows don't produce non-fast-forward rejections.

### Fixed

- `Config::default()` missing `github_token` field (would have caused a compile error on new installs).

---

## [0.12.1] — 2026-06-06

### Changed

- **Native packaging**: release workflow now produces `.deb` (Ubuntu/Debian/Mint) and `.rpm` (Fedora/openSUSE) packages instead of an AppImage. `pandoc` and `hunspell` are declared as package dependencies so they are installed automatically.
- **Bundled tinymist**: the LSP binary is bundled at `/usr/lib/zerkalo/tinymist` inside the deb/rpm packages — no separate download step needed after install. The source-build path still prompts to install tinymist separately.
- **install.sh**: now detects dpkg/rpm and downloads the appropriate native package; falls back to cargo build only as a last resort.

---

## [0.12.0] — 2026-06-05

### Added

- **Keyboard Shortcut Remap**: Command Palette moved to **Ctrl+K** (was Ctrl+P); Git Sync moved to **Ctrl+Shift+S** (was Ctrl+Shift+G). Both are configurable via `~/.config/zerkalo/keybindings.toml` using the new `command_palette` and `shortcuts_help` keys.
- **Ctrl+Shift+H — Dynamic Keyboard Shortcuts Help**: opens a dialog showing the *current* effective keybindings read from `keybindings.toml` at runtime rather than a static list.
- **Compilation Time Display**: status bar now shows "Compiled in Xs". Times over 3 s turn **yellow** and show a tooltip with three optimization tips (Draft profile, image placement, file splitting). Stats are appended to `~/.cache/zerkalo/compile_stats.json` on every compile.
- **Auto-backup on Idle**: the autosave backup ticker is now idle-triggered — it fires `auto_save_idle_ms` milliseconds (default 30 000) after the last keystroke, not on a fixed wall-clock interval. Backups are skipped when the document has active compile errors. `auto_save_idle_ms` is a new field in `config.toml`.
- **Command Palette Enhancements**: four new commands — **Find in Files…** (opens the project search panel), **Toggle Profile** (switches Final ↔ Draft compile profile), **Browse Snapshots…** (opens the snapshot timeline for the current file), and **Project Outline** placeholder (use Ctrl+G for full heading navigation).

---

## [0.11.0] — 2026-06-05

### Added

- **Configurable Compilation Profiles**: header-bar dropdown switches between **Final** (full 144 dpi render) and **Draft** (72 dpi, fast preview) profiles. Draft mode passes `sys.inputs.at("draft", default: "false") == "true"` so documents can detect the mode and skip slow elements. Profile persists to `config.toml`.
- **Session Snapshots & Version Recovery**: every Ctrl+S (and ☰ → Save) writes a timestamped `.typ` snapshot to `~/.local/share/zerkalo/snapshots/<project>/<file>/`. The last 50 snapshots per file are retained automatically. ☰ → **Browse Snapshots…** opens a timeline dialog showing each snapshot with a simple diff against the current text; **Restore** replaces the editor content.
- **Enhanced Spell Check**: project-specific dictionary at `<work_dir>/.zerkalo/dictionary.dic` (hunspell `.dic` format). Global user dictionary moved to `~/.config/zerkalo/user.dic`. Right-click on a misspelled word now shows **Add to Project Dictionary** when a project dictionary is available, in addition to the existing **Add to Dictionary** (global).
- **Inline Typst Error Assistant** (`src/error_patterns.rs`): hovering over a red-underlined error line shows a popover with the error description. For known patterns (missing brace/bracket/paren, unknown variable) a **Fix It** button applies the automated correction inline. The fix table lives in `src/error_patterns.rs` and is easy to extend.

---

## [0.10.0] — 2026-06-05

### Added

- **Find in Files enhancements** (Ctrl+Shift+F): search results now highlight the matched text in bold (Pango markup); `.gitignore` patterns are respected so build artifacts and output directories are excluded; replace-in-files mode (toggle button in search bar) with a replace entry and "Replace All" button that writes files and reloads any open tabs; last 10 searches stored in `config.toml` and shown in a dropdown next to the search entry.
- **Interactive Preview Click-to-Jump**: Ctrl+Click on the preview jumps to the matching source line by extracting text from the current PDF page via `pdftotext`; if no PDF exists it is compiled on demand. New "Copy Text from Preview" button (clipboard icon) and "Jump to Editor" button (jump icon) in the preview toolbar. Graceful error message if `pdftotext` (poppler-utils) is not installed.
- **Export Progress Dialog**: redesigned with a scrollable log view showing real-time stderr output line-by-line for all export operations; batch export mode with per-format checkboxes so multiple formats can be exported in one click; "Install Dependencies…" button opens the System Check Wizard; full error detail is always visible instead of only the first line.

---

## [0.9.0] — 2026-06-05

### Added

- **System Check Wizard**: dependency rows now detect the Linux distro from `/etc/os-release` and show the exact `apt`/`dnf`/`pacman`/`zypper` install command for each missing tool (pandoc, hunspell, git, tinymist). A "Verify" button re-checks presence after installation.
- **Template Marker Recovery**: ☰ → "Repair Template Markers…" scans the active file for the `// ── Document body` marker; if missing, re-inserts it at the preamble boundary and saves a `.typ.bak` backup. Generated templates now include a "DO NOT DELETE" warning comment above the marker.
- **Compile-on-save mode** (`compile_on_save = true` by default): on-keystroke debounce no longer triggers compilation; compilation fires on Ctrl+S instead. New `manual_compile_only` setting (default `false`) disables all automatic compilation — use Ctrl+Shift+P to compile manually. Both settings exposed in Settings → Compilation.
- **Filesystem watcher** (`notify` crate): watches the project directory for external `.typ` file changes (e.g., sync agents, other editors) and triggers re-compilation automatically.

### Fixed

- Config test: `spell_language` → `spell_languages` (field name mismatch)
- Template dialog test: added missing `sidecar_to_settings` function used by the round-trip test

---

## [0.8.19] — 2026-06-05

### Fixed

- Update Template dialog now reads metadata (title, author, etc.) from the document rather than the sidecar, so in-document edits to `#let doc-*` variables are reflected when the dialog opens
- Chicago Author-Date bibliography section heading corrected from "Reference List" to "References" (CMOS §15.2)

---

## [0.8.18] — 2026-06-05

### Fixed

- Preview auto-reflows when the window is resized: the viewport-width is now watched and `fit_width` re-runs whenever `auto_fit` is active. Zooming in/out disables auto_fit; clicking the fit-width button re-enables it

---

## [0.8.17] — 2026-06-05

### Added

- Multi-language spell checking: Settings → Extras → Spell Check now shows a list of active dictionaries with remove buttons, and an "Add language" dropdown to add more. A word is considered correctly spelled if it passes in any of the active dictionaries (so bilingual documents don't flag words from either language)

---

## [0.8.16] — 2026-06-05

### Fixed

- Word wrap now correctly reflowing on window resize: when wrap is on the horizontal scroll policy is `Never` (GTK wraps at the window edge); when wrap is off it switches to `Automatic` so long lines can be scrolled rather than silently clipped

---

## [0.8.15] — 2026-06-05

### Added

- Clicking the word count in the status bar opens a Document Statistics window: words (with session delta), characters, paragraphs, sentences, reading time, and project total if a project root is set

---

## [0.8.14] — 2026-06-05

### Fixed

- "search" status bar button now correctly shows/hides the Find & Replace bar (same as Ctrl+F), not a code-search toggle

---

## [0.8.13] — 2026-06-05

### Added

- Clicking the version number in the status bar opens the changelog in a scrollable window

---

## [0.8.12] — 2026-06-05

### Changed

- **GOST type B** toggle moved from the sidebar to the status bar — same clickable-text format as autocorrect (bold = on, dim = off)
- **search** toggle added at the left end of the status bar — controls whether Find/Replace searches inside `#commands` and `//comments` (bold = searching code too, dim = prose only)
- Removed the old sidebar Switch widget for GOST type B

---

## [0.8.11] — 2026-06-05

### Fixed

- Droplet package import updated from 0.2.0 to 0.3.1

---

## [0.8.10] — 2026-06-05

### Improved

- Completion popup snippets now show a plain-English description of what each snippet does, instead of just the raw key name
- Snippet labels no longer carry the redundant "· snippet" suffix — the kind badge already shows that
- Added a `dropcap` snippet: typing `#dropcap` now offers a ready-to-use example with a note that the Droplet package must be enabled in template settings → Packages

---

## [0.8.9] — 2026-06-05

### Added

- Autocorrect toggle in the status bar: click the word "autocorrect" to turn it on (bold) or off (dim). State is saved to config immediately, so it persists across sessions.

---

## [0.8.8] — 2026-06-05

### Fixed

- LSP/snippet completion popup no longer overlaps the text being typed — it now anchors at the left margin of the editor, below the current line
- Popup is wider (480 px) and taller (380 px max), so function signatures and documentation are readable without truncation
- Detail text now wraps instead of being cut off with an ellipsis
- Added a footer hint showing the keyboard controls (↑↓ navigate · Tab/↵ insert · Esc dismiss)

---

## [0.8.7] — 2026-06-05

### Internal

- Removed 6 dead functions from `template_dialog.rs` (`extract_preamble`, `sidecar_to_settings`, `replace_in_set_blocks`, `reapply_preamble`, `update_body_front_matter`, `update_body_front_matter_headingless`) along with their tests

---

## [0.8.6] — 2026-06-05

### Fixed
- **Citation panel — missing titles** — replaced regex `[^{}]*` field parser with a brace-depth-aware manual parser; titles containing nested braces (e.g. `{On {Church} and {State}}`, `{{All Caps Title}}`) now parse correctly instead of returning empty
- **Citation panel — double-click** — switched from per-row `connect_activate` to a single `list.connect_row_activated` handler (the canonical GTK4 activation path); double-click and Enter now both insert the citation key; `activate_on_single_click` explicitly set to `false` to match expected UX

---

## [0.8.5] — 2026-06-05

### Fixed
- **No-marker confirmation** — "Update Template Settings" now shows a destructive-action confirmation dialog when the document has no `// ── Document body` marker, warning the user that their content will be replaced
- **Corrupt sidecar logging** — `load_sidecar` now emits a `WARN` log entry when the `.zerkalo.toml` exists but fails to parse (previously swallowed the error silently)

---

## [0.8.4] — 2026-06-05

### Changed
- **Template settings sidecar** — each `.typ` document now gets a `<stem>.zerkalo.toml` sidecar file that stores all template settings (style, font, paper, margins, sections, languages, packages, metadata). "Update Template Settings" reads from the sidecar instead of text-parsing the `.typ` file, so pre-fill is always reliable.
- **Apply redesign** — "Apply to Current" now regenerates the preamble/title/front-matter completely from the new settings and splices at the `// ── Document body` marker, preserving user body content. Replaces the fragile four-pass text-surgery approach.
- **`TemplateDialog` extended** — dialog now stores and preselects page-numbers, language switches, and package switches from sidecar (previously could not round-trip these fields).

---

## [0.8.3] — 2026-06-05

### Changed
- **app_window.rs split** — CSS loading extracted to `load_app_css()`; hamburger menu items extracted to `build_hamburger_menu_items()` + `HamburgerItems` struct
- **Plan panel project fallback** — panel accepts `work_dir`; when no file is open it loads `project.plan` from the project root instead of disabling
- **Export dialog** — remembers last-used format across sessions via `last_export_format` in config
- **Style button loop** — replaced `unwrap()` on downcast with safe `if let`

### Added
- **Session delta label** — status bar shows `↑ N` words added since file was opened
- **Tab error indicator** — red ⬤ dot on tab label when the file has compile/LSP errors
- **Ctrl+G** — opens command palette pre-filtered to document headings only

### Fixed
- `Cargo.lock` removed from `.gitignore` (correct for binary applications)

---

## [0.8.2] — 2026-06-05

### Changed
- **TODO panel → Plan panel** — replaced the per-file checklist with a freeform text scratchpad; notes are saved as a `.plan` sidecar file alongside the `.typ` document

---

## [0.8.1] — 2026-06-05

### Fixed
- **Style switch** — switching styles no longer wipes out the abstract, outline, extra pagebreaks, bib file pointer, or bibliography; the title-block replacement now stops at the first front-matter/body marker instead of scanning the whole document for `#pagebreak()`
- **Default font** — template dialog now defaults to Times New Roman instead of GOST type B
- **Sidebar** — sidebar can now be compressed to a much smaller width; search entry, buttons, and labels have `min-width: 0` so the paned divider is no longer blocked; citation key labels ellipsize rather than forcing a minimum width

---

## [0.7.1] — 2026-06-01

### Fixed
- **Simple mode** — cheatsheet/help toggle and pop-out preview button are now visible in simple mode; only watch mode, page navigation, compile-time label, and advanced menu items are hidden

---

## [0.7.0] — 2026-06-01

### Added
- **Import wrapping** — LaTeX, DOCX, and PDF files imported via ☰ → Import… now receive a Zerkalo-managed template section (`ZERKALO-TEMPLATE-BEGIN/END`) automatically; imported documents are immediately responsive to "Update Template Settings" without any manual preamble setup
- **Startup checks for `hunspell` and `tinymist`** — if either is missing, a dialog at startup shows per-distro install instructions (`zypper`, `apt`, `brew`, `dnf`); pandoc and pdftotext error dialogs also now include platform-specific install commands
- **22 new unit tests** — covering `parse_font`, `parse_paper`, `parse_spacing`, `replace_in_set_blocks`, `strip_style_block`, `reapply_preamble` (font and spacing propagation), and `strip_pandoc_preamble`

### Changed
- **Line spacing recalibrated** — spacing options now use Typst `leading:` (inter-line gap) rather than `spacing:` (paragraph gap): Single = 0.65 em, 1.5 Lines = 0.9 em, Double = 1.2 em; templates generate both `leading:` and a fixed `spacing: 1.2em` in `#set par`
- **Font replacement scoped** — "Update Template Settings" font substitution now only touches `#set text(…)` blocks; comments and string literals containing the old font name are left unchanged
- **Spacing propagation** — "Update Template Settings" now propagates `leading:` changes to every `#set par(…)` block in the document (including hand-written config sections after the template marker), matching the existing font-propagation behaviour

### Fixed
- **RefCell re-entrancy crashes (3 classes)** — `set_content`, `set_active_content`, and `close_file` each held an active borrow guard when calling `buffer.set_text()` or `notebook.remove_page()`, which synchronously fired GTK signals that re-entered the same `RefCell` and panicked; all three patched with the borrow-then-clone-then-drop pattern
- **Startup crash: stale `glib::SourceId`** — `SourceId::remove()` was called on a timer ID that had already auto-removed itself on first fire, causing a panic on startup; timer callbacks now clear their own slot immediately so stale IDs are never removed
- **Template style-block override** — a `ZERKALO-STYLE-BEGIN/END` block from the legacy Style button appearing after the template marker would silently override font and spacing; it is now stripped whenever "Update Template Settings" is applied
- **Tab dropdown borrow safety** — the tab-list popover held `state.borrow()` across GTK widget construction and `vbox.append()` calls; the borrow is now released before any GTK calls

---

## [0.4.0] — 2026-05-28

### Added
- **Preview ↔ Reference toggle** — `?` button in the preview toolbar switches the right column between live preview and a built-in reference panel with three tabs: Cheatsheet (full academic Typst syntax), Help (overview + getting started), and FAQ
- **Typst Cheatsheet** — comprehensive in-app reference covering headings, text formatting, citations, figures, tables, math, footnotes, footnote entry settings, special elements (outline, pagebreak, spacers, horizontal rules), links, blocks, multi-column layout, set rules (text, paragraph, page, heading), includes/imports, and git sync shortcut
- **Git sync keyboard shortcut** (`Ctrl+Shift+G`) — triggers commit & push; configurable in `~/.config/zerkalo/keybindings.toml` as `git_sync`
- **DOCX import** — ☰ → Import… → Word (.docx); converts via `pandoc -f docx -t typst --standalone`; applies same post-processing as LaTeX import (pagebreaks, bibliography stub)
- **PDF import** — ☰ → Import… → PDF (.pdf); extracts text via `pdftotext -layout`; wraps in a minimal Typst preamble
- **Unified Import… dialog** — single picker in ☰ → Import… presents LaTeX, DOCX, and PDF options
- **Preview page navigation** — prev/next buttons and "N / M" counter in the preview toolbar; scroll-to-page with midpoint detection
- **Minimap toggle** — `⊞` button in header bar shows/hides a thin GtkSourceView source map alongside the editor
- **Template gallery** — five built-in presets in New from Template: Generic Academic, Research Article APA, GOST 7.32, IEEE, Academic Letter; gallery tab with preview rendering
- **Per-file compile state** — file tree shows `dialog-error-symbolic` icon on files with compile errors; clears on success
- **Inline compile-error banner** — first error line shown in a scrollable banner below the preview; clears on successful compile
- **Drag-and-drop image insertion** — drag an image onto the editor to copy it to the work folder and insert `#figure(image("…"), caption: [])` markup
- **Autosave indicator** — title bar subtitle shows "Modified" while unsaved; "Saved" (auto-clears after 2 s) on save
- **Recent documents grouped by date** — open dropdown groups files as Today / This week / Older
- **Comment highlighting** — `//` and `/* */` comment blocks receive a theme-aware paragraph background fill; adjacent `//` lines merge into one span
- **Style dropdown label** — header style dropdown label updates to the name of the currently applied style
- **Ctrl+? shortcut** — opens the Help & Shortcuts window

### Changed
- **Title bar** — active filename shown without `.typ` extension
- **Header bar layout** — Style dropdown beside the title; Todo button right of Preview; hamburger menu rightmost; preview toolbar moved to bottom of preview area
- **Minimap width** — reduced to 72 px (thin, non-intrusive)
- **Git icon** — changed to `vcs-commit-symbolic`
- **About dialog** — updated to 0.4.0; lists embedded Typst compiler
- **Startup tool check** — removed `typst` from the check; only `git` is needed (compiler is now embedded)

### Fixed
- **GOST template language** — GOST 7.32 template now generates `lang: "en"` (was `"ru"`) to avoid font and hyphenation issues

---

## [0.5.0] — 2026-05-28

### Added
- **Spell check** — prose words in `.typ` documents are checked against the system Hunspell dictionary; misspelled words receive a blue wavy underline; right-click on any underlined word shows up to 6 suggestions (click to replace) and an "Ignore All" option; Typst markup (`#`, `@`, `$`, `//`, `/* */`, raw blocks) is excluded from checking
- **Spell language selection** — Settings → Spell Check → Dictionary language; lists all `.dic` files found under `/usr/share/hunspell` and `/usr/share/myspell`
- **Autocorrect** — optional (off by default); Settings → Spell Check → Autocorrect; replaces a word on word-boundary input when the top Hunspell suggestion has Levenshtein distance ≤ 1; proper nouns are never autocorrected; undo-able as a separate action
- **Breadcrumb bar** — a bar above the editor shows the full heading path at the cursor position (e.g. "Chapter One › The Problem Stated"); updated on every cursor move
- **Update Template Settings** — ☰ → Update Template Settings / sidebar "Update Template…" button re-applies preamble settings (citation style, paper size, margins, fonts, spacing, ToC/Abstract/Keywords) to an existing document without touching the body; the current style is pre-selected by reading the `// @zerkalo-style:` metadata line
- **Embedded Typst compiler** — preview compilation and rendering are fully in-process via the `typst`, `typst-render`, and `typst-kit` crates; no `typst` binary or `pdftoppm` required; render resolution fixed at 2.0 px/pt (≈ 144 dpi)
- **Multi-remote Git push** — `sync()` pushes to every configured remote; per-remote failures reported individually without blocking other remotes
- **Backup remote setup** — Setup wizard and ☰ → Backup Remotes… dialog let users add a second remote (e.g. Codeberg) alongside the primary origin
- **Broken-citation jump** — clicking a broken `@key` citation in the Refs panel jumps to and selects that citation in the editor
- **Animated find bar** — `Ctrl+F` slides the find/replace bar in with a 200 ms `gtk4::Revealer` `SlideDown` animation instead of appearing instantly
- **Dark-mode syntax fallback** — `apply_style_scheme` tries `Adwaita-dark → oblivion → solarized-dark → classic-dark` in order; light mode tries `Adwaita → classic`
- **Sidebar section headers** — dim "Structure" label above the outline panel and "Project" label above the Refs/History/Files notebook
- **Simple-mode explanation** — `?` button beside the Simple mode switch opens a tooltip-style dialog explaining what the mode hides
- **Paned divider hover** — CSS transition highlights the editor↔preview drag handle in the accent colour on hover
- **Style button shows filename** — the Style dropdown label now reads "GOST 7.32 · main" (detected style + active filename); updates on tab switch and file open
- **Minimap in hamburger menu** — minimap toggle moved from the header to ☰ → Toggle Minimap; Browse Documents also moved to the hamburger View section, decluttering the header
- **Abbreviated cursor position** — status bar shows "L12:C5" (was "Ln 12, Col 5") with a "Line 12, Column 5" tooltip

### Changed
- **Settings dialog** — reorganised into three tabs: General (folders + compilation), Editor (color scheme + font/whitespace), Extras (bibliography + spell check); was a single long scrollable page
- **Header bar** — only `sidebar toggle | focus | Style ▾` on the start; end unchanged; docs browser and minimap toggle moved to hamburger
- **Heading styles corrected and unified** — all styles use `block(width: 100%)` + `#set par(first-line-indent: 0pt)`; SBL gets five heading levels; Turabian H2 centred plain; ASA H1 flush-left ALL CAPS; Chicago Notes-Bib separated from Turabian

### Fixed
- **GTK "Unknown tag" warnings** — `ensure_diag_tags()` now called before `remove_tag_by_name` in both `mark_diagnostics()` and `clear_diagnostic_marks()`
- **Preview pixbuf race condition** — generation counter discards stale results; PNG bytes read into memory in the worker thread
- **Launcher not launching** — removed `DBusActivatable=true` from the desktop file
- **Find bar layout** — removed `set_width_chars(12)` reservation on the result label that caused a large empty gap
- **Minimap position** — minimap was added outside the editor pane and covered text; now placed inline beside the `ScrolledWindow` inside the editor pane

---

## [0.6.0] — 2026-05-28

### Added
- **Line spacing control** — Settings → Editor → Line spacing: Compact (0 px), Normal (2 px, default), Spacious (6 px); persisted in config
- **Zen writing mode** — Focus button now dims the sidebar to 30 % opacity via a CSS transition instead of hiding it entirely; editor text gains 40 px left/right padding so the writing area feels centred
- **Typewriter scrolling** — Settings → Editor → Typewriter scrolling; on every cursor move the view scrolls to keep the cursor at ~45 % from the top of the viewport; automatically disabled during mouse-selection drags
- **Per-document word-count goal** — add `// @goal: 3000` anywhere in a `.typ` file to set a word target; a progress bar appears in the status bar showing progress toward the goal; bar is hidden when no goal is set
- **Command palette** — `Ctrl+P` opens a fuzzy command palette listing all standard app commands and every heading in the current document; `↑`/`↓` navigates; `Enter` activates; `Esc` closes
- **Selection word/sentence stats** — while text is selected the status bar replaces the word count with "N words, M sentences selected"; reverts to the document word count when selection is cleared
- **High contrast mode** — Settings → Editor → High contrast mode; adds a `high-contrast` CSS class to the window that forces white-on-black in the editor text view; persisted in config
- **Auto-pair brackets and quotes** — typing `(`, `[`, `{`, or `"` inserts the matching closing character and places the cursor between them; implemented as a single undo-able buffer action
- **Save-before-close dialog** — closing the window with unsaved files now shows a modal listing each modified filename with **Save All**, **Discard**, and **Cancel** responses; "Save All" writes all modified buffers to disk before closing

### Changed
- **Horizontal scroll locked in word-wrap mode** — editor `ScrolledWindow` horizontal policy is set to `Never` when word wrap is active, eliminating the rightward cursor-follow drift; policy is updated when word wrap is toggled and when existing tabs are affected
- **Sidebar scroll fixed** — all sidebar panels (reference manager, file tree, outline, search, todo) now enforce horizontal scroll policy `Never`, preventing unexpected rightward scroll when clicking items

### Removed
- **Git history panel** — the git-history sidebar panel has been removed; it was unreliable and of unclear value. Use `git log` in a terminal or a dedicated Git client for history browsing.

### Fixed
- **Update Template Settings** — the "Update Template Settings" flow no longer opens a file-save dialog; preamble is applied in-memory and written directly to the current file

---

## [0.2.0] — 2026-05-26

### Added
- **Style switcher** — header-bar dropdown applies a full citation style to the active document; styles: SBL, Chicago (Notes-Bib), Chicago (Author-Date), MLA, APA 7th, ASA, Turabian, Harvard; updates or appends `#bibliography(...)` at the end of the document with the correct style key and section title ("Works Cited" for MLA, "Reference List" for Chicago Author-Date)
- **New from Template dialog** — five-tab dialog (Document, Layout, Sections, Languages, Packages) generates a complete Typst preamble; citation styles, paper sizes, margin presets, font selection, line spacing, page numbers, ToC/Abstract/Keywords toggles, language support (Russian, Hebrew, Greek, Japanese, Sanskrit, Tibetan, Chinese), extra packages (Droplet, Codly, Showybox, Gentle Clues, Tablex, Drafting)
- **Todo panel** — split pane with Global and per-file todo lists; checkbox rows; Enter adds item; checked items move to a Completed section with strikethrough; persisted as `- [ ] / - [x]` markdown files
- **Session restore** — open files, active tab, and cursor positions saved to `~/.local/share/zerkalo/session.json` and restored on next launch
- **Configurable keybindings** — `~/.config/zerkalo/keybindings.toml` written on first launch with defaults; parsed at runtime so edits take effect on next start
- **LaTeX import** — ☰ → Import LaTeX File; converts `.tex` to Typst via `pandoc -f latex -t typst` and opens the result in a new tab
- **Export ODT and LaTeX** — two new formats added to the Export dialog (pandoc)
- **Inline LSP diagnostics** — compile errors and LSP warnings rendered as red/amber underlines in the editor using GtkSourceView TextTags
- **Built-in academic snippets** — figure, table, footnote, bibliography, pagebreak, outline, lorem, set rule, show rule, block; prepended to the LSP completion popup with `#`-prefix matching
- **Font management** — ☰ → Font Management; searchable checkbox list of all fc-list fonts; Enable All / Disable All; persisted to `~/.config/zerkalo/font-preferences.toml`
- **GOST Type B font** — bundled in `assets/fonts/` and installed to the user font directory on first launch
- **Welcome window** — version-keyed "What's New" dialog shown on first launch of each new version; scrollable; includes Quick Start and keyboard shortcuts
- **Cursor-tracking outline** — outline panel highlights the heading the cursor is currently under as you type
- **Whole-word find** — "W" toggle button in the Find bar; checks word boundaries before and after each match
- **LSP diagnostic deduplication** — when tinymist sends diagnostics, compile-stderr errors are suppressed to avoid duplicates
- **Outline click navigation** — single click on an outline row centres and selects the heading line in the editor
- **Auto-compile on file open** — switching to a tab immediately triggers a compile (no manual Preview click needed)
- **Simple mode sidebar toggle** — switch at the bottom of the sidebar
- **System accent colours** — outline hover and selected rows use `@accent_color` / `@accent_bg_color` from the Adwaita theme

### Changed
- Debounce reduced from 500 ms to 300 ms
- Preview button labelled "Preview" (was an icon-only button)
- Segmented Outline|Symbols control moved inside the outline panel
- Version bumped to 0.2.0

### Removed
- DOI/ISBN import (Zotero does not sync `.bib` additions made by external tools)

### Fixed
- Style switcher crash: `apply_style()` held a `state.borrow()` across `buffer.set_text()`, which fired `connect_changed` → `borrow_mut()` → RefCell panic; fixed by cloning the buffer before releasing the borrow
- Outline click did nothing: `row.connect_activate` only fires on Enter/double-click; replaced with `list_box.connect_row_activated` which fires on single click
- `scroll_to_iter` centering: `use_align` was `false`, causing the `yalign: 0.5` argument to be ignored; corrected to `true`

---

## [0.1.0] — 2026-05-24

### Added
- GTK4 + libadwaita window, GtkSourceView editor, live preview via `typst compile` + `pdftoppm`
- Multi-file tabbed editor with modified-indicator dot and close button
- Project file tree (create, delete, click to open)
- Document outline sidebar (heading tree, click to jump)
- Symbol insert panel (Cyrillic, Greek, Hebrew, Sanskrit)
- Citation autocomplete: `@` trigger with BibTeX popup; Tab/Return to accept
- LSP completions: `#` trigger via tinymist; kind badge, Tab/Return to accept
- Find & Replace (`Ctrl+F`): forward/backward search, replace one/all, wrap-around
- Live word count and reading-time estimate in status bar
- Cursor line/column indicator in status bar
- Export: PDF (typst), HTML (typst), DOCX (pandoc)
- Git sync: one-click commit + push; remote setup dialog on first sync
- Help window (Overview, Shortcuts, FAQ, About tabs)
- Settings dialog (appearance, editor, compilation, bibliography)
- Hamburger menu (☰) consolidating settings, help, file operations
- Recent files list in the open dropdown
- Setzer-style open dropdown: search box + work-folder scan (2 levels deep)
- Save / Save As / New Document via native file dialogs
- Desktop integration: `install.sh` / `uninstall.sh`; SVG icon + PNG sizes 16–256 px generated at install time
- tracing-based logging to `~/.local/share/zerkalo/zerkalo.log`
- Global config at `~/.config/zerkalo/config.toml`

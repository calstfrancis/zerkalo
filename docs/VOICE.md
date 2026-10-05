# How Zerkalo talks

Zerkalo is somewhere people write things that matter to them — sermons,
theses, letters — often tired, often anxious about losing work. The words in
the program are part of the comfort. These rules apply to every label, dialog,
toast, tooltip and help page.

1. **Say what is safe first.** A message about something going wrong opens with
   what is fine: "Your work is saved on this computer." Then what happened,
   then what the person can do.
2. **No blame, no alarms.** Avoid *failed*, *error*, *invalid*, *illegal*,
   *cannot be undone*, *discard*, *fatal*. Say "didn't go through",
   "couldn't", "isn't quite right". Use red only when something is truly about
   to be lost; ordinary trouble is amber.
3. **Buttons say what they do.** "Save and close", "Close without saving",
   "Keep editing" — never a bare Cancel / Discard / OK pair for something that
   matters.
4. **Undo beats asking.** If an action can be taken back, do it and offer Undo
   (or keep a saved version) instead of stopping the person with a question.
5. **No jargon on screen.** *root file, LSP, repository, commit, push, regex,
   CSL, compile* belong in "Details" or Help, not in a headline. Use "main
   document", "suggestions", "online backup", "saved version", "pattern",
   "citation style", "preview".
6. **Quiet by default.** Nothing flashes or pulses while someone is writing.
   Good news needs no announcement.
7. **Cutting is editing.** Never show a negative word count or a streak that
   can be "lost".
8. **Sentence case**, plain statements or questions, one idea per sentence.
9. **Raw technical text is for Details**, never the headline. Use
   `crate::friendly::reason` to put a plain sentence in front of it.

`cargo test` enforces a few of these (see `friendly::tests::the_voice_guide_is_kept`).

## Plain-language wording for problems Zerkalo's engine reports.
## Each problem has a headline and a piece of advice; one line each, because
## Fluent keeps line breaks and these are wrapped by the label, not by hand.

diag-fallback = Zerkalo can't make sense of this part
diag-fallback-advice = Look at the underlined spot for a typo or a missing character. Its exact words were: “{ $raw }”. The ⋯ menu can search the Typst forum for them.
diag-empty = Something went wrong while compiling

diag-unknown-variable-named = Zerkalo doesn't know what “{ $name }” means
diag-unknown-variable = Zerkalo doesn't know what that name means
diag-unknown-variable-advice = It's used here but never defined. Check the spelling, or add a definition (#let, which creates a named value) or an import above this point. If you meant to write it as ordinary text rather than a command, remove the # in front of it.

diag-unknown-font-named = The font “{ $name }” isn't installed
diag-unknown-font = That font isn't installed
diag-unknown-font-advice = The document will use a substitute, so the layout may look wrong. Either install the font, or pick another in Template → Body Font.

diag-file-not-found-named = Zerkalo can't find the file “{ $name }”
diag-file-not-found = Zerkalo can't find a file this document uses
diag-file-not-found-advice = Check the name is spelled the same as the real file, and that it sits in the same folder as your document (or that the path in the #include or #image line matches where it actually is).

diag-bibliography-unreadable = Your bibliography file has an entry Zerkalo can't read
diag-bibliography-unreadable-advice = One malformed entry breaks the whole file, not just that entry — which is also why every citation in the document may be showing as “not found” right now; they'll resolve again once this is fixed. A common cause from Zotero/BetterBibTeX exports: a non-numeric year like “Winter 2001” instead of a plain “2001”. The line below points at the exact entry.

diag-package-unavailable = A package this document uses couldn't be downloaded
diag-package-unavailable-advice = Packages download the first time they're used, so this usually means there was no internet connection. Reconnect and compile again.

diag-missing-closer-named = A “{ $thing }” is missing
diag-missing-closer = Something opened here was never closed
diag-missing-closer-advice = Every ( [ { "{" } and " you open has to be closed again. The place marked here is where Zerkalo ran out of document still looking for the closing one.

diag-unexpected-end = The document ends in the middle of something
diag-unexpected-end-advice = A bracket, parenthesis, brace or quotation mark was opened and never closed, so Zerkalo reached the end still waiting for it.

diag-missing-argument-named = This command still needs “{ $what }”
diag-missing-argument = This command is missing something it needs
diag-missing-argument-advice = A command was used without one of the things it needs — for example #image() needs the name of a picture inside the brackets.

diag-unexpected-argument-named = “{ $name }” isn't something this command accepts
diag-unexpected-argument = A command was given something it doesn't take
diag-unexpected-argument-advice = This is usually a misspelled option name (fill: rather than colour:), a missing comma between two values, or one value too many.

diag-missing-label-named = Nothing in the document is labelled “{ $key }”
diag-missing-label = A reference points at something that isn't in the document
diag-missing-label-advice = If this is a citation, check the key matches an entry in your .bib file (your list of citation sources) exactly, and that the file is attached in Settings → Bibliography. If it's a cross-reference, check the <label> it points at is really there and spelled the same. If you just meant to type an @ sign, put a backslash in front of it: \@.

diag-wrong-kind = A value here isn't the kind that was needed
diag-wrong-kind-advice = Words meant as text usually need to be in "quotes", and passages of document content need to be in [square brackets]. A number shouldn't be in quotes.

diag-divide-by-zero = Something here divides by zero
diag-divide-by-zero-advice = Check the value being divided by — it works out to zero.

diag-incomplete-rule = A styling rule here is incomplete
diag-incomplete-rule-advice = Re-apply your style from Template to rewrite these rules, or delete any half-finished #show line (a formatting rule) at this spot.

diag-file-access = This document isn't allowed to read that file
diag-file-access-advice = Zerkalo can only read files inside your project folder. Move the file in beside your document.

## Mistakes that look like ordinary writing but mean something in Typst.

diag-trap-dollar = A dollar sign is being read as the start of a formula
diag-trap-dollar-advice = In Zerkalo a $ opens a maths formula, and a second $ closes it. To write an actual dollar sign, such as in a price, put a backslash in front of it: \$5. If you did mean a formula, add the closing $ where it ends.

diag-trap-star = A “*” was opened but never closed
diag-trap-star-advice = Text between two asterisks becomes bold, so a lone * is waiting for its partner. To write an actual asterisk, put a backslash in front of it: \*. Otherwise add the closing * where the bold text ends.

diag-trap-underscore = An “_” was opened but never closed
diag-trap-underscore-advice = Text between two underscores becomes italic, so a lone _ is waiting for its partner. To write an actual underscore, put a backslash in front of it: \_. Otherwise add the closing _ where the italic text ends.

diag-trap-at = “{ $word }” looks like an email address or handle, but @ starts a reference
diag-trap-at-advice = In Zerkalo, @ followed by a word refers to a citation or a labelled part of the document. To write an actual @ sign, put a backslash in front of it: name\@example.com.

diag-trap-hash = A “#” here isn't followed by anything Zerkalo can use
diag-trap-hash-advice = A # starts a command, such as #image or #let, so it needs a command name straight after it. To write an actual # sign, put a backslash in front of it: \#.

diag-trap-label = A “<” was opened but never closed
diag-trap-label-advice = <name> is a label, so a lone < is waiting for its closing >. To write an actual less-than sign, put a backslash in front of it: \<. Otherwise add the closing > where the label name ends.

diag-expected-expression = Zerkalo expected a value or command here
diag-expected-expression-advice = Something is missing at the marked spot, often the value after an operator such as + or =, or the name of a command after a #. If you meant ordinary text, remove the # or put a backslash in front of it: \#.

## Descriptions of one-click fixes, shown next to the Fix button.

fix-close-brace = Add a closing “{ "}" }” at the end of this line
fix-close-bracket = Add a closing “]” at the end of this line
fix-close-paren = Add a closing “)” at the end of this line
fix-close-here = Add the missing closing character at the end of this line
fix-plain-text = Show the # as ordinary text instead of a command
fix-escape-dollar = Show the $ as an ordinary dollar sign
fix-escape-star = Show the * as an ordinary asterisk
fix-escape-underscore = Show the _ as an ordinary underscore
fix-escape-label = Show the < as an ordinary less-than sign
fix-escape-at = Show the @ as an ordinary @ sign
fix-escape-hash = Show the # as an ordinary # sign
fix-close-all = Add the missing closing brackets at the end of the document

diag-unclosed-string = A closing quotation mark is missing
diag-unclosed-string-advice = A piece of text in quotes was opened but never closed. Add the closing " at the end of it.

diag-stray-closing = There's a “{ $thing }” with nothing to close
diag-stray-closing-advice = Delete this one, or add the matching opening symbol earlier in the text.

diag-stray-punctuation-named = Zerkalo didn't expect this { $thing }
diag-stray-punctuation = Zerkalo didn't expect this
diag-stray-punctuation-advice = It's probably left over or in the wrong place. Try deleting it, and check the commas and brackets around it.

diag-missing-separator = Two things need to be on separate lines
diag-missing-separator-advice = Zerkalo can't tell where the first one ends. Put a line break between them.

diag-expected-name = A name is missing here
diag-expected-name-advice = A command such as #let or #set needs a name right after it, and there isn't one at this spot.

diag-mixed-types = A calculation here mixes different kinds of values
diag-mixed-types-advice = For example a number and some text, which can't be added or compared. Check both sides of the sign.

diag-no-such-field = Something here asks for a part that isn't there
diag-no-such-field-advice = Check the spelling of the word after the dot, and that the value it belongs to is the kind you expect.

diag-out-of-range = The document asks for an item beyond the end of a list
diag-out-of-range-advice = The number after "at" is bigger than the list is long. Lists start counting at 0.

diag-cannot-loop = This can't be repeated over
diag-cannot-loop-advice = A "for" needs a list, a range, some text or a set of named values to go through, and got something else.

diag-panic = The document stopped itself on purpose
diag-panic-advice-named = The document or its template reports: “{ $message }”
diag-panic-advice = A template or the document has a check that failed. Its own message should explain what to change.

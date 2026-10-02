//! When typing a bracket or quote should also type its partner.
//!
//! Pairing blindly is wrong as often as it is right: typing `(` in front of
//! existing text, or `"` to close a quotation, leaves a stray character behind
//! for the writer to delete. The decision here looks at what surrounds the
//! cursor instead. It is pure text-in, verdict-out, so it can be tested
//! without a window.

/// What to do with a typed bracket or quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Insert the character and its partner, cursor between them.
    Pair,
    /// Insert just the character, as if there were no pairing at all.
    Single,
    /// The partner is already in front of the cursor and is the one this
    /// character would close: step over it instead of typing a second one.
    SkipOver,
}

/// The closing partner of an opening character, if `c` opens a pair.
pub fn partner(c: char) -> Option<char> {
    match c {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '$' => Some('$'),
        _ => None,
    }
}

/// Whether typing a bracket right in front of `next` is fine: the next
/// character is nothing, a gap, a closing bracket or sentence punctuation. If
/// it is anything else the writer is wrapping or prefixing existing text, and
/// closing the pair for them would land the partner in the wrong place.
fn is_open_air(next: Option<char>) -> bool {
    match next {
        None => true,
        Some(c) => {
            c.is_whitespace() || matches!(c, ')' | ']' | '}' | ',' | ';' | '.' | ':' | '!' | '?')
        }
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

/// Number of `target` characters in `text` that are not escaped by a backslash.
fn count_unescaped(text: &str, target: char) -> usize {
    let mut n = 0;
    let mut escaped = false;
    for c in text.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == target {
            n += 1;
        }
    }
    n
}

/// How many `open` brackets in `text` have not been closed by `close`.
fn unclosed(text: &str, open: char, close: char) -> usize {
    let mut depth = 0usize;
    let mut escaped = false;
    for c in text.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == open {
            depth += 1;
        } else if c == close {
            depth = depth.saturating_sub(1);
        }
    }
    depth
}

/// Decide what to do when `typed` is pressed.
///
/// `before` is the text leading up to the cursor — the current paragraph, which
/// is as far back as an unclosed bracket or quote is worth hunting for.
/// `next` is the character straight after the cursor, `None` at the end of a
/// line or of the file.
pub fn decide(typed: char, before: &str, next: Option<char>) -> Action {
    let prev = before.chars().next_back();
    let escaped = before.chars().rev().take_while(|&c| c == '\\').count() % 2 == 1;

    match typed {
        ')' | ']' | '}' => {
            let open = match typed {
                ')' => '(',
                ']' => '[',
                _ => '{',
            };
            // Only step over a closer that is really the partner of something
            // still open. Otherwise the writer is typing a bracket of their
            // own and has to get it.
            if next == Some(typed) && !escaped && unclosed(before, open, typed) > 0 {
                Action::SkipOver
            } else {
                Action::Single
            }
        }
        '(' | '[' | '{' => {
            if escaped || !is_open_air(next) {
                Action::Single
            } else {
                Action::Pair
            }
        }
        '"' | '$' => {
            if escaped {
                return Action::Single;
            }
            // An odd count means one is already open, so this one closes it.
            let scope = if typed == '"' {
                // A string does not run past its own line.
                before.rsplit('\n').next().unwrap_or(before)
            } else {
                before
            };
            if count_unescaped(scope, typed) % 2 == 1 {
                if next == Some(typed) {
                    Action::SkipOver
                } else {
                    Action::Single
                }
            } else if prev.is_some_and(is_word) || !is_open_air(next) {
                // Opening glued to the word before it (`5"`, `word$`) or to
                // the one after it is a lone mark, not the start of a pair.
                Action::Single
            } else {
                Action::Pair
            }
        }
        _ => Action::Single,
    }
}

/// Whether backspace between `prev` and `next` should remove both: they are an
/// empty pair, which is nearly always one this module just made.
pub fn is_empty_pair(prev: char, next: char) -> bool {
    partner(prev) == Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_opening_bracket_in_open_air_is_paired() {
        assert_eq!(decide('(', "", None), Action::Pair);
        assert_eq!(decide('(', "foo ", None), Action::Pair);
        assert_eq!(decide('[', "see ", Some(' ')), Action::Pair);
        assert_eq!(decide('{', "#let x = ", Some('\n')), Action::Pair);
    }

    #[test]
    fn a_call_paren_glued_to_a_name_still_pairs() {
        assert_eq!(decide('(', "#text", None), Action::Pair);
    }

    #[test]
    fn an_opening_bracket_in_front_of_text_is_alone() {
        // Backtracking to put the left half in front of words already written.
        assert_eq!(decide('(', "", Some('w')), Action::Single);
        assert_eq!(decide('[', "see ", Some('#')), Action::Single);
        assert_eq!(decide('(', "a ", Some('(')), Action::Single);
        assert_eq!(decide('"', "a ", Some('w')), Action::Single);
    }

    #[test]
    fn an_opening_bracket_before_closing_punctuation_still_pairs() {
        assert_eq!(decide('(', "f(", Some(')')), Action::Pair);
        assert_eq!(decide('(', "so ", Some('.')), Action::Pair);
        assert_eq!(decide('[', "a ", Some(',')), Action::Pair);
    }

    #[test]
    fn a_bracket_after_a_backslash_is_literal() {
        assert_eq!(decide('(', "a \\", None), Action::Single);
        assert_eq!(decide('"', "say \\", None), Action::Single);
        // ...but not when the backslash is itself escaped.
        assert_eq!(decide('(', "a \\\\", None), Action::Pair);
    }

    #[test]
    fn a_closer_steps_over_the_partner_of_an_open_bracket() {
        assert_eq!(decide(')', "f(x", Some(')')), Action::SkipOver);
        assert_eq!(decide(']', "[a\n\nb", Some(']')), Action::SkipOver);
        assert_eq!(decide('}', "{ a ", Some('}')), Action::SkipOver);
    }

    #[test]
    fn a_closer_with_nothing_open_is_just_typed() {
        // The `)` ahead belongs to something earlier; this one is the writer's.
        assert_eq!(decide(')', "x", Some(')')), Action::Single);
        assert_eq!(decide(')', "f(x)", Some(')')), Action::Single);
        assert_eq!(decide(')', "f(x", None), Action::Single);
        assert_eq!(decide(']', "f(x", Some(')')), Action::Single);
    }

    #[test]
    fn nested_brackets_step_out_one_level_at_a_time() {
        assert_eq!(decide(')', "f(g(x", Some(')')), Action::SkipOver);
        assert_eq!(decide(')', "f(g(x)", Some(')')), Action::SkipOver);
        assert_eq!(decide(')', "f(g(x))", Some(')')), Action::Single);
    }

    #[test]
    fn a_quote_in_open_air_opens_a_pair() {
        assert_eq!(decide('"', "", None), Action::Pair);
        assert_eq!(decide('"', "say ", None), Action::Pair);
        assert_eq!(decide('"', "(", Some(')')), Action::Pair);
    }

    #[test]
    fn a_quote_glued_to_a_word_is_a_closing_or_lone_mark() {
        assert_eq!(decide('"', "say \"hello", None), Action::Single);
        assert_eq!(decide('"', "a 5", None), Action::Single);
    }

    #[test]
    fn a_closing_quote_after_punctuation_is_not_doubled() {
        assert_eq!(decide('"', "He said \"stop.", None), Action::Single);
        assert_eq!(decide('"', "He said \"stop!", Some(' ')), Action::Single);
    }

    #[test]
    fn a_quote_steps_over_its_own_closer() {
        assert_eq!(decide('"', "say \"hi", Some('"')), Action::SkipOver);
    }

    #[test]
    fn quotes_on_earlier_lines_do_not_count() {
        assert_eq!(decide('"', "a \"b\"\nc ", None), Action::Pair);
        assert_eq!(decide('"', "a \"unclosed\nc ", None), Action::Pair);
    }

    #[test]
    fn escaped_quotes_do_not_count() {
        assert_eq!(decide('"', "a \\\"b ", None), Action::Pair);
    }

    #[test]
    fn math_dollars_pair_then_close_without_doubling() {
        assert_eq!(decide('$', "", None), Action::Pair);
        assert_eq!(decide('$', "x = ", None), Action::Pair);
        assert_eq!(decide('$', "$x", Some('$')), Action::SkipOver);
        assert_eq!(decide('$', "$x + y", None), Action::Single);
    }

    #[test]
    fn a_dollar_amount_is_not_the_start_of_math() {
        assert_eq!(decide('$', "costs 5", None), Action::Single);
        assert_eq!(decide('$', "a ", Some('5')), Action::Single);
    }

    #[test]
    fn display_math_can_span_lines() {
        assert_eq!(decide('$', "$ x\n+ y ", None), Action::Single);
        assert_eq!(decide('$', "$ x\n+ y ", Some('$')), Action::SkipOver);
    }

    #[test]
    fn ordinary_characters_are_untouched() {
        assert_eq!(decide('a', "x", None), Action::Single);
    }

    #[test]
    fn backspace_removes_an_empty_pair_only() {
        assert!(is_empty_pair('(', ')'));
        assert!(is_empty_pair('"', '"'));
        assert!(is_empty_pair('$', '$'));
        assert!(!is_empty_pair('(', ']'));
        assert!(!is_empty_pair('a', 'a'));
        assert!(!is_empty_pair(')', '('));
    }
}

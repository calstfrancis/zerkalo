//! What a search finds, for the editor's find bar and Find in files alike,
//! so the two always agree.
//!
//! A plain search ignores accents ("zizek" finds "Žižek") unless asked not to,
//! is case-sensitive only when the search has a capital in it, and lets a
//! space match any run of spaces and line breaks, so a phrase still turns up
//! when the source wraps it across two lines. A regular expression is taken
//! exactly as written.

use std::borrow::Cow;

use regex::{Regex, RegexBuilder};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub match_accents: bool,
}

/// One match, in characters (what GTK's text iterators count) and in bytes
/// of the searched text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    pub start: usize,
    pub end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}

pub struct Query {
    re: Regex,
    fold: bool,
    regex: bool,
}

/// The letter under an accent: one character for one, so offsets into the
/// folded text are offsets into the original. Letters with no single-letter
/// plain form (ß, æ) stay as they are.
pub fn fold_char(c: char) -> char {
    if c.is_ascii() {
        return c;
    }
    match c {
        'ł' => return 'l',
        'Ł' => return 'L',
        'đ' => return 'd',
        'Đ' => return 'D',
        'ø' => return 'o',
        'Ø' => return 'O',
        'ı' => return 'i',
        'ħ' => return 'h',
        'Ħ' => return 'H',
        _ => {}
    }
    let mut base = None;
    let mut only_marks = true;
    unicode_normalization::char::decompose_canonical(c, |d| {
        if base.is_none() {
            base = Some(d);
        } else if !is_mark(d) {
            only_marks = false;
        }
    });
    match base {
        Some(b) if b != c && only_marks => b,
        _ => c,
    }
}

fn is_mark(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036f}' | '\u{1ab0}'..='\u{1aff}' | '\u{1dc0}'..='\u{1dff}')
}

pub fn fold(text: &str) -> String {
    text.chars().map(fold_char).collect()
}

impl Query {
    pub fn new(text: &str, options: Options) -> Result<Self, regex::Error> {
        let fold_it = !options.regex && !options.match_accents;
        let body = if options.regex {
            text.to_string()
        } else {
            let text: Cow<str> = if fold_it {
                Cow::Owned(fold(text))
            } else {
                Cow::Borrowed(text)
            };
            let words: Vec<String> = text.split_whitespace().map(regex::escape).collect();
            if words.is_empty() {
                regex::escape(&text)
            } else {
                let mut body = words.join(r"\s+");
                if text.starts_with(char::is_whitespace) {
                    body.insert_str(0, r"\s+");
                }
                if text.ends_with(char::is_whitespace) {
                    body.push_str(r"\s+");
                }
                body
            }
        };
        let pattern = if options.whole_word {
            format!(r"\b(?:{body})\b")
        } else {
            body
        };
        let case_sensitive =
            options.case_sensitive || (!options.regex && text.chars().any(char::is_uppercase));
        let re = RegexBuilder::new(&pattern)
            .case_insensitive(!case_sensitive)
            .build()?;
        Ok(Self {
            re,
            fold: fold_it,
            regex: options.regex,
        })
    }

    /// Every non-empty match in `text`, in order.
    pub fn find(&self, text: &str) -> Vec<Found> {
        let hay: Cow<str> = if self.fold {
            Cow::Owned(fold(text))
        } else {
            Cow::Borrowed(text)
        };
        // Folding keeps the character count, not the byte count, so byte
        // positions in `text` come from its own character positions.
        let bytes: Vec<usize> = if self.fold {
            text.char_indices()
                .map(|(b, _)| b)
                .chain([text.len()])
                .collect()
        } else {
            Vec::new()
        };
        let mut out = Vec::new();
        let (mut hay_byte, mut chars) = (0usize, 0usize);
        for m in self.re.find_iter(&hay) {
            if m.start() == m.end() {
                continue;
            }
            chars += hay[hay_byte..m.start()].chars().count();
            let start = chars;
            chars += hay[m.start()..m.end()].chars().count();
            hay_byte = m.end();
            let (byte_start, byte_end) = if self.fold {
                (bytes[start], bytes[chars])
            } else {
                (m.start(), m.end())
            };
            out.push(Found {
                start,
                end: chars,
                byte_start,
                byte_end,
            });
        }
        out
    }

    /// What `found` in `text` becomes: `with` as typed, or for a regular
    /// expression with `$1`-style groups filled in.
    pub fn replacement(&self, text: &str, found: &Found, with: &str) -> String {
        if !self.regex {
            return with.to_string();
        }
        match self.re.captures_at(text, found.byte_start) {
            Some(caps) if caps.get(0).is_some_and(|m| m.start() == found.byte_start) => {
                let mut out = String::new();
                caps.expand(with, &mut out);
                out
            }
            _ => with.to_string(),
        }
    }

    /// `text` with the given matches replaced.
    pub fn replace_found(&self, text: &str, found: &[Found], with: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        for f in found {
            out.push_str(&text[at..f.byte_start]);
            out.push_str(&self.replacement(text, f, with));
            at = f.byte_end;
        }
        out.push_str(&text[at..]);
        out
    }

    #[cfg(test)]
    pub fn replace_all(&self, text: &str, with: &str) -> (String, usize) {
        let found = self.find(text);
        (self.replace_found(text, &found, with), found.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> Options {
        Options::default()
    }

    fn spans(q: &str, o: Options, text: &str) -> Vec<(usize, usize)> {
        Query::new(q, o)
            .unwrap()
            .find(text)
            .iter()
            .map(|f| (f.start, f.end))
            .collect()
    }

    #[test]
    fn accents_are_ignored_unless_asked_for() {
        let text = "Žižek and Zizek, Gutiérrez";
        assert_eq!(spans("zizek", plain(), text), vec![(0, 5), (10, 15)]);
        assert_eq!(spans("gutierrez", plain(), text), vec![(17, 26)]);
        let exact = Options {
            match_accents: true,
            ..plain()
        };
        assert_eq!(spans("zizek", exact, text), vec![(10, 15)]);
        assert_eq!(spans("Žižek", exact, text), vec![(0, 5)]);
    }

    #[test]
    fn folding_keeps_byte_positions_in_the_original() {
        let text = "a Žižek b";
        let f = Query::new("zizek", plain()).unwrap().find(text);
        assert_eq!(&text[f[0].byte_start..f[0].byte_end], "Žižek");
    }

    #[test]
    fn a_capital_makes_the_search_case_sensitive() {
        let text = "kant Kant KANT";
        assert_eq!(spans("kant", plain(), text).len(), 3);
        assert_eq!(spans("Kant", plain(), text), vec![(5, 9)]);
        let case = Options {
            case_sensitive: true,
            ..plain()
        };
        assert_eq!(spans("kant", case, text), vec![(0, 4)]);
    }

    #[test]
    fn a_space_matches_a_line_break() {
        let text = "keeps running through the\nfield and the  field";
        assert_eq!(spans("the field", plain(), text).len(), 2);
    }

    #[test]
    fn plain_text_is_literal() {
        assert_eq!(spans("a.c", plain(), "abc a.c"), vec![(4, 7)]);
        assert!(Query::new("(", plain()).is_ok());
    }

    #[test]
    fn regex_is_exact_and_fills_in_groups() {
        let o = Options {
            regex: true,
            ..plain()
        };
        assert!(Query::new("(", o).is_err());
        let q = Query::new(r"(\w+)@(\w+)", o).unwrap();
        assert_eq!(q.replace_all("x a@b y", "$2 at $1").0, "x b at a y");
        // No folding in a regex: the pattern means what it says.
        assert!(spans("zizek", o, "Žižek").is_empty());
    }

    #[test]
    fn whole_words_and_empty_matches() {
        let w = Options {
            whole_word: true,
            ..plain()
        };
        assert_eq!(
            spans("the", w, "the other then the"),
            vec![(0, 3), (15, 18)]
        );
        let r = Options {
            regex: true,
            ..plain()
        };
        assert_eq!(spans("x*", r, "abxx"), vec![(2, 4)]);
    }

    #[test]
    fn plain_replacement_is_taken_literally() {
        let (out, n) = Query::new("cost", plain())
            .unwrap()
            .replace_all("cost and Cost", "$5");
        assert_eq!((out.as_str(), n), ("$5 and $5", 2));
    }
}

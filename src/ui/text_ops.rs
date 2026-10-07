//! Pure text rules behind the editor's writing helpers (list continuation,
//! expand-selection, delimiter matching). Offsets are in characters, not bytes,
//! to match GtkTextBuffer.

#[derive(Debug, PartialEq, Eq)]
pub enum Continuation {
    /// Not a list line.
    None,
    /// The item is empty: Enter should end the list instead of adding a marker.
    EndList,
    /// The prefix (indent + marker + space) the next line should start with.
    Next(String),
}

struct ListLine<'a> {
    indent: &'a str,
    marker: String,
    next_marker: String,
    content: &'a str,
}

fn parse_list_line(line: &str) -> Option<ListLine<'_>> {
    let body = line.trim_start_matches([' ', '\t']);
    let indent = &line[..line.len() - body.len()];
    let (marker, next_marker, rest) = if let Some(rest) = body.strip_prefix("- ") {
        ("-".to_string(), "-".to_string(), rest)
    } else if let Some(rest) = body.strip_prefix("+ ") {
        ("+".to_string(), "+".to_string(), rest)
    } else if let Some(rest) = body.strip_prefix("/ ") {
        ("/".to_string(), "/".to_string(), rest)
    } else {
        let digits: String = body.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() || digits.len() > 6 {
            return None;
        }
        let after = &body[digits.len()..];
        let rest = after.strip_prefix(". ")?;
        let n: u64 = digits.parse().ok()?;
        (format!("{digits}."), format!("{}.", n + 1), rest)
    };
    Some(ListLine {
        indent,
        marker,
        next_marker,
        content: rest.trim(),
    })
}

/// What pressing Enter at the end of `line` should do about a list.
pub fn continuation(line: &str) -> Continuation {
    match parse_list_line(line) {
        None => Continuation::None,
        Some(l) if l.content.is_empty() => Continuation::EndList,
        Some(l) => Continuation::Next(format!("{}{} ", l.indent, l.next_marker)),
    }
}

/// Characters of indent + marker + space at the start of a list line.
pub fn list_prefix_chars(line: &str) -> Option<usize> {
    let l = parse_list_line(line)?;
    Some(l.indent.chars().count() + l.marker.chars().count() + 1)
}

/// The prefix itself, for measuring how wide a wrapped line's hanging indent is.
pub fn list_prefix(line: &str) -> Option<String> {
    let n = list_prefix_chars(line)?;
    Some(line.chars().take(n).collect())
}

/// A pasted address that should become a link when it lands on selected text.
pub fn is_bare_url(s: &str) -> bool {
    let s = s.trim();
    (s.starts_with("https://") || s.starts_with("http://"))
        && s.len() > 8
        && !s.chars().any(|c| c.is_whitespace() || c == '"')
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

/// Bracket pairs and Typst markup delimiters (`$`, `"`, `*`, `_`) within one
/// paragraph, as `(open, close)` indices into `chars`.
pub fn delimiter_pairs(chars: &[char]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut stack: Vec<(char, usize)> = Vec::new();
    let mut open_markup: [Option<usize>; 4] = [None; 4];
    let markup = ['$', '"', '*', '_'];
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && chars[i - 1] == '\\' {
            continue;
        }
        match c {
            '(' | '[' | '{' => stack.push((c, i)),
            ')' | ']' | '}' => {
                let want = match c {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                if let Some(pos) = stack.iter().rposition(|(o, _)| *o == want) {
                    pairs.push((stack[pos].1, i));
                    stack.truncate(pos);
                }
            }
            _ => {
                if let Some(k) = markup.iter().position(|m| *m == c) {
                    if c == '_' {
                        let prev = i > 0 && is_word(chars[i - 1]);
                        let next = chars.get(i + 1).is_some_and(|n| is_word(*n));
                        if prev && next {
                            continue;
                        }
                    }
                    match open_markup[k].take() {
                        Some(o) => pairs.push((o, i)),
                        None => open_markup[k] = Some(i),
                    }
                }
            }
        }
    }
    pairs
}

/// The partner of the delimiter at `idx` within its paragraph, if it has one.
pub fn matching_delimiter(text: &str, idx: usize) -> Option<usize> {
    let chars: Vec<char> = text.chars().collect();
    let (ps, pe) = paragraph_bounds(&chars, idx.min(chars.len()));
    let slice = &chars[ps..pe];
    let local = idx.checked_sub(ps)?;
    if !matches!(slice.get(local), Some('$' | '"' | '*' | '_')) {
        return None;
    }
    delimiter_pairs(slice).into_iter().find_map(|(a, b)| {
        if a == local {
            Some(b + ps)
        } else if b == local {
            Some(a + ps)
        } else {
            None
        }
    })
}

fn paragraph_bounds(chars: &[char], at: usize) -> (usize, usize) {
    let at = at.min(chars.len());
    let mut s = at;
    while s > 0 {
        if chars[s - 1] == '\n' && (s < 2 || chars[s - 2] == '\n') && s != at {
            break;
        }
        if s >= 2 && chars[s - 1] == '\n' && chars[s - 2] == '\n' {
            break;
        }
        s -= 1;
    }
    let mut e = at;
    while e < chars.len() {
        if chars[e] == '\n' && chars.get(e + 1) == Some(&'\n') {
            break;
        }
        e += 1;
    }
    (s, e)
}

fn heading_level(line: &[char]) -> Option<usize> {
    let n = line.iter().take_while(|c| **c == '=').count();
    (n > 0 && line.get(n) == Some(&' ')).then_some(n)
}

/// The next larger sensible selection around `start..end`: word, the contents
/// of the enclosing delimiters, the delimiters too, the line, the paragraph,
/// the section, the document.
pub fn expand_selection(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let (start, end) = (start.min(len), end.min(len));
    let mut cands: Vec<(usize, usize)> = Vec::new();

    let mut ws = start;
    while ws > 0 && is_word(chars[ws - 1]) {
        ws -= 1;
    }
    let mut we = end;
    while we < len && is_word(chars[we]) {
        we += 1;
    }
    if we > ws {
        cands.push((ws, we));
    }

    let (ps, pe) = paragraph_bounds(&chars, start);
    for (a, b) in delimiter_pairs(&chars[ps..pe]) {
        cands.push((ps + a + 1, ps + b));
        cands.push((ps + a, ps + b + 1));
    }

    let mut ls = start;
    while ls > 0 && chars[ls - 1] != '\n' {
        ls -= 1;
    }
    let mut le = end;
    while le < len && chars[le] != '\n' {
        le += 1;
    }
    cands.push((ls, le));
    if pe > ps {
        cands.push((ps, pe));
    }

    // Section: from the heading at or above to the next heading of the same or
    // higher rank.
    let mut line_starts = vec![0usize];
    for (i, c) in chars.iter().enumerate() {
        if *c == '\n' {
            line_starts.push(i + 1);
        }
    }
    let line_end = |ls: usize| {
        let mut e = ls;
        while e < len && chars[e] != '\n' {
            e += 1;
        }
        e
    };
    let at_line = line_starts
        .partition_point(|&s| s <= start)
        .saturating_sub(1);
    if let Some((hi, level)) = (0..=at_line).rev().find_map(|i| {
        let ls = line_starts[i];
        heading_level(&chars[ls..line_end(ls)]).map(|l| (i, l))
    }) {
        let sec_end = line_starts[hi + 1..]
            .iter()
            .find(|&&ls| heading_level(&chars[ls..line_end(ls)]).is_some_and(|l| l <= level))
            .map(|&ls| ls.saturating_sub(1))
            .unwrap_or(len);
        let mut sec_end = sec_end;
        while sec_end > line_starts[hi] && chars[sec_end - 1] == '\n' {
            sec_end -= 1;
        }
        cands.push((line_starts[hi], sec_end));
    }
    cands.push((0, len));

    cands
        .into_iter()
        .filter(|&(a, b)| a <= start && b >= end && (a, b) != (start, end) && b > a)
        .min_by_key(|&(a, b)| b - a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullets_and_plus_items_continue_with_the_same_marker() {
        assert_eq!(continuation("- apples"), Continuation::Next("- ".into()));
        assert_eq!(
            continuation("  + nested"),
            Continuation::Next("  + ".into())
        );
    }

    #[test]
    fn numbered_items_count_up() {
        assert_eq!(continuation("3. third"), Continuation::Next("4. ".into()));
    }

    #[test]
    fn term_items_continue_as_terms() {
        assert_eq!(
            continuation("/ Term: text"),
            Continuation::Next("/ ".into())
        );
    }

    #[test]
    fn an_empty_item_ends_the_list() {
        assert_eq!(continuation("- "), Continuation::EndList);
        assert_eq!(continuation("  2. "), Continuation::EndList);
    }

    #[test]
    fn ordinary_lines_are_not_lists() {
        assert_eq!(continuation("Just prose"), Continuation::None);
        assert_eq!(continuation("-no space"), Continuation::None);
        assert_eq!(
            continuation("2024. A year"),
            Continuation::Next("2025. ".into())
        );
        assert_eq!(continuation("1.5 metres"), Continuation::None);
    }

    #[test]
    fn the_hanging_prefix_covers_indent_marker_and_space() {
        assert_eq!(list_prefix("  - item").as_deref(), Some("  - "));
        assert_eq!(list_prefix("10. item").as_deref(), Some("10. "));
        assert_eq!(list_prefix("plain"), None);
    }

    #[test]
    fn urls_are_recognised_only_when_bare() {
        assert!(is_bare_url("https://example.org/a?b=1"));
        assert!(is_bare_url("  http://example.org \n"));
        assert!(!is_bare_url("see https://example.org"));
        assert!(!is_bare_url("example.org"));
        assert!(!is_bare_url("https://"));
    }

    #[test]
    fn markup_delimiters_match_within_a_paragraph() {
        let t = "some *bold* and $x$ here";
        assert_eq!(matching_delimiter(t, 5), Some(10));
        assert_eq!(matching_delimiter(t, 10), Some(5));
        assert_eq!(matching_delimiter(t, 16), Some(18));
        assert_eq!(matching_delimiter(t, 0), None);
    }

    #[test]
    fn an_underscore_inside_a_word_is_not_a_delimiter() {
        assert_eq!(matching_delimiter("snake_case_name", 5), None);
        assert_eq!(matching_delimiter("an _emphasised_ word", 3), Some(14));
    }

    #[test]
    fn delimiters_do_not_pair_across_paragraphs() {
        assert_eq!(matching_delimiter("a *b\n\nc* d", 2), None);
    }

    #[test]
    fn expansion_grows_word_then_contents_then_delimiters_then_line() {
        let t = "intro\n\nsay (hello *big* world) now\n\nend";
        let at = t.find("big").unwrap() + 1;
        let word = expand_selection(t, at, at).unwrap();
        assert_eq!(&t[word.0..word.1], "big");
        let step = expand_selection(t, word.0, word.1).unwrap();
        assert_eq!(&t[step.0..step.1], "*big*");
        let step = expand_selection(t, step.0, step.1).unwrap();
        assert_eq!(&t[step.0..step.1], "hello *big* world");
        let step = expand_selection(t, step.0, step.1).unwrap();
        assert_eq!(&t[step.0..step.1], "(hello *big* world)");
        let step = expand_selection(t, step.0, step.1).unwrap();
        assert_eq!(&t[step.0..step.1], "say (hello *big* world) now");
    }

    #[test]
    fn expansion_reaches_section_then_document() {
        let t = "= One\nalpha\n\nbeta\n\n= Two\ngamma";
        let at = t.find("beta").unwrap() + 1;
        let para = expand_selection(t, at, at).unwrap();
        assert_eq!(&t[para.0..para.1], "beta");
        let sec = expand_selection(t, para.0, para.1).unwrap();
        assert_eq!(&t[sec.0..sec.1], "= One\nalpha\n\nbeta");
        let doc = expand_selection(t, sec.0, sec.1).unwrap();
        assert_eq!(doc, (0, t.chars().count()));
        assert_eq!(expand_selection(t, doc.0, doc.1), None);
    }
}

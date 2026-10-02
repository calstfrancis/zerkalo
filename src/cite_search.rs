//! One search for every place a citation is looked up (the `@` popup, the
//! Citations panel, the References window), so they all find the same things
//! in the same order.
//!
//! A query is a few words in any order. Every word must be found somewhere in
//! the entry's author, title, year, key or journal/publisher, ignoring case
//! and accents — "zizek gender 90" finds Žižek's 1990 book. Entries the
//! document already cites come first, then the best matches (a word that
//! starts the author's name or the key beats one buried in a title).

use std::collections::HashSet;

use crate::bibliography::BibEntry;

/// Lower-cases and strips the accents a library of names is full of, so that
/// "zizek" matches "Žižek" and "gutierrez" matches "Gutiérrez".
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => out.push('a'),
            'ç' | 'ć' | 'č' | 'ĉ' | 'ċ' => out.push('c'),
            'ď' | 'đ' => out.push('d'),
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => out.push('e'),
            'ĝ' | 'ğ' | 'ġ' | 'ģ' => out.push('g'),
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => out.push('i'),
            'ł' | 'ĺ' | 'ļ' | 'ľ' => out.push('l'),
            'ñ' | 'ń' | 'ņ' | 'ň' => out.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => out.push('o'),
            'ŕ' | 'ŗ' | 'ř' => out.push('r'),
            'ś' | 'ŝ' | 'ş' | 'š' => out.push('s'),
            'ţ' | 'ť' => out.push('t'),
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => out.push('u'),
            'ý' | 'ÿ' | 'ŷ' => out.push('y'),
            'ź' | 'ż' | 'ž' => out.push('z'),
            'ß' => out.push_str("ss"),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            // Combining marks left over from decomposed text.
            '\u{0300}'..='\u{036f}' => {}
            _ => out.push(c),
        }
    }
    out
}

fn words(text: &str) -> Vec<String> {
    fold(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// How well `entry` answers `query`; `None` if it doesn't. Lower is better.
pub fn score(entry: &BibEntry, query_words: &[String]) -> Option<u32> {
    if query_words.is_empty() {
        return Some(0);
    }
    let author = words(&entry.author);
    let key = fold(&entry.key);
    let title = words(&entry.title);
    let rest = words(&format!("{} {}", entry.year, entry.venue));
    let mut total = 0;
    for q in query_words {
        let best =
            if author.iter().any(|w| w.starts_with(q.as_str())) || key.starts_with(q.as_str()) {
                0
            } else if title.iter().any(|w| w.starts_with(q.as_str()))
                || rest.iter().any(|w| w.starts_with(q.as_str()))
            {
                1
            } else if key.contains(q.as_str())
                || author.iter().any(|w| w.contains(q.as_str()))
                || title.iter().any(|w| w.contains(q.as_str()))
            {
                2
            } else {
                return None;
            };
        total += best;
    }
    Some(total)
}

/// Indexes into `entries` that match `query`, the document's own citations
/// first and then best match first; entries tying keep their order in the
/// bibliography.
pub fn search(entries: &[BibEntry], query: &str, cited: &HashSet<String>) -> Vec<usize> {
    let q = words(query);
    let mut hits: Vec<(bool, u32, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| score(e, &q).map(|s| (!cited.contains(&e.key), s, i)))
        .collect();
    hits.sort();
    hits.into_iter().map(|(_, _, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, author: &str, title: &str, year: &str, venue: &str) -> BibEntry {
        BibEntry {
            key: key.into(),
            author: author.into(),
            title: title.into(),
            year: year.into(),
            venue: venue.into(),
            ..Default::default()
        }
    }

    fn lib() -> Vec<BibEntry> {
        vec![
            entry(
                "butler1990",
                "Judith Butler",
                "Gender Trouble",
                "1990",
                "Routledge",
            ),
            entry(
                "zizek1989",
                "Slavoj Žižek",
                "The Sublime Object of Ideology",
                "1989",
                "Verso",
            ),
            entry(
                "cone1970",
                "James H. Cone",
                "A Black Theology of Liberation",
                "1970",
                "",
            ),
            entry(
                "gut1971",
                "Gustavo Gutiérrez",
                "Teología de la liberación — A Theology of Liberation",
                "1971",
                "",
            ),
        ]
    }

    fn keys(entries: &[BibEntry], idx: Vec<usize>) -> Vec<&str> {
        idx.into_iter().map(|i| entries[i].key.as_str()).collect()
    }

    #[test]
    fn words_in_any_order_across_fields() {
        let l = lib();
        let none = HashSet::new();
        assert_eq!(keys(&l, search(&l, "butler gender", &none)), ["butler1990"]);
        assert_eq!(
            keys(&l, search(&l, "gender butler 90", &none)),
            ["butler1990"]
        );
        assert_eq!(keys(&l, search(&l, "verso", &none)), ["zizek1989"]);
    }

    #[test]
    fn accents_and_case_are_ignored_both_ways() {
        let l = lib();
        let none = HashSet::new();
        assert_eq!(keys(&l, search(&l, "ZIZEK", &none)), ["zizek1989"]);
        assert_eq!(keys(&l, search(&l, "žižek", &none)), ["zizek1989"]);
        assert_eq!(
            keys(&l, search(&l, "gutierrez teologia", &none)),
            ["gut1971"]
        );
    }

    #[test]
    fn a_comma_form_name_finds_a_first_last_entry() {
        let l = lib();
        let none = HashSet::new();
        assert_eq!(keys(&l, search(&l, "Cone, James", &none)), ["cone1970"]);
    }

    #[test]
    fn every_word_must_match() {
        let l = lib();
        assert!(search(&l, "butler liberation", &HashSet::new()).is_empty());
    }

    #[test]
    fn already_cited_works_come_first_then_better_matches() {
        let l = lib();
        // "liberation" is in two titles; with Cone cited he leads.
        let cited: HashSet<String> = ["cone1970".to_string()].into();
        assert_eq!(
            keys(&l, search(&l, "liberation", &cited)),
            ["cone1970", "gut1971"]
        );
        // Nothing cited: a name beats a word buried in a title.
        let l2 = vec![
            entry("a", "Ann Other", "On Butler", "2000", ""),
            entry("b", "Judith Butler", "Precarious Life", "2004", ""),
        ];
        assert_eq!(
            keys(&l2, search(&l2, "butler", &HashSet::new())),
            ["b", "a"]
        );
    }

    #[test]
    fn an_empty_query_lists_everything_cited_first() {
        let l = lib();
        let cited: HashSet<String> = ["gut1971".to_string()].into();
        let all = keys(&l, search(&l, "", &cited));
        assert_eq!(all[0], "gut1971");
        assert_eq!(all.len(), 4);
    }
}

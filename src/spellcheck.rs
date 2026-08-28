use std::collections::{HashMap, HashSet};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const DICT_DIRS: &[&str] = &[
    "/usr/share/hunspell",
    "/usr/share/myspell",
    "/usr/local/share/hunspell",
    "/usr/local/share/myspell",
];

// ── Core struct ───────────────────────────────────────────────────────────────

pub struct SpellChecker {
    pub languages: Vec<String>,
    pub enabled: bool,
    pub autocorrect: bool,
    /// Words from the global user dictionary plus the session's ad-hoc
    /// "ignore" choices. Kept apart from the project's own list so switching
    /// projects doesn't carry one project's vocabulary into the next.
    ignored: HashSet<String>,
    /// Words from the currently-open project's `.zerkalo/dictionary.dic`.
    project_ignored: HashSet<String>,
    project_dict_path: Option<PathBuf>,
}

fn global_user_dict_path() -> PathBuf {
    let base = shellexpand::tilde("~/.config/zerkalo").into_owned();
    PathBuf::from(base).join("user.dic")
}

fn project_dict_path(project_root: &Path) -> PathBuf {
    project_root.join(".zerkalo").join("dictionary.dic")
}

fn load_dic_words(path: &Path) -> HashSet<String> {
    let mut words = HashSet::new();
    let Ok(content) = std::fs::read_to_string(path) else {
        return words;
    };
    // Hunspell .dic format: first line is word count, then one word per line
    for (i, line) in content.lines().enumerate() {
        let word = line.split('/').next().unwrap_or(line).trim();
        if i == 0 && word.parse::<usize>().is_ok() {
            continue;
        }
        if !word.is_empty() {
            words.insert(word.to_lowercase());
        }
    }
    words
}

fn append_dic_word(path: &Path, word: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // If file doesn't exist, create with header count "0"
    if !path.exists() {
        let _ = std::fs::write(path, "0\n");
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(path) {
        let _ = f.write_all(format!("{}\n", word.to_lowercase()).as_bytes());
    }
}

impl SpellChecker {
    pub fn new(languages: Vec<String>) -> Self {
        let user_path = global_user_dict_path();
        let mut ignored = load_dic_words(&user_path);
        // Legacy fallback: also load old user_dict.txt format
        let legacy = PathBuf::from(shellexpand::tilde("~/.local/share/zerkalo").into_owned())
            .join("user_dict.txt");
        if let Ok(content) = std::fs::read_to_string(&legacy) {
            for word in content.lines() {
                let w = word.trim();
                if !w.is_empty() {
                    ignored.insert(w.to_lowercase());
                }
            }
        }
        Self {
            languages,
            enabled: true,
            autocorrect: false,
            ignored,
            project_ignored: HashSet::new(),
            project_dict_path: None,
        }
    }

    /// Switches to `root`'s project dictionary, *replacing* the previous
    /// project's words rather than accumulating them — otherwise a word added
    /// to one project's dictionary stayed accepted in every project opened
    /// afterwards for the rest of the session.
    pub fn set_project_root(&mut self, root: &Path) {
        let path = project_dict_path(root);
        self.project_ignored = load_dic_words(&path);
        self.project_dict_path = Some(path);
    }

    pub fn primary_language(&self) -> &str {
        self.languages
            .first()
            .map(|s| s.as_str())
            .unwrap_or("en_US")
    }

    pub fn ignore(&mut self, word: &str) {
        self.ignored.insert(word.to_lowercase());
    }

    pub fn add_to_user_dict(&mut self, word: &str) {
        self.ignore(word);
        append_dic_word(&global_user_dict_path(), word);
    }

    pub fn add_to_project_dict(&mut self, word: &str) {
        if let Some(ref path) = self.project_dict_path.clone() {
            self.project_ignored.insert(word.to_lowercase());
            append_dic_word(path, word);
        } else {
            self.add_to_user_dict(word);
        }
    }

    pub fn has_project_dict(&self) -> bool {
        self.project_dict_path.is_some()
    }

    /// Every word to skip: global dictionary, session ignores, and the current
    /// project's dictionary. Callers snapshot this to filter on a worker
    /// thread, so it has to include the project words too.
    pub fn ignored(&self) -> HashSet<String> {
        self.ignored.union(&self.project_ignored).cloned().collect()
    }

    pub fn is_ignored(&self, word: &str) -> bool {
        let lower = word.to_lowercase();
        self.ignored.contains(&lower) || self.project_ignored.contains(&lower)
    }

    /// Check a set of unique words. Returns a map of misspelled word → suggestion list.
    /// A word is considered correct if it passes in ANY of the configured languages.
    #[allow(dead_code)] // multi-language variant; the UI path uses check_words_batch
    pub fn check_unique(&self, unique_words: &[&str]) -> HashMap<String, Vec<String>> {
        let filtered: Vec<&str> = unique_words
            .iter()
            .copied()
            .filter(|w| !self.is_ignored(w))
            .collect();
        if filtered.is_empty() || self.languages.is_empty() {
            return HashMap::new();
        }
        // Start with all words flagged by the primary language (includes suggestions).
        let mut result = check_words_in_language(&filtered, self.primary_language());
        // For each additional language, remove words that pass in that language.
        for lang in self.languages.iter().skip(1) {
            let also_wrong = check_words_in_language(&filtered, lang);
            result.retain(|word, _| also_wrong.contains_key(word));
        }
        result
    }

    /// Return list of dictionary language codes installed on the system.
    /// Requires both halves of the pair — a `.dic` with no matching `.aff`
    /// isn't loadable.
    pub fn available_languages() -> Vec<String> {
        let mut langs = Vec::new();
        for dir in DICT_DIRS {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let s = name.to_string_lossy();
                if let Some(stem) = s.strip_suffix(".dic") {
                    if Path::new(dir).join(format!("{stem}.aff")).exists() {
                        langs.push(stem.to_string());
                    }
                }
            }
        }
        langs.sort();
        langs.dedup();
        langs
    }

    /// Get suggestions for a single word (used by right-click menu).
    /// Uses the primary language for suggestions.
    ///
    /// The first call for a given language loads and parses its dictionary
    /// (real work — the full word list plus every affix rule), so this
    /// still shouldn't be called from the GTK main thread. Use
    /// [`suggestions_for_word`] from a worker instead when the caller is on
    /// the main loop.
    #[allow(dead_code)] // the UI now calls suggestions_for_word off-thread
    pub fn suggestions_for(&self, word: &str) -> Vec<String> {
        if self.is_ignored(word) {
            return Vec::new();
        }
        suggestions_for_word(word, self.primary_language())
    }
}

/// Suggestions for one word in one language, with no borrow of the checker —
/// so it can be sent to a worker thread while the main loop keeps running.
pub fn suggestions_for_word(word: &str, language: &str) -> Vec<String> {
    let words = [word];
    check_words_in_language(&words, language)
        .remove(&word.to_lowercase())
        .unwrap_or_default()
}

// ── Word extraction ───────────────────────────────────────────────────────────

/// Extract prose words from a Typst document, returning
/// `(char_offset_start, char_offset_end, word)` tuples.
/// The offsets correspond to Unicode code-point positions (same unit as
/// `gtk4::TextIter::offset()`).  Markup regions are skipped so spell-check
/// tags land only on prose text.
pub fn extract_words(text: &str) -> Vec<(usize, usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut result = Vec::new();
    let mut i = 0;
    let mut in_raw_block = false;
    let mut in_block_comment = false;
    let mut in_math = false;

    while i < n {
        let c = chars[i];

        // ── raw block ``` ... ``` ──────────────────────────────────────────────
        if in_raw_block {
            if c == '`' && chars.get(i + 1) == Some(&'`') && chars.get(i + 2) == Some(&'`') {
                in_raw_block = false;
                i += 3;
            } else {
                i += 1;
            }
            continue;
        }

        // ── block comment /* ... */ ───────────────────────────────────────────
        if in_block_comment {
            if c == '*' && chars.get(i + 1) == Some(&'/') {
                in_block_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        // ── math $...$ ────────────────────────────────────────────────────────
        if in_math {
            if c == '$' {
                in_math = false;
            }
            i += 1;
            continue;
        }

        // ── open raw block ──────────────────────────────────────────────────
        if c == '`' && chars.get(i + 1) == Some(&'`') && chars.get(i + 2) == Some(&'`') {
            in_raw_block = true;
            i += 3;
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // ── line comment // ──────────────────────────────────────────────────
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // ── block comment /* ─────────────────────────────────────────────────
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            in_block_comment = true;
            i += 2;
            continue;
        }

        // ── inline raw `...` ─────────────────────────────────────────────────
        if c == '`' {
            i += 1;
            while i < n && chars[i] != '`' && chars[i] != '\n' {
                i += 1;
            }
            if i < n && chars[i] == '`' {
                i += 1;
            }
            continue;
        }

        // ── math $...$ ────────────────────────────────────────────────────────
        if c == '$' {
            in_math = true;
            i += 1;
            continue;
        }

        // ── citation @key ────────────────────────────────────────────────────
        if c == '@' {
            i += 1;
            while i < n
                && (chars[i].is_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '-'
                    || chars[i] == ':')
            {
                i += 1;
            }
            continue;
        }

        // ── label <key> ──────────────────────────────────────────────────────
        if c == '<' {
            while i < n && chars[i] != '>' && chars[i] != '\n' {
                i += 1;
            }
            if i < n && chars[i] == '>' {
                i += 1;
            }
            continue;
        }

        // ── hash function #ident(...)[...]{...} ──────────────────────────────
        if c == '#' {
            i += 1;
            // skip identifier
            while i < n
                && (chars[i].is_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '-'
                    || chars[i] == '.')
            {
                i += 1;
            }
            // skip argument groups; [ ] content is prose so we DO NOT skip it here
            // but we must skip ( ) and { }
            while i < n && matches!(chars[i], '(' | '{') {
                i = skip_balanced(&chars, i, n);
            }
            // [ ] content: let the outer loop handle it (it's prose)
            continue;
        }

        // ── heading markers = at line start ───────────────────────────────────
        // Skip only the = markers; the heading text remains for extraction.
        {
            let at_line_start = i == 0 || chars[i - 1] == '\n';
            if at_line_start && c == '=' {
                while i < n && chars[i] == '=' {
                    i += 1;
                }
                if i < n && chars[i] == ' ' {
                    i += 1; // skip the space after ==
                }
                continue; // fall through to normal extraction for rest of line
            }
        }

        // ── collect alphabetic word ───────────────────────────────────────────
        if c.is_alphabetic() {
            let start = i;
            loop {
                if i < n && chars[i].is_alphabetic() {
                    i += 1;
                } else if i < n
                    && matches!(chars[i], '\'' | '\u{2018}' | '\u{2019}')
                    && chars.get(i + 1).is_some_and(|c| c.is_alphabetic())
                {
                    // An apostrophe flanked by letters is part of the word
                    // itself — a contraction ("doesn't") or possessive
                    // ("Cal's") — not a quote mark, so it's kept attached
                    // rather than splitting the word into fragments that
                    // are never real dictionary entries on their own.
                    i += 1;
                } else {
                    break;
                }
            }
            // Accept words of 3+ chars to reduce false positives
            if i - start >= 3 {
                let word: String = chars[start..i].iter().collect();
                result.push((start, i, word));
            }
            continue;
        }

        i += 1;
    }

    result
}

fn skip_balanced(chars: &[char], start: usize, n: usize) -> usize {
    let open = chars[start];
    let close = match open {
        '(' => ')',
        '{' => '}',
        '[' => ']',
        _ => return start + 1,
    };
    let mut i = start + 1;
    let mut depth = 1usize;
    while i < n && depth > 0 {
        if chars[i] == open {
            depth += 1;
        } else if chars[i] == close {
            depth -= 1;
        }
        i += 1;
    }
    i
}

// ── Dictionary loading ────────────────────────────────────────────────────────
//
// In-process against the system's own Hunspell-format `.aff`/`.dic` files —
// no `hunspell` binary or subprocess. This used to shell out to `hunspell -a`
// (ispell pipe mode) per batch, which meant a fork/exec per call, a real
// dependency on the CLI tool being on the host's PATH (awkward from inside a
// flatpak sandbox, where reaching a host binary at all needs
// `flatpak-spawn --host`), and made the poll/timing bugs around it (see
// editor_pane.rs's spell-suggestions popover) possible to hit in the first
// place. A dictionary is real work to parse (the full word list plus every
// affix rule), so each one is loaded once per language and cached — every
// caller already runs on its own spawned thread (the background misspelling
// scan, the right-click suggestions popup, autocorrect), so a
// Mutex-guarded cache is enough; nothing here is on the keystroke path.
fn dictionary_cache() -> &'static Mutex<HashMap<String, Option<Arc<spellbook::Dictionary>>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<spellbook::Dictionary>>>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The `.aff`/`.dic` pair for `language` (e.g. `"en_US"`), if both exist in
/// any of `DICT_DIRS`.
fn find_dict_files(language: &str) -> Option<(PathBuf, PathBuf)> {
    for dir in DICT_DIRS {
        let aff = Path::new(dir).join(format!("{language}.aff"));
        let dic = Path::new(dir).join(format!("{language}.dic"));
        if aff.exists() && dic.exists() {
            return Some((aff, dic));
        }
    }
    None
}

fn get_dictionary(language: &str) -> Option<Arc<spellbook::Dictionary>> {
    let mut cache = dictionary_cache().lock().unwrap();
    if let Some(entry) = cache.get(language) {
        return entry.clone();
    }
    let loaded = find_dict_files(language).and_then(|(aff_path, dic_path)| {
        let aff = std::fs::read_to_string(&aff_path).ok()?;
        let dic = std::fs::read_to_string(&dic_path).ok()?;
        match spellbook::Dictionary::new(&aff, &dic) {
            Ok(d) => Some(Arc::new(d)),
            Err(e) => {
                tracing::warn!("Failed to parse dictionary for {language}: {e}");
                None
            }
        }
    });
    cache.insert(language.to_string(), loaded.clone());
    loaded
}

// ── Batch checking ────────────────────────────────────────────────────────────

pub(crate) fn check_words_batch(
    words: &[&str],
    languages: &[String],
) -> HashMap<String, Vec<String>> {
    if words.is_empty() || languages.is_empty() {
        return HashMap::new();
    }
    let mut result = check_words_in_language(words, &languages[0]);
    for lang in languages.iter().skip(1) {
        let also_wrong = check_words_in_language(words, lang);
        result.retain(|word, _| also_wrong.contains_key(word));
    }
    result
}

fn check_words_in_language(words: &[&str], language: &str) -> HashMap<String, Vec<String>> {
    let mut result = HashMap::new();
    if words.is_empty() {
        return result;
    }
    let Some(dict) = get_dictionary(language) else {
        return result;
    };
    for word in words {
        if !dict.check(word) {
            let mut suggestions = Vec::new();
            dict.suggest(word, &mut suggestions);
            suggestions.truncate(8);
            result.insert(word.to_lowercase(), suggestions);
        }
    }
    result
}

// ── Autocorrect helpers ───────────────────────────────────────────────────────

/// Compute simple Levenshtein distance (capped at 3 for performance).
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    if m.abs_diff(n) > 2 {
        return 3; // cap
    }
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    dp[m][n]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        extract_words(text).into_iter().map(|(_, _, w)| w).collect()
    }

    // ── extract_words: prose ─────────────────────────────────────────────────

    #[test]
    fn extracts_plain_words_with_their_offsets() {
        assert_eq!(
            extract_words("the quick fox"),
            vec![
                (0, 3, "the".to_string()),
                (4, 9, "quick".to_string()),
                (10, 13, "fox".to_string()),
            ]
        );
    }

    #[test]
    fn words_shorter_than_three_characters_are_skipped() {
        assert_eq!(words("a an the is are"), vec!["the", "are"]);
    }

    #[test]
    fn empty_and_wordless_input_yields_nothing() {
        for text in ["", "   ", "123 456", "!!! ... ???", "\n\n\n"] {
            assert!(extract_words(text).is_empty(), "text {text:?}");
        }
    }

    #[test]
    fn punctuation_splits_words_without_being_captured() {
        assert_eq!(words("hello, world! yes?"), vec!["hello", "world", "yes"]);
    }

    #[test]
    fn hyphens_split_words_but_apostrophes_stay_attached() {
        assert_eq!(words("well-known"), vec!["well", "known"]);
        // Contractions and possessives keep their apostrophe — checking
        // "don" or "doesn" against the dictionary on its own is never
        // going to be a real word, so splitting on the apostrophe just
        // manufactures a false positive.
        assert_eq!(words("don't"), vec!["don't"]);
        assert_eq!(words("doesn't"), vec!["doesn't"]);
        assert_eq!(words("Cal's book"), vec!["Cal's", "book"]);
        assert_eq!(words("I'll"), vec!["I'll"]);
        // A quote mark isn't part of the word: only an apostrophe with a
        // letter on both sides counts, so a trailing possessive plural or
        // a genuinely quoted word doesn't absorb the punctuation.
        assert_eq!(words("the dogs' toys"), vec!["the", "dogs", "toys"]);
        assert_eq!(words("she said 'hello'"), vec!["she", "said", "hello"]);
    }

    /// Offsets are code-point positions (matching `TextIter::offset()`), not
    /// byte indices — so multi-byte characters earlier in the text must not
    /// shift the offsets of later words.
    #[test]
    fn offsets_are_counted_in_code_points_not_bytes() {
        let extracted = extract_words("naïve théorie");
        assert_eq!(
            extracted,
            vec![(0, 5, "naïve".to_string()), (6, 13, "théorie".to_string())]
        );
        let chars: Vec<char> = "naïve théorie".chars().collect();
        let (start, end, word) = &extracted[1];
        assert_eq!(chars[*start..*end].iter().collect::<String>(), *word);
    }

    #[test]
    fn non_latin_alphabetic_text_is_extracted() {
        assert_eq!(words("Слово Божие"), vec!["Слово", "Божие"]);
    }

    // ── extract_words: Typst markup is skipped ───────────────────────────────

    #[test]
    fn line_and_block_comments_are_skipped() {
        assert_eq!(words("real // ignored\nmore"), vec!["real", "more"]);
        assert_eq!(words("real /* ignored */ more"), vec!["real", "more"]);
        assert_eq!(words("real /* spans\nlines */ more"), vec!["real", "more"]);
    }

    #[test]
    fn inline_and_fenced_raw_blocks_are_skipped() {
        assert_eq!(words("prose `ignored` prose"), vec!["prose", "prose"]);
        assert_eq!(
            words("before\n```rust\nignored code\n```\nafter"),
            vec!["before", "after"]
        );
    }

    #[test]
    fn math_regions_are_skipped() {
        assert_eq!(words("prose $alpha beta$ prose"), vec!["prose", "prose"]);
    }

    #[test]
    fn citations_and_labels_are_skipped() {
        assert_eq!(
            words("see @augustine:confessions here"),
            vec!["see", "here"]
        );
        assert_eq!(words("text <my-label> text"), vec!["text", "text"]);
    }

    #[test]
    fn heading_markers_are_skipped_but_heading_text_is_kept() {
        assert_eq!(words("= Introduction\nbody"), vec!["Introduction", "body"]);
        assert_eq!(words("=== Deep heading"), vec!["Deep", "heading"]);
    }

    /// An `=` mid-line is not a heading marker, so nothing special happens —
    /// it just isn't alphabetic and gets stepped over.
    #[test]
    fn an_equals_sign_mid_line_is_not_treated_as_a_heading() {
        assert_eq!(words("alpha = beta"), vec!["alpha", "beta"]);
    }

    #[test]
    fn hash_function_names_and_their_paren_or_brace_arguments_are_skipped() {
        assert_eq!(words("#figure(image(\"cat.png\")) after"), vec!["after"]);
        assert_eq!(words("#block{ignored code} after"), vec!["after"]);
        assert_eq!(words("#nested(outer(inner)) after"), vec!["after"]);
    }

    /// Brace-skipping is anchored to `#ident` — a `{...}` block that isn't
    /// directly attached to a hash function is walked as ordinary prose.
    #[test]
    fn a_detached_brace_block_is_not_skipped() {
        assert_eq!(
            words("#let value = {contents} after"),
            vec!["value", "contents", "after"]
        );
    }

    /// Bracketed content is prose (`#emph[...]`), so it must still be checked
    /// even though the function name and paren arguments around it are not.
    #[test]
    fn bracketed_content_after_a_hash_function_is_still_prose() {
        assert_eq!(
            words("#emph[emphasised prose]"),
            vec!["emphasised", "prose"]
        );
        assert_eq!(
            words("#text(size: 10pt)[visible words]"),
            vec!["visible", "words"]
        );
    }

    #[test]
    fn offsets_survive_skipped_markup_and_still_index_the_original_text() {
        let text = "// comment\nReformation @luther:1517 followed";
        let chars: Vec<char> = text.chars().collect();
        for (start, end, word) in extract_words(text) {
            assert_eq!(chars[start..end].iter().collect::<String>(), word);
        }
        assert_eq!(words(text), vec!["Reformation", "followed"]);
    }

    #[test]
    fn an_unterminated_raw_block_swallows_the_rest_of_the_text() {
        assert_eq!(words("before ```\nnever closed"), vec!["before"]);
    }

    // ── levenshtein ──────────────────────────────────────────────────────────

    #[test]
    fn identical_strings_have_distance_zero() {
        assert_eq!(levenshtein("word", "word"), 0);
        assert_eq!(levenshtein("", ""), 0);
    }

    #[test]
    fn single_edits_have_distance_one() {
        assert_eq!(levenshtein("cat", "cats"), 1, "insertion");
        assert_eq!(levenshtein("cats", "cat"), 1, "deletion");
        assert_eq!(levenshtein("cat", "bat"), 1, "substitution");
    }

    #[test]
    fn distance_is_symmetric() {
        for (a, b) in [("teh", "the"), ("recieve", "receive"), ("", "abc")] {
            assert_eq!(levenshtein(a, b), levenshtein(b, a), "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn a_transposition_counts_as_two_edits() {
        assert_eq!(levenshtein("teh", "the"), 2);
    }

    #[test]
    fn distance_from_an_empty_string_is_the_other_length_while_within_the_cap() {
        assert_eq!(levenshtein("", "ab"), 2);
        assert_eq!(levenshtein("ab", ""), 2);
    }

    /// The function caps at 3 for performance: any pair whose lengths differ by
    /// more than 2 returns 3 without computing the real distance.
    #[test]
    fn length_differences_beyond_two_short_circuit_to_the_cap() {
        assert_eq!(levenshtein("a", "abcdefgh"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("completely", "different words here"), 3);
    }

    #[test]
    fn levenshtein_counts_code_points_not_bytes() {
        assert_eq!(levenshtein("café", "cafe"), 1);
        assert_eq!(levenshtein("naïve", "naive"), 1);
    }

    // ── Dictionary loading: only the not-found path is CI-safe to test here —
    // CI runners have no hunspell dictionaries installed (nothing in
    // .github/workflows installs one), so a real-dictionary test would be
    // flaky there even though it passes locally (verified by hand against
    // this machine's /usr/share/hunspell/en_US.{aff,dic}: check/suggest both
    // behave as expected, e.g. "wrold" -> ["world", "wold"]).

    #[test]
    fn an_unknown_language_has_no_dictionary_files() {
        assert_eq!(find_dict_files("xx_XX_not_a_real_language"), None);
    }

    #[test]
    fn checking_against_an_unknown_language_returns_nothing_rather_than_panicking() {
        assert!(check_words_in_language(&["word"], "xx_XX_not_a_real_language").is_empty());
        assert!(get_dictionary("xx_XX_not_a_real_language").is_none());
    }
}

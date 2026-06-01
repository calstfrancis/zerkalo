use std::collections::{HashMap, HashSet};
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const DICT_DIRS: &[&str] = &[
    "/usr/share/hunspell",
    "/usr/share/myspell",
    "/usr/local/share/hunspell",
    "/usr/local/share/myspell",
];

// ── Core struct ───────────────────────────────────────────────────────────────

pub struct SpellChecker {
    pub language: String,
    pub enabled: bool,
    pub autocorrect: bool,
    ignored: HashSet<String>,
}

fn user_dict_path() -> PathBuf {
    let base = shellexpand::tilde("~/.local/share/zerkalo").into_owned();
    PathBuf::from(base).join("user_dict.txt")
}

impl SpellChecker {
    pub fn new(language: &str) -> Self {
        let mut ignored = HashSet::new();
        if let Ok(content) = std::fs::read_to_string(user_dict_path()) {
            for word in content.lines() {
                let w = word.trim();
                if !w.is_empty() {
                    ignored.insert(w.to_lowercase());
                }
            }
        }
        Self {
            language: language.to_string(),
            enabled: true,
            autocorrect: false,
            ignored,
        }
    }

    pub fn ignore(&mut self, word: &str) {
        self.ignored.insert(word.to_lowercase());
    }

    pub fn add_to_user_dict(&mut self, word: &str) {
        self.ignore(word);
        let path = user_dict_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = f.write_all(format!("{}\n", word.to_lowercase()).as_bytes());
        }
    }

    pub fn is_ignored(&self, word: &str) -> bool {
        self.ignored.contains(&word.to_lowercase())
    }

    /// Check a set of unique words. Returns a map of misspelled word → suggestion list.
    pub fn check_unique(&self, unique_words: &[&str]) -> HashMap<String, Vec<String>> {
        let filtered: Vec<&str> = unique_words
            .iter()
            .copied()
            .filter(|w| !self.is_ignored(w))
            .collect();
        if filtered.is_empty() {
            return HashMap::new();
        }
        run_hunspell_batch(&filtered, &self.language)
    }

    /// Return list of dictionary language codes installed on the system.
    pub fn available_languages() -> Vec<String> {
        let mut langs = Vec::new();
        for dir in DICT_DIRS {
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let s = name.to_string_lossy();
                if s.ends_with(".dic") {
                    langs.push(s.trim_end_matches(".dic").to_string());
                }
            }
        }
        langs.sort();
        langs.dedup();
        langs
    }

    /// Get suggestions for a single word (used by right-click menu).
    pub fn suggestions_for(&self, word: &str) -> Vec<String> {
        if self.is_ignored(word) {
            return Vec::new();
        }
        let words = [word];
        run_hunspell_batch(&words, &self.language)
            .remove(&word.to_lowercase())
            .unwrap_or_default()
    }
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
            if c == '`'
                && chars.get(i + 1) == Some(&'`')
                && chars.get(i + 2) == Some(&'`')
            {
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
        if c == '`'
            && chars.get(i + 1) == Some(&'`')
            && chars.get(i + 2) == Some(&'`')
        {
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
            while i < n && chars[i].is_alphabetic() {
                i += 1;
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

// ── Hunspell subprocess ───────────────────────────────────────────────────────

fn run_hunspell_batch(words: &[&str], language: &str) -> HashMap<String, Vec<String>> {
    let mut result = HashMap::new();
    if words.is_empty() {
        return result;
    }

    let input = words.join("\n") + "\n";

    let mut child = match Command::new("hunspell")
        .args(["-a", "-d", language])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return result,
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(_) => return result,
    };

    let text = String::from_utf8_lossy(&output.stdout);

    // Collect non-blank lines after the @ header.
    // Each non-blank result line corresponds to one input word in order.
    let result_lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('@'))
        .collect();

    for (idx, line) in result_lines.iter().enumerate() {
        let Some(word) = words.get(idx) else { break };
        if line.starts_with('&') {
            // & word count offset: sugg1, sugg2, ...
            let suggestions: Vec<String> = line
                .find(':')
                .map(|pos| {
                    line[pos + 1..]
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .take(8)
                        .collect()
                })
                .unwrap_or_default();
            result.insert(word.to_lowercase(), suggestions);
        } else if line.starts_with('#') {
            // # word offset — misspelled, no suggestions
            result.insert(word.to_lowercase(), Vec::new());
        }
        // * + - = correct, no entry needed
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
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
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

/// Turn a project name into a safe folder name: lowercase, spaces → hyphens,
/// non-alphanumeric (except hyphens) stripped.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_lowercases_and_hyphenates_spaces() {
        assert_eq!(slugify("My Great Essay"), "my-great-essay");
    }

    #[test]
    fn slugify_collapses_consecutive_separators() {
        assert_eq!(slugify("Foo___Bar  Baz"), "foo-bar-baz");
    }

    #[test]
    fn slugify_strips_leading_trailing_separators() {
        assert_eq!(slugify("  -Hello-  "), "hello");
    }

    #[test]
    fn slugify_empty_input_is_empty() {
        assert_eq!(slugify(""), "");
    }
}

//! Thin wrapper over `fluent-templates` establishing the string-lookup
//! convention every user-facing string should go through. The active locale
//! is read once from the system's `LC_ALL`/`LC_MESSAGES`/`LANG`/`LANGUAGE`
//! environment variables (POSIX precedence order), falling back to `en` if
//! none is set or parseable — see [`current_locale`].
//!
//! Only `locales/en/` exists today, and — as of this writing — only
//! `settings_dialog.rs` has been migrated to call through [`tr`]/[`tr_args`];
//! every other UI module still uses literal strings. Adding a second locale
//! is a `locales/<lang>/` directory plus translating `settings_dialog.ftl`'s
//! keys; a locale the system reports but Zerkalo has no directory for just
//! falls back to `en` via `fallback_language` below, so nothing breaks in
//! the meantime. Migrating the rest of the UI to [`tr`]/[`tr_args`] is
//! separate, much larger work not started yet.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

use fluent_templates::{static_loader, Loader};
use unic_langid::{langid, LanguageIdentifier};

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en",
        // Fluent wraps interpolated variables in invisible bidi-isolation
        // marks (U+2068/U+2069) by default, meant to protect surrounding
        // text when mixing scripts of different directionality. Zerkalo
        // has no RTL layout to protect and those marks would otherwise leak
        // into copy-pasted error text and accessible names, so turn it off.
        customise: |bundle| bundle.set_use_isolating(false),
    };
}

/// The system locale, read once at first use. Checks `LC_ALL`, `LC_MESSAGES`,
/// `LANG`, then `LANGUAGE` (POSIX's precedence order for message locale),
/// taking the first one set to a value fluent-templates' `LanguageIdentifier`
/// parser accepts. POSIX locale strings look like `fr_FR.UTF-8` — the
/// encoding/modifier suffix (after `.` or `@`) is stripped and underscores
/// become hyphens before parsing, since `LanguageIdentifier` speaks BCP-47
/// (`fr-FR`), not POSIX locale syntax.
fn current_locale() -> &'static LanguageIdentifier {
    static LOCALE: OnceLock<LanguageIdentifier> = OnceLock::new();
    LOCALE.get_or_init(|| {
        ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
            .into_iter()
            .find_map(|var| std::env::var(var).ok().and_then(|v| parse_posix_locale(&v)))
            .unwrap_or_else(|| langid!("en"))
    })
}

/// Parses a POSIX locale string (e.g. `fr_FR.UTF-8`, `de_DE@euro`, or bare
/// `ja`) into a BCP-47 [`LanguageIdentifier`] (e.g. `fr-FR`). Returns `None`
/// for `"C"`/`"POSIX"`/empty/anything the identifier parser rejects, so the
/// caller can fall through to the next candidate variable (or the final
/// `en` default).
fn parse_posix_locale(val: &str) -> Option<LanguageIdentifier> {
    let lang_part = val.split(['.', '@']).next().unwrap_or(val);
    // "C" and "POSIX" are the POSIX locale sentinels for "no locale set" —
    // both happen to parse as syntactically-valid (if meaningless) BCP-47
    // language subtags, so they need an explicit reject rather than relying
    // on the parser to fail.
    if lang_part.eq_ignore_ascii_case("C") || lang_part.eq_ignore_ascii_case("POSIX") {
        return None;
    }
    lang_part.replace('_', "-").parse().ok()
}

/// Looks up a user-facing string by its Fluent message ID. Falls back to
/// the ID itself on a miss (a typo'd ID, or a string not yet migrated)
/// rather than panicking — a wrong-looking label in the UI is a bug you
/// can see and fix; a panic on every affected screen is not a trade worth
/// making for translation coverage.
pub fn tr(id: &str) -> String {
    LOCALES
        .try_lookup(current_locale(), id)
        .unwrap_or_else(|| id.to_string())
}

/// Same as [`tr`], with `{ $name }`-style variables filled in from `args`.
pub fn tr_args(id: &str, args: &[(&str, &str)]) -> String {
    let map: HashMap<Cow<'static, str>, fluent_templates::fluent_bundle::FluentValue> = args
        .iter()
        .map(|(k, v)| {
            (
                Cow::Owned(k.to_string()),
                fluent_templates::fluent_bundle::FluentValue::from(*v),
            )
        })
        .collect();
    LOCALES
        .try_lookup_with_args(current_locale(), id, &map)
        .unwrap_or_else(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tr_resolves_a_real_message() {
        assert_eq!(tr("settings-window-title"), "Settings");
    }

    #[test]
    fn tr_falls_back_to_the_id_on_a_miss_instead_of_panicking() {
        assert_eq!(tr("this-id-does-not-exist"), "this-id-does-not-exist");
    }

    #[test]
    fn tr_args_fills_in_a_variable() {
        assert_eq!(
            tr_args("settings-connected-as", &[("username", "alice")]),
            "Connected as alice"
        );
    }

    #[test]
    fn tr_args_falls_back_to_the_id_on_a_miss() {
        assert_eq!(
            tr_args("nonexistent-with-args", &[("x", "1")]),
            "nonexistent-with-args"
        );
    }

    #[test]
    fn parse_posix_locale_strips_encoding_and_modifier_suffixes() {
        assert_eq!(parse_posix_locale("fr_FR.UTF-8").unwrap(), langid!("fr-FR"));
        assert_eq!(parse_posix_locale("de_DE@euro").unwrap(), langid!("de-DE"));
        assert_eq!(parse_posix_locale("ja").unwrap(), langid!("ja"));
    }

    #[test]
    fn parse_posix_locale_rejects_c_and_posix() {
        assert!(parse_posix_locale("C").is_none());
        assert!(parse_posix_locale("POSIX").is_none());
        assert!(parse_posix_locale("").is_none());
    }
}

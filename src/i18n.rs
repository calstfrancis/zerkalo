//! Thin wrapper over `fluent-templates` establishing the string-lookup
//! convention every user-facing string should go through. English is the
//! only locale for now (`locales/en/`) — this phase is infrastructure, not
//! translation content. Adding a second locale later is a `locales/<lang>/`
//! directory plus a locale-selection setting; nothing about the call sites
//! using [`tr`]/[`tr_args`] needs to change.

use std::borrow::Cow;
use std::collections::HashMap;

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

const CURRENT_LOCALE: LanguageIdentifier = langid!("en");

/// Looks up a user-facing string by its Fluent message ID. Falls back to
/// the ID itself on a miss (a typo'd ID, or a string not yet migrated)
/// rather than panicking — a wrong-looking label in the UI is a bug you
/// can see and fix; a panic on every affected screen is not a trade worth
/// making for translation coverage.
pub fn tr(id: &str) -> String {
    LOCALES
        .try_lookup(&CURRENT_LOCALE, id)
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
        .try_lookup_with_args(&CURRENT_LOCALE, id, &map)
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
}

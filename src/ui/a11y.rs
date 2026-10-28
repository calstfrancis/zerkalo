//! Following what the person's system already says about how they need to
//! read and see — contrast and motion — instead of a separate Zerkalo switch
//! they'd have to find.

use gtk4::prelude::*;
use libadwaita as adw;

/// Whether the system has asked for animation to be reduced (GNOME: Settings →
/// Accessibility → Reduce animation).
pub fn reduced_motion() -> bool {
    crate::config::shared().borrow().comfort.reduce_motion
        || SYSTEM_ANIMATIONS.with(|a| a.get()) == Some(false)
        || gtk4::Settings::default().is_some_and(|s| !s.is_gtk_enable_animations())
}

thread_local! {
    static SYSTEM_ANIMATIONS: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Applies the Comfort settings that GTK itself honours: animations off, a
/// steady text cursor, no error bell. Animations are switched off on
/// Zerkalo's own settings object, which stops every CSS transition, revealer
/// slide and libadwaita animation in one place; the desktop's own choice is
/// remembered the first time so it can be put back.
pub fn apply_comfort(c: &crate::config::ComfortPrefs) {
    let Some(s) = gtk4::Settings::default() else {
        return;
    };
    if SYSTEM_ANIMATIONS.with(|a| a.get()).is_none() {
        SYSTEM_ANIMATIONS.with(|a| a.set(Some(s.is_gtk_enable_animations())));
    }
    let system = SYSTEM_ANIMATIONS.with(|a| a.get()).unwrap_or(true);
    s.set_gtk_enable_animations(system && !c.reduce_motion);
    if c.steady_cursor {
        s.set_gtk_cursor_blink(false);
    }
    s.set_gtk_error_bell(false);
}

/// Puts the `high-contrast` class on `window` when either Zerkalo's own
/// setting or the system's high-contrast preference is on.
pub fn apply_high_contrast(window: &impl IsA<gtk4::Widget>, zerkalo_setting: bool) {
    if zerkalo_setting || adw::StyleManager::default().is_high_contrast() {
        window.add_css_class("high-contrast");
    } else {
        window.remove_css_class("high-contrast");
    }
}

#[cfg(test)]
mod tests {
    /// A button that is only an icon has no name for a screen reader unless
    /// someone gives it one. Every icon-only `Button::from_icon_name` in the
    /// interface must set an accessible label.
    #[test]
    fn every_icon_button_has_an_accessible_name() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
        let mut files = Vec::new();
        walk(&root, &mut files);
        let mut missing = Vec::new();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            let body = text.split("#[cfg(test)]").next().unwrap_or(&text);
            let lines: Vec<&str> = body.lines().collect();
            for (i, l) in lines.iter().enumerate() {
                let Some(rest) = l.trim_start().strip_prefix("let ") else {
                    continue;
                };
                if !l.contains("Button::from_icon_name(") {
                    continue;
                }
                let name = rest
                    .trim_start_matches("mut ")
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .next()
                    .unwrap_or("");
                let window = lines[i..(i + 30).min(lines.len())].join("\n");
                let labelled = window.contains(&format!("{name}.update_property"))
                    || window.contains(&format!("{name}\n")) && window.contains(".update_property");
                if !labelled {
                    missing.push(format!("{}:{}: {name}", f.display(), i + 1));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "icon-only buttons with no accessible label:\n{}",
            missing.join("\n")
        );
    }
}

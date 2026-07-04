//! Shared helpers for deriving colors from the active libadwaita theme,
//! for the handful of spots (Pango markup, TextTag properties) that can't
//! reference GTK CSS named colors (`@accent_color`, etc.) directly.

use gtk4::gdk::RGBA;
use gtk4::prelude::*;
use libadwaita as adw;

pub fn is_dark() -> bool {
    adw::StyleManager::default().is_dark()
}

/// Diff-view colors shared by history_panel.rs and snapshot_dialog.rs. These
/// are plain hex (not `@accent_color`-style CSS) because they're applied via
/// `TextTag` "background"/"foreground" properties, not a CssProvider.
pub struct DiffColors {
    pub removed_bg: &'static str,
    pub removed_fg: &'static str,
    pub added_bg: &'static str,
    pub added_fg: &'static str,
    #[allow(dead_code)] // only read by history_panel.rs, which is not currently wired into the app
    pub hunk_fg: &'static str,
}

pub fn diff_colors() -> DiffColors {
    if is_dark() {
        DiffColors {
            removed_bg: "#5c1f1f",
            removed_fg: "#ff9999",
            added_bg: "#1a3a1a",
            added_fg: "#99dd99",
            hunk_fg: "#7aa8d6",
        }
    } else {
        DiffColors {
            removed_bg: "#ffeaea",
            removed_fg: "#9a1010",
            added_bg: "#e6f7e6",
            added_fg: "#116b11",
            hunk_fg: "#3865b0",
        }
    }
}

pub fn rgba_to_hex(c: &RGBA) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c.red() * 255.0).round() as u8,
        (c.green() * 255.0).round() as u8,
        (c.blue() * 255.0).round() as u8
    )
}

/// Resolves a GTK named color (e.g. "error_color", "accent_color") on the given
/// widget's style context to a solid hex string, falling back if unresolved.
pub fn lookup_color_hex(widget: &impl IsA<gtk4::Widget>, name: &str, fallback: &str) -> String {
    widget
        .as_ref()
        .style_context()
        .lookup_color(name)
        .map(|c| rgba_to_hex(&c))
        .unwrap_or_else(|| fallback.to_string())
}

/// Blends window_fg_color into window_bg_color to approximate the "dim-label"
/// muted foreground as a solid hex, since Pango markup can't apply CSS alpha.
pub fn muted_fg_hex(widget: &impl IsA<gtk4::Widget>) -> String {
    let ctx = widget.as_ref().style_context();
    match (
        ctx.lookup_color("window_fg_color"),
        ctx.lookup_color("window_bg_color"),
    ) {
        (Some(fg), Some(bg)) => {
            let a = 0.6f32;
            let blend = |f: f32, b: f32| f * a + b * (1.0 - a);
            format!(
                "#{:02x}{:02x}{:02x}",
                (blend(fg.red(), bg.red()) * 255.0).round() as u8,
                (blend(fg.green(), bg.green()) * 255.0).round() as u8,
                (blend(fg.blue(), bg.blue()) * 255.0).round() as u8
            )
        }
        _ => "#888888".to_string(),
    }
}

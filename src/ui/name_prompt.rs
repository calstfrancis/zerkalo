use std::path::{Path, PathBuf};

use adw::prelude::*;
use gtk4::{Box as GtkBox, Entry, Label, Orientation};
use libadwaita as adw;

// New documents always land in the Zerkalo folder (Library-managed) instead of
// wherever a save dialog pointed — documents saved elsewhere lost access to
// the project's fonts and bibliography.
pub fn document_path_for_name(dir: &Path, name: &str) -> Result<PathBuf, &'static str> {
    let mut name = name.trim();
    let ext_at = name.len().saturating_sub(4);
    if name
        .get(ext_at..)
        .is_some_and(|e| e.eq_ignore_ascii_case(".typ"))
    {
        name = name[..ext_at].trim_end();
    }
    if name.is_empty() {
        return Err("Type a name for the document.");
    }
    if name.starts_with('.') {
        return Err("Names can't start with a dot.");
    }
    if name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) || name.contains('\0') {
        return Err("Names can't contain / \\ : * ? \" < > |");
    }
    let path = dir.join(format!("{name}.typ"));
    if path.exists() {
        return Err("A document with that name already exists.");
    }
    Ok(path)
}

/// Moves a `.typ` file (and its comments/template sidecars, if present) into
/// `dest_dir`, picking a free name with `suggest_free_name` on a collision.
/// Pure filesystem logic, factored out of the Library window's "Move into
/// Zerkalo Folder…" action so it's testable without a GTK display. Returns
/// the new path on success.
pub fn move_into_dir(
    path: &Path,
    dest_dir: &Path,
    sidecar_fns: &[fn(&Path) -> PathBuf],
) -> std::io::Result<PathBuf> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string());
    let name = suggest_free_name(dest_dir, &stem);
    let new_path = dest_dir.join(format!("{name}.typ"));
    std::fs::rename(path, &new_path)?;
    for sidecar_of in sidecar_fns {
        let old_sc = sidecar_of(path);
        if old_sc.is_file() {
            let _ = std::fs::rename(&old_sc, sidecar_of(&new_path));
        }
    }
    Ok(new_path)
}

/// `base` if free, else the first `"base N"` (starting at 2) that doesn't
/// already exist in `dir` — so the name box never opens pre-filled with a
/// name that's immediately rejected as taken.
pub fn suggest_free_name(dir: &Path, base: &str) -> String {
    let base = base.trim();
    let base = if base.is_empty() { "Untitled" } else { base };
    if !dir.join(format!("{base}.typ")).exists() {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base} {n}");
        if !dir.join(format!("{candidate}.typ")).exists() {
            return candidate;
        }
        n += 1;
    }
}

pub fn ask_document_name(
    parent: &impl IsA<gtk4::Window>,
    dir: &Path,
    heading: &str,
    confirm_label: &str,
    suggested: &str,
    on_ok: impl Fn(PathBuf) + 'static,
) {
    let dlg = adw::MessageDialog::new(Some(parent), Some(heading), None);
    dlg.add_response("cancel", "Cancel");
    dlg.add_response("ok", confirm_label);
    dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dlg.set_default_response(Some("ok"));
    dlg.set_close_response("cancel");

    let vbox = GtkBox::new(Orientation::Vertical, 6);
    let entry = Entry::new();
    entry.set_placeholder_text(Some("Document name"));
    entry.set_text(suggested);
    entry.set_activates_default(true);
    entry.set_width_chars(28);
    let hint = Label::new(None);
    hint.add_css_class("caption");
    hint.add_css_class("dim-label");
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    vbox.append(&entry);
    vbox.append(&hint);
    dlg.set_extra_child(Some(&vbox));

    let dir = dir.to_path_buf();
    let validate = {
        let dlg = dlg.clone();
        let hint = hint.clone();
        let dir = dir.clone();
        move |text: &str| match document_path_for_name(&dir, text) {
            Ok(_) => {
                hint.set_text("Saved in your Zerkalo folder and listed in the Library.");
                dlg.set_response_enabled("ok", true);
            }
            Err(msg) => {
                hint.set_text(msg);
                dlg.set_response_enabled("ok", false);
            }
        }
    };
    validate(suggested);
    entry.connect_changed(move |e| validate(&e.text()));

    let entry_c = entry.clone();
    dlg.connect_response(None, move |_, resp| {
        if resp != "ok" {
            return;
        }
        if let Ok(path) = document_path_for_name(&dir, &entry_c.text()) {
            on_ok(path);
        }
    });
    dlg.present();
    entry.grab_focus();
    entry.select_region(0, -1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_typ_and_strips_a_typed_extension() {
        let dir = std::env::temp_dir().join(format!("zerkalo_name_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            document_path_for_name(&dir, " My Essay ").unwrap(),
            dir.join("My Essay.typ")
        );
        assert_eq!(
            document_path_for_name(&dir, "notes.TYP").unwrap(),
            dir.join("notes.typ")
        );
        assert_eq!(
            document_path_for_name(&dir, "Привет").unwrap(),
            dir.join("Привет.typ")
        );
        assert!(document_path_for_name(&dir, "  ").is_err());
        assert!(document_path_for_name(&dir, ".typ").is_err());
        assert!(document_path_for_name(&dir, "../escape").is_err());
        assert!(document_path_for_name(&dir, ".hidden").is_err());
        std::fs::write(dir.join("taken.typ"), "").unwrap();
        assert!(document_path_for_name(&dir, "taken").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn suggest_free_name_finds_the_first_open_slot() {
        let dir = std::env::temp_dir().join(format!("zerkalo_free_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(suggest_free_name(&dir, "Untitled"), "Untitled");
        std::fs::write(dir.join("Untitled.typ"), "").unwrap();
        assert_eq!(suggest_free_name(&dir, "Untitled"), "Untitled 2");
        std::fs::write(dir.join("Untitled 2.typ"), "").unwrap();
        assert_eq!(suggest_free_name(&dir, "Untitled"), "Untitled 3");
        assert_eq!(suggest_free_name(&dir, "  "), "Untitled 3");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn move_into_dir_moves_the_file_and_its_sidecars() {
        let root = std::env::temp_dir().join(format!("zerkalo_move_{}", std::process::id()));
        let src_dir = root.join("src");
        let dest_dir = root.join("dest");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&dest_dir).unwrap();

        let src = src_dir.join("strays.typ");
        std::fs::write(&src, "= Stray\n").unwrap();
        let comments_sc = src_dir.join("strays.comments.toml");
        std::fs::write(&comments_sc, "# comments").unwrap();
        // No .zerkalo.toml sidecar for this doc — the move must not error on
        // a sidecar function whose file doesn't exist.
        fn comments_path(p: &Path) -> PathBuf {
            p.with_extension("comments.toml")
        }
        fn template_path(p: &Path) -> PathBuf {
            p.with_extension("zerkalo.toml")
        }

        let new_path = move_into_dir(&src, &dest_dir, &[comments_path, template_path]).unwrap();

        assert_eq!(new_path, dest_dir.join("strays.typ"));
        assert!(new_path.is_file());
        assert!(!src.exists());
        assert!(dest_dir.join("strays.comments.toml").is_file());
        assert!(!comments_sc.exists());
        assert_eq!(std::fs::read_to_string(&new_path).unwrap(), "= Stray\n");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn move_into_dir_avoids_a_name_collision_at_the_destination() {
        let root =
            std::env::temp_dir().join(format!("zerkalo_move_collide_{}", std::process::id()));
        let src_dir = root.join("src");
        let dest_dir = root.join("dest");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&dest_dir).unwrap();
        let src = src_dir.join("notes.typ");
        std::fs::write(&src, "content").unwrap();
        std::fs::write(dest_dir.join("notes.typ"), "already here").unwrap();

        let new_path = move_into_dir(&src, &dest_dir, &[]).unwrap();

        assert_eq!(new_path, dest_dir.join("notes 2.typ"));
        assert_eq!(
            std::fs::read_to_string(dest_dir.join("notes.typ")).unwrap(),
            "already here"
        );
        assert_eq!(std::fs::read_to_string(&new_path).unwrap(), "content");

        let _ = std::fs::remove_dir_all(&root);
    }
}

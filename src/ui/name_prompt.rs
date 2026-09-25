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
}

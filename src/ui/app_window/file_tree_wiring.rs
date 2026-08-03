//! The file-tree sidebar: opening files, the root-file context menu, and the
//! project-mode toggle with its inline controls. Split out of `AppWindow::new`.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{AlertDialog, Box as GtkBox, Button, Label, Orientation, ToggleButton};
use libadwaita as adw;

use crate::library::Library;
use super::super::editor_pane::EditorPane;
use super::super::file_tree::FileTree;
use super::super::preview_pane::PreviewPane;
use super::compute_include_path;

/// What the file-tree wiring needs from `AppWindow::new`.
pub(super) struct FileTreeCtx {
    pub(super) window: adw::ApplicationWindow,
    pub(super) editor_pane: EditorPane,
    pub(super) preview_pane: PreviewPane,
    pub(super) toast_overlay: adw::ToastOverlay,
    pub(super) project_root: PathBuf,
    pub(super) library: Rc<RefCell<Library>>,
    pub(super) file_title_widget: adw::WindowTitle,
    pub(super) title_extras: GtkBox,
    pub(super) file_tree_holder: Rc<RefCell<Option<FileTree>>>,
    pub(super) configured_root: Rc<RefCell<Option<PathBuf>>>,
    pub(super) proj_mode_active: Rc<Cell<bool>>,
    pub(super) root_banner: Rc<RefCell<Option<adw::Banner>>>,
}

/// Returns the tree itself, which later sections still reference.
pub(super) fn wire_file_tree(ctx: &FileTreeCtx) -> FileTree {
    // ── File tree ────────────────────────────────────────────────────────
    let file_tree = FileTree::new(ctx.project_root.clone());
    {
        let ep = ctx.editor_pane.clone();
        let lib = ctx.library.clone();
        file_tree.set_on_open(move |path| {
            if let Ok(content) = std::fs::read_to_string(&path) {
                ep.open_file(path.clone(), &content);
            }
            lib.borrow_mut().touch_opened(&path).ok();
        });
    }
    {
        let root = ctx.project_root.clone();
        let ft = file_tree.clone();
        let ep = ctx.editor_pane.clone();
        file_tree.set_on_new_file(move |name| {
            let path = root.join(&name);
            if !path.exists() {
                let _ = std::fs::write(&path, "");
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                ep.open_file(path, &content);
            }
            ft.refresh();
        });
    }
    {
        let ft = file_tree.clone();
        let win_for_ft_del = ctx.window.clone();
        file_tree.set_on_delete(move |path| {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("this file")
                .to_string();
            let alert = AlertDialog::builder()
                .modal(true)
                .message("Move to trash?")
                .detail(format!("'{}' will be moved to the system trash.", name))
                .buttons(["Cancel", "Move to Trash"])
                .cancel_button(0)
                .default_button(0)
                .build();
            let ft2 = ft.clone();
            alert.choose(
                Some(&win_for_ft_del),
                None::<&gtk4::gio::Cancellable>,
                move |result| {
                    if result == Ok(1) {
                        let _ = gtk4::gio::File::for_path(&path)
                            .trash(None::<&gtk4::gio::Cancellable>);
                        ft2.refresh();
                    }
                },
            );
        });
    }
    {
        let root = ctx.project_root.clone();
        let ft = file_tree.clone();
        file_tree.set_on_new_folder(move |name| {
            let _ = std::fs::create_dir_all(root.join(&name));
            ft.refresh();
        });
    }
    {
        let root = ctx.project_root.clone();
        let ft = file_tree.clone();
        let ep = ctx.editor_pane.clone();
        file_tree.set_on_new_chapter(move |name| {
            let slug = crate::templates::slugify(&name);
            if slug.is_empty() { return; }
            let filename = format!("{slug}.typ");
            let file_path = root.join(&filename);
            if file_path.exists() { return; }
            let _ = std::fs::write(&file_path, format!("= {name}\n\n"));
            // Insert #include before #bibliography (or at end) in main.typ
            let main_path = root.join("main.typ");
            if main_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&main_path) {
                    let include_line = format!("#include \"{filename}\"");
                    let new_content = if let Some(pos) = content.find("\n#bibliography(") {
                        format!("{}\n{}{}", &content[..pos], include_line, &content[pos..])
                    } else {
                        format!("{}\n{}\n", content.trim_end(), include_line)
                    };
                    let _ = std::fs::write(&main_path, new_content);
                }
            }
            ft.refresh();
            // Open the new chapter file
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                ep.open_file(file_path, &content);
            }
        });
    }
    {
        let ep = ctx.editor_pane.clone();
        let preview = ctx.preview_pane.clone();
        file_tree.set_on_insert_include(move |abs_path| {
            let rel = compute_include_path(&preview, &abs_path);
            ep.insert_at_cursor(&format!("#include \"{rel}\"\n"));
        });
    }
    {
        let ep = ctx.editor_pane.clone();
        let preview = ctx.preview_pane.clone();
        file_tree.set_on_insert_import(move |abs_path| {
            let rel = compute_include_path(&preview, &abs_path);
            let stem = abs_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("*");
            ep.insert_at_cursor(&format!("#import \"{rel}\": {stem}\n"));
        });
    }
    // ── Set / Clear root file via context menu ────────────────────────────
    {
        let preview = ctx.preview_pane.clone();
        let root_ref = ctx.configured_root.clone();
        let root_dir = ctx.project_root.clone();
        let title_w = ctx.file_title_widget.clone();
        let ep_for_root = ctx.editor_pane.clone();
        file_tree.set_on_set_root(move |path| {
            preview.set_root_file(path.clone());
            *root_ref.borrow_mut() = Some(path.clone());
            // Update breadcrumb if there's an active file
            if let Some(active) = ep_for_root.get_active_path() {
                if path != active {
                    let root_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("root");
                    let active_name = active.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                    title_w.set_subtitle(&format!("{root_name} › {active_name}"));
                } else {
                    title_w.set_subtitle("");
                }
            }
            // Save to project config
            let rel = path.strip_prefix(&root_dir).unwrap_or(&path).to_path_buf();
            let mut pcfg = crate::config::ProjectConfig::load(&root_dir).unwrap_or_default();
            pcfg.root_file = Some(rel);
            let _ = pcfg.save(&root_dir);
            preview.trigger_compile();
        });
    }
    {
        let preview = ctx.preview_pane.clone();
        let root_ref = ctx.configured_root.clone();
        let root_dir = ctx.project_root.clone();
        let title_w = ctx.file_title_widget.clone();
        file_tree.set_on_clear_root(move |()| {
            preview.clear_root_file();
            *root_ref.borrow_mut() = None;
            title_w.set_subtitle("");
            // Save to project config
            let mut pcfg = crate::config::ProjectConfig::load(&root_dir).unwrap_or_default();
            pcfg.root_file = None;
            let _ = pcfg.save(&root_dir);
        });
    }
    {
        let editor_for_tab_out = ctx.editor_pane.clone();
        file_tree.set_on_tab_out(move || {
            editor_for_tab_out.grab_focus();
        });
    }

    // ── Project toggle in status bar ─────────────────────────────────────
    //
    // A ToggleButton labelled "project" (default OFF). When toggled ON,
    // inline root-file controls become visible in the status bar.
    {
        let proj_toggle = ToggleButton::new();
        proj_toggle.add_css_class("flat");
        proj_toggle.add_css_class("status-toggle");
        proj_toggle.set_tooltip_text(Some("Toggle project controls (root file)"));
        proj_toggle.update_property(&[gtk4::accessible::Property::Label("Toggle project controls")]);
        proj_toggle.set_active(false);

        let proj_btn_label = Label::new(Some("project"));
        proj_btn_label.set_use_markup(true);
        proj_btn_label.add_css_class("caption");
        proj_btn_label.set_margin_top(3);
        proj_btn_label.set_margin_bottom(3);
        proj_toggle.set_child(Some(&proj_btn_label));

        // ── Inline controls (hidden until toggle is ON) ───────────────────
        let proj_controls = GtkBox::new(Orientation::Horizontal, 4);
        proj_controls.set_visible(false);
        proj_controls.set_margin_start(4);

        let root_value_lbl = Label::new(Some("no root"));
        root_value_lbl.add_css_class("caption");
        root_value_lbl.add_css_class("dim-label");
        root_value_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        root_value_lbl.set_max_width_chars(22);
        proj_controls.append(&root_value_lbl);

        let set_root_btn = Button::with_label("Set\u{2026}");
        set_root_btn.add_css_class("flat");
        set_root_btn.add_css_class("caption");
        proj_controls.append(&set_root_btn);

        // Distinct icons: this one clears the chosen root, the next one
        // puts the whole control away. Two bare ✕ glyphs side by side read
        // as the same button twice.
        let clear_root_btn = Button::from_icon_name("edit-clear-symbolic");
        clear_root_btn.add_css_class("flat");
        clear_root_btn.set_tooltip_text(Some("Clear root file"));
        clear_root_btn.update_property(&[gtk4::accessible::Property::Label("Clear root file")]);
        proj_controls.append(&clear_root_btn);

        // Dismiss: for a one-file document there's no root to pick, and the
        // controls plus the main.typ banner are pure clutter. Shuts them for
        // this project and remembers it; the "project" toggle stays, so one
        // click brings them back.
        let dismiss_root_btn = Button::from_icon_name("ctx.window-close-symbolic");
        dismiss_root_btn.add_css_class("flat");
        dismiss_root_btn.set_tooltip_text(Some(
            "Hide project controls for this document (click \"project\" to bring them back)",
        ));
        dismiss_root_btn.update_property(&[
            gtk4::accessible::Property::Label("Hide project controls"),
        ]);
        proj_controls.append(&dismiss_root_btn);

        // Initialise from current root state
        {
            let root_name = ctx.configured_root.borrow().as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());
            if let Some(name) = root_name {
                root_value_lbl.set_text(&name);
                proj_btn_label.set_markup("<b>project</b>");
                clear_root_btn.set_sensitive(true);
            } else {
                clear_root_btn.set_sensitive(false);
            }
        }

        {
            let ctrls = proj_controls.clone();
            let toggle_c = proj_toggle.clone();
            let banner_rc = ctx.root_banner.clone();
            let root_dir_c = ctx.project_root.clone();
            let toast_c = ctx.toast_overlay.clone();
            let title_c = ctx.file_title_widget.clone();
            dismiss_root_btn.connect_clicked(move |_| {
                toggle_c.set_active(false);
                ctrls.set_visible(false);
                // The "root › file" breadcrumb is part of the same story.
                title_c.set_subtitle("");
                if let Some(b) = banner_rc.borrow().as_ref() {
                    b.set_revealed(false);
                }
                let mut pcfg =
                    crate::config::ProjectConfig::load(&root_dir_c).unwrap_or_default();
                pcfg.root_controls_dismissed = true;
                let _ = pcfg.save(&root_dir_c);
                toast_c.add_toast(adw::Toast::new(
                    "Project controls hidden — click \"project\" to show them again",
                ));
            });
        }

        // Toggle → show/hide inline controls and root banner; update ctx.proj_mode_active
        {
            let ctrls = proj_controls.clone();
            let banner_rc = ctx.root_banner.clone();
            let proj_mode_c = ctx.proj_mode_active.clone();
            proj_toggle.connect_toggled(move |btn| {
                let on = btn.is_active();
                proj_mode_c.set(on);
                ctrls.set_visible(on);
                if let Some(b) = banner_rc.borrow().as_ref() {
                    b.set_revealed(on);
                }
            });
        }

        let root_value_lbl_rc = Rc::new(root_value_lbl);
        let proj_btn_label_rc = Rc::new(proj_btn_label);
        let clear_root_btn_rc = Rc::new(clear_root_btn);

        // "Set…" button
        {
            let win_c = ctx.window.clone();
            let root_dir_c = ctx.project_root.clone();
            let root_ref_c = ctx.configured_root.clone();
            let preview_c = ctx.preview_pane.clone();
            let title_c = ctx.file_title_widget.clone();
            let ep_c = ctx.editor_pane.clone();
            let rvl = root_value_lbl_rc.clone();
            let bll = proj_btn_label_rc.clone();
            let clr = clear_root_btn_rc.clone();
            set_root_btn.connect_clicked(move |_| {
                let dialog = gtk4::FileDialog::new();
                dialog.set_title("Set Root File");
                let filter = gtk4::FileFilter::new();
                filter.set_name(Some("Typst files (*.typ)"));
                filter.add_pattern("*.typ");
                let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
                filters.append(&filter);
                dialog.set_filters(Some(&filters));
                dialog.set_initial_folder(Some(&gtk4::gio::File::for_path(&root_dir_c)));
                let root_dir2 = root_dir_c.clone();
                let root_ref2 = root_ref_c.clone();
                let preview2 = preview_c.clone();
                let title2 = title_c.clone();
                let ep2 = ep_c.clone();
                let rvl2 = rvl.clone();
                let bll2 = bll.clone();
                let clr2 = clr.clone();
                dialog.open(Some(&win_c), None::<&gtk4::gio::Cancellable>, move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            preview2.set_root_file(path.clone());
                            *root_ref2.borrow_mut() = Some(path.clone());
                            if let Some(active) = ep2.get_active_path() {
                                if path != active {
                                    let rn = path.file_name().and_then(|n| n.to_str()).unwrap_or("root");
                                    let an = active.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                                    title2.set_subtitle(&format!("{rn} › {an}"));
                                } else {
                                    title2.set_subtitle("");
                                }
                            }
                            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                            rvl2.set_text(name);
                            bll2.set_markup("<b>project</b>");
                            clr2.set_sensitive(true);
                            let rel = path.strip_prefix(&root_dir2).unwrap_or(&path).to_path_buf();
                            let mut pcfg = crate::config::ProjectConfig::load(&root_dir2).unwrap_or_default();
                            pcfg.root_file = Some(rel);
                            let _ = pcfg.save(&root_dir2);
                            preview2.trigger_compile();
                        }
                    }
                });
            });
        }

        // "✕" clear button
        {
            let root_ref_c = ctx.configured_root.clone();
            let root_dir_c = ctx.project_root.clone();
            let preview_c = ctx.preview_pane.clone();
            let title_c = ctx.file_title_widget.clone();
            let rvl = root_value_lbl_rc.clone();
            let bll = proj_btn_label_rc.clone();
            let clr = clear_root_btn_rc.clone();
            clear_root_btn_rc.connect_clicked(move |_| {
                preview_c.clear_root_file();
                *root_ref_c.borrow_mut() = None;
                title_c.set_subtitle("");
                rvl.set_text("no root");
                bll.set_markup("project");
                clr.set_sensitive(false);
                let mut pcfg = crate::config::ProjectConfig::load(&root_dir_c).unwrap_or_default();
                pcfg.root_file = None;
                let _ = pcfg.save(&root_dir_c);
            });
        }

        // Insert before SIMPLE: toggle first (ends up just left of SIMPLE),
        // then controls (ends up just left of toggle, so: [controls | toggle | SIMPLE]).
        ctx.title_extras.append(&proj_toggle);
        ctx.title_extras.append(&proj_controls);
    }

    // Wire file_tree into the compile-done holder
    *ctx.file_tree_holder.borrow_mut() = Some(file_tree.clone());


    file_tree
}

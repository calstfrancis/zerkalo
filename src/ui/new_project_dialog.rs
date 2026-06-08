use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Orientation, StringList};
use libadwaita as adw;
use adw::prelude::*;

use crate::config::ProjectConfig;
use crate::templates::{slugify, AnyTemplate};

pub struct NewProjectDialog {
    window: adw::Window,
}

impl NewProjectDialog {
    /// `work_dir`: parent folder where the new project subfolder will be created.
    /// `templates`: full list of templates to show (use `templates::all_templates()`).
    /// `on_create`: called with the new project folder path when confirmed.
    pub fn new(
        parent: &impl IsA<gtk4::Window>,
        work_dir: PathBuf,
        templates: Vec<AnyTemplate>,
        on_create: impl Fn(PathBuf) + 'static,
    ) -> Self {
        let window = adw::Window::builder()
            .title("New Project")
            .transient_for(parent)
            .modal(true)
            .default_width(480)
            .resizable(false)
            .build();

        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(true);

        // ── Template list (combo) ──────────────────────────────────────────
        let template_labels: Vec<&str> = templates.iter().map(|t| t.label()).collect();
        let string_list = StringList::new(&template_labels);

        let name_row = adw::EntryRow::new();
        name_row.set_title("Project Name");

        let template_row = adw::ComboRow::new();
        template_row.set_title("Template");
        template_row.set_model(Some(&string_list));

        let group = adw::PreferencesGroup::new();
        group.set_margin_start(16);
        group.set_margin_end(16);
        group.set_margin_top(16);
        group.add(&name_row);
        group.add(&template_row);

        // ── Description label ─────────────────────────────────────────────
        let desc_label = Label::new(templates.first().map(|t| t.description()));
        desc_label.set_wrap(true);
        desc_label.set_xalign(0.0);
        desc_label.add_css_class("dim-label");
        desc_label.set_margin_start(16);
        desc_label.set_margin_end(16);
        desc_label.set_margin_top(4);

        // ── User templates hint ───────────────────────────────────────────
        let user_count = templates.iter().filter(|t| matches!(t, AnyTemplate::User(_))).count();
        let user_hint_text = if user_count > 0 {
            format!("{user_count} custom template(s) loaded from ~/.config/zerkalo/templates/")
        } else {
            "Add custom templates to ~/.config/zerkalo/templates/<name>/".to_string()
        };
        let user_hint = Label::new(Some(&user_hint_text));
        user_hint.set_wrap(true);
        user_hint.set_xalign(0.0);
        user_hint.add_css_class("dim-label");
        user_hint.add_css_class("caption");
        user_hint.set_margin_start(16);
        user_hint.set_margin_end(16);
        user_hint.set_margin_top(2);

        // ── Path preview label ────────────────────────────────────────────
        let path_label = Label::new(None);
        path_label.set_wrap(true);
        path_label.set_xalign(0.0);
        path_label.add_css_class("dim-label");
        path_label.set_margin_start(16);
        path_label.set_margin_end(16);
        path_label.set_margin_top(8);
        path_label.set_margin_bottom(4);
        update_path_label(&path_label, &work_dir, "");

        // ── Buttons ───────────────────────────────────────────────────────
        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.set_hexpand(true);

        let create_btn = Button::with_label("Create Project");
        create_btn.add_css_class("suggested-action");
        create_btn.set_hexpand(true);
        create_btn.set_sensitive(false);

        let btn_row = GtkBox::new(Orientation::Horizontal, 8);
        btn_row.set_margin_start(16);
        btn_row.set_margin_end(16);
        btn_row.set_margin_top(12);
        btn_row.set_margin_bottom(16);
        btn_row.append(&cancel_btn);
        btn_row.append(&create_btn);

        // ── Layout ────────────────────────────────────────────────────────
        let body = GtkBox::new(Orientation::Vertical, 0);
        body.append(&group);
        body.append(&desc_label);
        body.append(&user_hint);
        body.append(&path_label);
        body.append(&btn_row);

        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&header);
        tv.set_content(Some(&body));
        window.set_content(Some(&tv));

        // ── Signals ───────────────────────────────────────────────────────
        let templates = Rc::new(templates);

        {
            let create_btn_c = create_btn.clone();
            let path_label_c = path_label.clone();
            let work_dir_c = work_dir.clone();
            name_row.connect_changed(move |entry| {
                let text = entry.text().to_string();
                let slug = slugify(&text);
                update_path_label(&path_label_c, &work_dir_c, &slug);
                create_btn_c.set_sensitive(!slug.is_empty());
            });
        }

        {
            let desc_label_c = desc_label.clone();
            let templates_c = templates.clone();
            template_row.connect_selected_item_notify(move |row| {
                let idx = row.selected() as usize;
                if let Some(tmpl) = templates_c.get(idx) {
                    desc_label_c.set_text(tmpl.description());
                }
            });
        }

        let win_for_cancel = window.clone();
        cancel_btn.connect_clicked(move |_| win_for_cancel.close());

        let on_create = Rc::new(RefCell::new(Some(on_create)));

        let win_for_create = window.clone();
        let name_row_c = name_row.clone();
        let template_row_c = template_row.clone();
        let work_dir_c = work_dir.clone();
        create_btn.connect_clicked(move |_| {
            let name = name_row_c.text().to_string();
            let slug = slugify(&name);
            if slug.is_empty() { return; }

            let idx = template_row_c.selected() as usize;
            let template = templates.get(idx)
                .cloned()
                .unwrap_or_else(|| crate::templates::builtin_templates().remove(0));

            let project_dir = work_dir_c.join(&slug);

            match create_project(&project_dir, &name, &template) {
                Ok(()) => {
                    win_for_create.close();
                    if let Some(cb) = on_create.borrow_mut().take() {
                        cb(project_dir);
                    }
                }
                Err(e) => {
                    let dlg = adw::MessageDialog::new(
                        Some(&win_for_create),
                        Some("Could not create project"),
                        Some(&e.to_string()),
                    );
                    dlg.add_response("ok", "OK");
                    dlg.present();
                }
            }
        });

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn update_path_label(label: &Label, work_dir: &std::path::Path, slug: &str) {
    if slug.is_empty() {
        label.set_text(&format!("Folder: {}/", work_dir.display()));
    } else {
        label.set_text(&format!("Folder: {}/{}/", work_dir.display(), slug));
    }
}

fn create_project(
    project_dir: &std::path::Path,
    name: &str,
    template: &AnyTemplate,
) -> crate::error::Result<()> {
    if project_dir.exists() {
        return Err(crate::error::ZerkaloError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Folder already exists: {}", project_dir.display()),
        )));
    }

    std::fs::create_dir_all(project_dir)?;
    template.generate(project_dir, name)?;

    let project_config = ProjectConfig {
        root_file: Some(std::path::PathBuf::from(template.root_file())),
        ..Default::default()
    };
    project_config.save(project_dir)?;

    Ok(())
}

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, Spinner};
use libadwaita as adw;
use adw::prelude::*;

pub struct ExportDialog {
    window: adw::Window,
}

impl ExportDialog {
    pub fn new(
        parent: &adw::ApplicationWindow,
        root_file: Option<PathBuf>,
        output_dir: PathBuf,
    ) -> Self {
        let window = adw::Window::new();
        window.set_title(Some("Export"));
        window.set_default_width(360);
        window.set_default_height(-1);
        window.set_transient_for(Some(parent));
        window.set_modal(true);
        window.set_resizable(false);

        let header = adw::HeaderBar::new();

        let content = GtkBox::new(Orientation::Vertical, 0);

        // Format row
        let prefs_group = adw::PreferencesGroup::new();
        prefs_group.set_margin_start(16);
        prefs_group.set_margin_end(16);
        prefs_group.set_margin_top(16);
        prefs_group.set_margin_bottom(8);

        let fmt_row = adw::ComboRow::new();
        fmt_row.set_title("Format");
        let fmt_model = gtk4::StringList::new(&["PDF", "HTML", "DOCX", "ODT", "LaTeX", "EPUB"]);
        fmt_row.set_model(Some(&fmt_model));
        prefs_group.add(&fmt_row);
        content.append(&prefs_group);

        // Status area
        let status_box = GtkBox::new(Orientation::Horizontal, 8);
        status_box.set_halign(Align::Center);
        status_box.set_margin_start(16);
        status_box.set_margin_end(16);
        status_box.set_margin_top(4);
        status_box.set_margin_bottom(4);
        let spinner = Spinner::new();
        let status_lbl = Label::new(None);
        status_lbl.add_css_class("caption");
        status_lbl.add_css_class("dim-label");
        status_box.append(&spinner);
        status_box.append(&status_lbl);
        content.append(&status_box);

        // Export button
        let btn_row = GtkBox::new(Orientation::Horizontal, 0);
        btn_row.set_halign(Align::Center);
        btn_row.set_margin_top(4);
        btn_row.set_margin_bottom(16);
        let export_btn = Button::with_label("Export");
        export_btn.add_css_class("suggested-action");
        export_btn.set_width_request(120);
        btn_row.append(&export_btn);
        content.append(&btn_row);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content));
        window.set_content(Some(&toolbar_view));

        // Wire export button
        {
            let spinner_c = spinner.clone();
            let status_c = status_lbl.clone();
            let fmt_c = fmt_row.clone();
            let out_dir = output_dir.clone();

            export_btn.connect_clicked(move |btn| {
                let Some(ref input) = root_file else {
                    status_c.set_text("No file is currently open.");
                    return;
                };

                let fmt_idx = fmt_c.selected();
                let stem = input
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output")
                    .to_string();

                let input_owned = input.clone();
                let out_dir_owned = out_dir.clone();
                let out_dir_for_open = out_dir.clone();

                spinner_c.start();
                status_c.set_text("Exporting…");
                btn.set_sensitive(false);

                let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
                std::thread::spawn(move || {
                    let val = match fmt_idx {
                        1 => {
                            // HTML via typst CLI (not available via embedded compiler)
                            let out_path = out_dir_owned.join(format!("{stem}.html"));
                            let result = std::process::Command::new("typst")
                                .arg("compile")
                                .arg("--format")
                                .arg("html")
                                .arg(&input_owned)
                                .arg(&out_path)
                                .output();
                            match result {
                                Ok(o) if o.status.success() => Ok(()),
                                Ok(o) => {
                                    let msg = String::from_utf8_lossy(&o.stderr).to_string();
                                    Err(if msg.is_empty() { "HTML export failed".to_string() } else { msg })
                                }
                                Err(_) => Err(
                                    "HTML export requires the typst CLI.\n\
                                     Install it with: cargo install typst-cli".to_string()
                                ),
                            }
                        }
                        2 => {
                            // DOCX via pandoc
                            let out_path = out_dir_owned.join(format!("{stem}.docx"));
                            let result = std::process::Command::new("pandoc")
                                .arg("-f")
                                .arg("typst")
                                .arg(&input_owned)
                                .arg("-o")
                                .arg(&out_path)
                                .output();
                            match result {
                                Ok(o) if o.status.success() => Ok(()),
                                Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
                                Err(_) => Err("pandoc not found. Install it to export DOCX.".to_string()),
                            }
                        }
                        3 => {
                            // ODT via pandoc
                            let out_path = out_dir_owned.join(format!("{stem}.odt"));
                            let result = std::process::Command::new("pandoc")
                                .arg("-f").arg("typst")
                                .arg(&input_owned)
                                .arg("-o").arg(&out_path)
                                .output();
                            match result {
                                Ok(o) if o.status.success() => Ok(()),
                                Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
                                Err(_) => Err("pandoc not found. Install it to export ODT.".to_string()),
                            }
                        }
                        4 => {
                            // LaTeX via pandoc
                            let out_path = out_dir_owned.join(format!("{stem}.tex"));
                            let result = std::process::Command::new("pandoc")
                                .arg("-f").arg("typst")
                                .arg(&input_owned)
                                .arg("-o").arg(&out_path)
                                .output();
                            match result {
                                Ok(o) if o.status.success() => Ok(()),
                                Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
                                Err(_) => Err("pandoc not found. Install it to export LaTeX.".to_string()),
                            }
                        }
                        5 => {
                            // EPUB via pandoc
                            let out_path = out_dir_owned.join(format!("{stem}.epub"));
                            let result = std::process::Command::new("pandoc")
                                .arg("-f").arg("typst")
                                .arg(&input_owned)
                                .arg("-o").arg(&out_path)
                                .output();
                            match result {
                                Ok(o) if o.status.success() => Ok(()),
                                Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
                                Err(_) => Err("pandoc not found. Install it to export EPUB.".to_string()),
                            }
                        }
                        _ => {
                            // PDF via embedded compiler
                            let out_path = out_dir_owned.join(format!("{stem}.pdf"));
                            match crate::compiler::compile_to_pdf_bytes(&input_owned) {
                                Ok(pdf_bytes) => std::fs::write(&out_path, &pdf_bytes)
                                    .map_err(|e| format!("Failed to write PDF: {e}")),
                                Err(e) => Err(e),
                            }
                        }
                    };
                    tx.send(val).ok();
                });

                let rx = Rc::new(rx);
                let btn_p = btn.clone();
                let spinner_p = spinner_c.clone();
                let status_p = status_c.clone();
                glib::timeout_add_local(Duration::from_millis(100), move || {
                    use std::sync::mpsc::TryRecvError;
                    match rx.try_recv() {
                        Ok(Ok(())) => {
                            spinner_p.stop();
                            status_p.set_text("Done. Opening output folder…");
                            std::process::Command::new("xdg-open")
                                .arg(&out_dir_for_open)
                                .spawn()
                                .ok();
                            glib::ControlFlow::Break
                        }
                        Ok(Err(e)) => {
                            spinner_p.stop();
                            let first = e.lines().next().unwrap_or("export failed");
                            status_p.set_text(&format!("Error: {first}"));
                            btn_p.set_sensitive(true);
                            glib::ControlFlow::Break
                        }
                        Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(TryRecvError::Disconnected) => {
                            spinner_p.stop();
                            btn_p.set_sensitive(true);
                            glib::ControlFlow::Break
                        }
                    }
                });
            });
        }

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

//! The tab bar: attaching a page, switching, closing, duplicating and deleting files, and dropping files onto the editor.

use super::*;

impl EditorPane {
    pub fn close_file_if_open(&self, path: &PathBuf) {
        if self.state.borrow().tabs.contains_key(path) {
            self.close_file(path);
        }
    }

    pub fn close_file(&self, path: &PathBuf) {
        // Extract page number and drop the borrow before remove_page, which fires
        // switch_page → connect_switch_page tries state.borrow() → double-borrow panic.
        let page_num = {
            let state = self.state.borrow();
            state
                .tabs
                .get(path)
                .and_then(|t| self.notebook.page_num(&t.notebook_page))
        };
        self.state.borrow_mut().tabs.remove(path);
        if let Some(n) = page_num {
            self.notebook.remove_page(Some(n));
        }
    }

    pub fn switch_to_file(&self, path: &PathBuf) {
        let page = self
            .state
            .borrow()
            .tabs
            .get(path)
            .map(|tab| tab.notebook_page.clone());
        if let Some(n) = page.and_then(|p| self.notebook.page_num(&p)) {
            self.notebook.set_current_page(Some(n));
        }
    }

    pub(super) fn wire_drag_and_drop(&self, view: &View) {
        let drop = DropTarget::new(
            gtk4::gdk::FileList::static_type(),
            gtk4::gdk::DragAction::COPY,
        );
        let on_drop_cb = self.on_image_drop.clone();
        let on_doc_drop_cb = self.on_document_drop.clone();
        drop.connect_drop(move |_, value, _, _| {
            if let Ok(file_list) = value.get::<gtk4::gdk::FileList>() {
                for file in file_list.files() {
                    if let Some(p) = file.path() {
                        let ext = p
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        if matches!(
                            ext.as_str(),
                            "png" | "jpg" | "jpeg" | "svg" | "gif" | "webp"
                        ) {
                            if let Some(f) = on_drop_cb.borrow().as_ref() {
                                f(p);
                            }
                            return true;
                        }
                        if matches!(
                            ext.as_str(),
                            "tex"
                                | "docx"
                                | "md"
                                | "markdown"
                                | "odt"
                                | "html"
                                | "htm"
                                | "epub"
                                | "rtf"
                                | "pdf"
                        ) {
                            if let Some(f) = on_doc_drop_cb.borrow().as_ref() {
                                f(p);
                            }
                            return true;
                        }
                    }
                }
            }
            false
        });
        view.add_controller(drop);
    }

    pub(super) fn attach_tab_page(
        &self,
        path: &Path,
        display_name: &str,
        page_widget: &gtk4::Overlay,
    ) -> (TabMark, TabMark) {
        let view = &self.notebook.view;
        let page = view.append(page_widget);
        page.set_title(display_name);
        page.set_tooltip(&path.display().to_string());
        (TabMark::unsaved(view, &page), TabMark::error(view, &page))
    }

    /// Tab-bar behaviour: a page's state entry goes when the page does, unsaved
    /// changes are confirmed before a user-initiated close, and the right-click
    /// menu (Duplicate / Close / Close Others / Close to the Right / Delete).
    pub(super) fn wire_tab_view(&self) {
        let view = self.notebook.view.clone();

        {
            let state = self.state.clone();
            view.connect_page_detached(move |_, page, _| {
                let child = page.child();
                // Dropped after the borrow ends: destroying a tab's widgets can
                // signal back into code that borrows the state.
                let removed: Vec<EditorTab> = match state.try_borrow_mut() {
                    Ok(mut st) => {
                        let keys: Vec<PathBuf> = st
                            .tabs
                            .iter()
                            .filter(|(_, t)| t.notebook_page.upcast_ref::<gtk4::Widget>() == &child)
                            .map(|(k, _)| k.clone())
                            .collect();
                        keys.iter().filter_map(|k| st.tabs.remove(k)).collect()
                    }
                    Err(_) => Vec::new(),
                };
                drop(removed);
            });
        }

        {
            let ep = self.clone();
            view.connect_close_page(move |_, page| {
                if ep.notebook.bypassing_close() {
                    return glib::Propagation::Proceed;
                }
                let child = page.child();
                let dirty = {
                    let st = ep.state.borrow();
                    st.tabs
                        .iter()
                        .find(|(_, t)| t.notebook_page.upcast_ref::<gtk4::Widget>() == &child)
                        .filter(|(_, t)| t.modified)
                        .map(|(p, t)| (p.clone(), t.display_name.clone()))
                };
                match dirty {
                    Some((path, name)) => {
                        ep.confirm_close_unsaved(page.clone(), path, name);
                        glib::Propagation::Stop
                    }
                    None => glib::Propagation::Proceed,
                }
            });
        }

        let menu = gtk4::gio::Menu::new();
        let dup_section = gtk4::gio::Menu::new();
        dup_section.append(Some("Duplicate"), Some("tab.duplicate"));
        let close_section = gtk4::gio::Menu::new();
        close_section.append(Some("Close"), Some("tab.close"));
        close_section.append(Some("Close Others"), Some("tab.close-others"));
        close_section.append(Some("Close to the Right"), Some("tab.close-right"));
        let delete_section = gtk4::gio::Menu::new();
        delete_section.append(Some("Delete File…"), Some("tab.delete"));
        menu.append_section(None, &dup_section);
        menu.append_section(None, &close_section);
        menu.append_section(None, &delete_section);
        view.set_menu_model(Some(&menu));

        let target: Rc<RefCell<Option<adw::TabPage>>> = Rc::new(RefCell::new(None));
        let group = gtk4::gio::SimpleActionGroup::new();
        let add = |name: &str, f: Box<dyn Fn(&EditorPane, adw::TabPage)>| {
            let action = gtk4::gio::SimpleAction::new(name, None);
            let ep = self.clone();
            let target = target.clone();
            action.connect_activate(move |_, _| {
                let page = target.borrow().clone();
                if let Some(page) = page {
                    f(&ep, page);
                }
            });
            group.add_action(&action);
            action
        };
        add(
            "duplicate",
            Box::new(|ep, page| {
                if let Some(path) = ep.path_for_page(&page) {
                    ep.duplicate_file(&path);
                }
            }),
        );
        add(
            "close",
            Box::new(|ep, page| ep.notebook.view.close_page(&page)),
        );
        let close_others = add(
            "close-others",
            Box::new(|ep, page| ep.notebook.view.close_other_pages(&page)),
        );
        let close_right = add(
            "close-right",
            Box::new(|ep, page| ep.notebook.view.close_pages_after(&page)),
        );
        add(
            "delete",
            Box::new(|ep, page| {
                if let Some(path) = ep.path_for_page(&page) {
                    ep.confirm_delete_file(path);
                }
            }),
        );
        self.notebook.bar.insert_action_group("tab", Some(&group));

        view.connect_setup_menu(move |v, page| {
            *target.borrow_mut() = page.cloned();
            let n = v.n_pages();
            let pos = page.map(|p| v.page_position(p)).unwrap_or(0);
            close_others.set_enabled(n > 1);
            close_right.set_enabled(pos + 1 < n);
        });
    }

    pub(super) fn path_for_page(&self, page: &adw::TabPage) -> Option<PathBuf> {
        let child = page.child();
        self.state
            .borrow()
            .tabs
            .iter()
            .find(|(_, t)| t.notebook_page.upcast_ref::<gtk4::Widget>() == &child)
            .map(|(p, _)| p.clone())
    }

    /// Writes a copy beside the original ("name copy.typ", "name copy 2.typ", …)
    /// from what the tab currently shows, unsaved edits included, and opens it.
    pub(super) fn duplicate_file(&self, path: &Path) {
        let content = {
            let st = self.state.borrow();
            st.tabs.get(path).map(|t| {
                let (s, e) = t.buffer.bounds();
                t.buffer.text(&s, &e, true).to_string()
            })
        };
        let Some(content) = content else { return };
        let stem = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled");
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{e}"))
            .unwrap_or_default();
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let mut copy = dir.join(format!("{stem} copy{ext}"));
        let mut n = 2;
        while copy.exists() {
            copy = dir.join(format!("{stem} copy {n}{ext}"));
            n += 1;
        }
        match crate::error::atomic_write(&copy, content.as_bytes()) {
            Ok(()) => self.open_file(copy, &content),
            Err(e) => self.note_save_result(copy, Some(crate::error::io_reason(&e))),
        }
    }

    pub(super) fn confirm_delete_file(&self, path: PathBuf) {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("this file")
            .to_string();
        let ep = self.clone();
        let cb = self.on_delete_file.clone();
        crate::ui::confirm::confirm_destructive(
            None,
            "Delete this file for good?",
            &format!(
                "'{name}' will be deleted from this computer, and Zerkalo can't bring it back."
            ),
            "Delete for good",
            move || {
                let _ = std::fs::remove_file(&path);
                ep.close_file(&path);
                if let Some(f) = cb.borrow().as_ref() {
                    f(path.clone());
                }
            },
        );
    }

    /// Three-way Save / Discard / Cancel for closing a tab with unsaved edits.
    /// The page stays open until `close_page_finish` says otherwise.
    pub(super) fn confirm_close_unsaved(
        &self,
        page: adw::TabPage,
        path: PathBuf,
        display_name: String,
    ) {
        let parent = self.outer.root().and_downcast::<gtk4::Window>();
        // Three responses, so this one builds its own dialog rather than using
        // the two-button helper in ui::confirm — same AdwMessageDialog either
        // way, so it still matches every other confirmation in the app.
        let alert = adw::MessageDialog::new(
            parent.as_ref(),
            Some(&format!("Save your changes to \u{201c}{display_name}\u{201d}?")),
            Some("If you close without saving, Zerkalo still keeps what you were writing as a saved version in Recover\u{2026}."),
        );
        alert.add_response("cancel", "Keep editing");
        alert.add_response("discard", "Close without saving");
        alert.add_response("save", "Save and close");
        alert.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        alert.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        alert.set_default_response(Some("save"));
        alert.set_close_response("cancel");
        let ep = self.clone();
        alert.connect_response(None, move |_, response| {
            let view = &ep.notebook.view;
            match response {
                "discard" => {
                    if let Some(root) = ep.project_root() {
                        let text = {
                            let st = ep.state.borrow();
                            st.tabs.get(&path).map(|t| {
                                let (s, e) = t.buffer.bounds();
                                t.buffer.text(&s, &e, true).to_string()
                            })
                        };
                        if let Some(text) = text {
                            crate::ui::snapshot_dialog::save_snapshot(&root, &path, &text);
                        }
                    }
                    view.close_page_finish(&page, true)
                }
                "save" => {
                    let content = {
                        let st = ep.state.borrow();
                        st.tabs.get(&path).map(|t| {
                            let (s, e) = t.buffer.bounds();
                            t.buffer.text(&s, &e, true).to_string()
                        })
                    };
                    let write = content
                        .map(|c| crate::error::atomic_write(&path, c.as_bytes()))
                        .unwrap_or(Ok(()));
                    match write {
                        Ok(()) => {
                            crate::auto_save::clear(&path);
                            ep.mark_saved(&path);
                            view.close_page_finish(&page, true);
                        }
                        Err(e) => {
                            ep.note_save_result(path.clone(), Some(crate::error::io_reason(&e)));
                            view.close_page_finish(&page, false);
                        }
                    }
                }
                _ => view.close_page_finish(&page, false),
            }
        });
        alert.present();
    }
}

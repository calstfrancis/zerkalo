//! Constructs the editor, the side panels and the document library window.
//! Split out of `AppWindow::new`.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button};
use libadwaita as adw;

use super::super::citation_panel::CitationPanel;
use super::super::comments_panel::CommentsPanel;
use super::super::dep_graph::DepGraph;
use super::super::editor_pane::EditorPane;
use super::super::library_window::LibraryWindow;
use super::super::outline_panel::OutlinePanel;
use super::super::package_browser::PackageBrowser;
use super::super::preview_pane::PreviewPane;
use super::super::ref_manager::RefManager;
use crate::config::Config;
use crate::library::Library;
use crate::writing_log::{new_file_start_words, FileStartWords, WritingLog};

pub(super) struct Panels {
    pub(super) citation_panel: CitationPanel,
    pub(super) comments_panel: CommentsPanel,
    pub(super) dep_graph: DepGraph,
    pub(super) editor_pane: EditorPane,
    pub(super) file_start_words: FileStartWords,
    pub(super) library_window: LibraryWindow,
    pub(super) outline_panel: OutlinePanel,
    pub(super) package_browser: PackageBrowser,
    pub(super) popout_pane: Rc<RefCell<Option<PreviewPane>>>,
    pub(super) popout_window: Rc<RefCell<Option<adw::Window>>>,
    pub(super) ref_manager: RefManager,
    pub(super) session_start: Rc<RefCell<std::time::Instant>>,
    pub(super) writing_log: Rc<RefCell<WritingLog>>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_panels(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    config: &Config,
    current_config: &Rc<RefCell<Config>>,
    library: &Rc<RefCell<Library>>,
    project_root: &std::path::Path,
    library_btn: &Button,
    style_btn: &Button,
    style_box: &GtkBox,
    style_popover: &gtk4::Popover,
) -> Panels {
    // ── Panels ──────────────────────────────────────────────────────────

    let editor_pane = EditorPane::new();

    let library_window = LibraryWindow::new(app, library.clone(), config.work_dir.clone());
    {
        let ep = editor_pane.clone();
        let win_for_open = window.clone();
        let lib_for_open = library.clone();
        library_window.set_on_open(move |path| {
            if let Ok(content) = std::fs::read_to_string(&path) {
                ep.open_file(path.clone(), &content);
            }
            lib_for_open.borrow_mut().touch_opened(&path).ok();
            win_for_open.present();
        });
    }
    {
        let lw = library_window.clone();
        library_btn.connect_clicked(move |_| lw.toggle());
    }

    let outline_panel = OutlinePanel::new();
    let citation_panel = CitationPanel::new();
    let ref_manager = RefManager::new();
    let dep_graph = DepGraph::new(project_root.to_path_buf());
    let package_browser = PackageBrowser::new();
    let comments_panel = CommentsPanel::new();

    let writing_log: Rc<RefCell<WritingLog>> = Rc::new(RefCell::new(WritingLog::load()));
    let file_start_words = new_file_start_words();
    let session_start: Rc<RefCell<std::time::Instant>> =
        Rc::new(RefCell::new(std::time::Instant::now()));

    // Wire style buttons → editor; update style_btn label to current style name
    {
        let mut child_opt = style_box.first_child();
        for (name, code, bib_style, bib_title, style_key) in crate::styles::STYLES {
            let Some(child) = child_opt else { break };
            let next = child.next_sibling();
            let Some(btn) = child.downcast::<Button>().ok() else {
                child_opt = next;
                continue;
            };
            let ep = editor_pane.clone();
            let pop = style_popover.clone();
            let code_s = code.to_string();
            let bib_s = bib_style.to_string();
            let title_s = bib_title.to_string();
            let key_s = style_key.to_string();
            let sbtn = style_btn.clone();
            let name_s = name.to_string();
            let cfg_for_style = current_config.clone();
            let win_for_style = window.clone();
            btn.connect_clicked(move |_| {
                pop.popdown();
                if bib_s == crate::styles::CUSTOM_STYLE_PLACEHOLDER {
                    let custom_path = cfg_for_style.borrow().custom_csl_path.clone();
                    match custom_path {
                        Some(path) => {
                            ep.apply_style(&code_s, &path.to_string_lossy(), &title_s, &key_s);
                            sbtn.set_label(&name_s);
                        }
                        None => {
                            let dlg = adw::MessageDialog::new(
                                Some(&win_for_style),
                                Some("No custom CSL file configured"),
                                Some(
                                    "Choose a .csl file in Settings before using the Custom style.",
                                ),
                            );
                            dlg.add_response("ok", "OK");
                            dlg.present();
                        }
                    }
                    return;
                }
                ep.apply_style(&code_s, &bib_s, &title_s, &key_s);
                sbtn.set_label(&name_s);
            });
            child_opt = btn.next_sibling();
        }
    }

    // Wire outline symbol insert → editor
    {
        let ep = editor_pane.clone();
        outline_panel.set_on_symbol_insert(move |ch| ep.insert_at_cursor(&ch));
    }

    // Wire outline heading click → jump to line in editor.
    // Defer jump_to_line to idle so all open_file callbacks (page-switch, LSP, etc.)
    // finish before we try to scroll, preventing reentrancy crashes.
    {
        let ep = editor_pane.clone();
        outline_panel.set_on_jump(move |path, line| {
            if let Ok(content) = std::fs::read_to_string(&path) {
                ep.open_file(path.clone(), &content);
            }
            let ep_idle = ep.clone();
            let path_idle = path.clone();
            glib::idle_add_local_once(move || {
                ep_idle.jump_to_line(&path_idle, line);
            });
        });
    }

    // Wire cursor movement → outline auto-select.
    // Preview scrolling is intentionally NOT driven by cursor movement — the
    // preview should only move via its own scrollbar or page-nav buttons.
    {
        let op = outline_panel.clone();
        editor_pane.set_on_cursor_heading(move |path, heading_line| {
            op.select_for_line(&path, heading_line);
        });
    }

    // Set project root for project-wide word count tooltip
    editor_pane.set_project_root(project_root.to_path_buf());

    // Wire dep_graph → open file in editor
    {
        let ep = editor_pane.clone();
        dep_graph.set_on_open(move |path| {
            if let Ok(content) = std::fs::read_to_string(&path) {
                ep.open_file(path, &content);
            }
        });
    }

    // Wire package_browser → insert import at cursor
    {
        let ep = editor_pane.clone();
        package_browser.set_on_insert(move |import| ep.insert_at_cursor(&import));
    }

    // Pop-out preview state
    let popout_window: Rc<RefCell<Option<adw::Window>>> = Rc::new(RefCell::new(None));
    let popout_pane: Rc<RefCell<Option<PreviewPane>>> = Rc::new(RefCell::new(None));

    Panels {
        citation_panel,
        comments_panel,
        dep_graph,
        editor_pane,
        file_start_words,
        library_window,
        outline_panel,
        package_browser,
        popout_pane,
        popout_window,
        ref_manager,
        session_start,
        writing_log,
    }
}

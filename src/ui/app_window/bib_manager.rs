//! The one place that knows which bibliography is in use.
//!
//! Everything that changes it — the document's own `#bibliography(...)` line,
//! Settings, the Citations panel's file / vault / new-file buttons, switching
//! tabs — goes through `BibManager`, and everything that displays it (editor
//! autocomplete, the Citations panel, the References window) is updated from
//! here, together, from one read of the file. It also owns the live watch, and
//! replaces it whenever the source changes, so edits to the *current* file are
//! always picked up and edits to a file no longer in use never are.
//!
//! It only ever reads bibliography files. The one thing it writes is the
//! `#bibliography(...)` path in the open document, and only when the person
//! has just chosen a source.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use crate::bib_source::{self, Origin, Resolved};
use crate::bibliography;
use crate::config::Config;

use super::super::citation_panel::CitationPanel;
use super::super::editor_pane::EditorPane;
use super::super::preview_pane::PreviewPane;
use super::super::ref_manager::RefManager;

enum Watch {
    Poll(Option<glib::SourceId>),
    Vault {
        timer_alive: Rc<std::cell::Cell<bool>>,
        _watch: fond_vault::VaultWatch,
    },
}

impl Drop for Watch {
    fn drop(&mut self) {
        match self {
            Watch::Poll(id) => {
                if let Some(id) = id.take() {
                    id.remove();
                }
            }
            Watch::Vault { timer_alive, .. } => timer_alive.set(false),
        }
    }
}

struct Inner {
    editor: EditorPane,
    panel: CitationPanel,
    refs: RefManager,
    preview: PreviewPane,
    config: Rc<RefCell<Config>>,
    project_root: PathBuf,
    current: RefCell<Option<Resolved>>,
    watch: RefCell<Option<Watch>>,
}

#[derive(Clone)]
pub(super) struct BibManager(Rc<Inner>);

impl BibManager {
    pub(super) fn new(
        editor: EditorPane,
        panel: CitationPanel,
        refs: RefManager,
        preview: PreviewPane,
        config: Rc<RefCell<Config>>,
        project_root: PathBuf,
    ) -> Self {
        Self(Rc::new(Inner {
            editor,
            panel,
            refs,
            preview,
            config,
            project_root,
            current: RefCell::new(None),
            watch: RefCell::new(None),
        }))
    }

    pub(super) fn current(&self) -> Option<Resolved> {
        self.0.current.borrow().clone()
    }

    fn project_bib(&self) -> Option<PathBuf> {
        crate::config::ProjectConfig::load(&self.0.project_root).and_then(|p| p.bib_path)
    }

    /// Decides again which bibliography applies to `doc` (the open file and
    /// its text as typed) and switches to it if that changed. Cheap enough to
    /// call on every tab switch and edit.
    pub(super) fn refresh(&self, doc: Option<(&Path, &str)>) {
        let project = self.project_bib();
        let global = self.0.config.borrow().bib_path.clone();
        let resolved = bib_source::resolve(
            doc.map(|(p, t)| (p, Some(t))),
            project.as_deref(),
            global.as_deref(),
            &self.0.project_root,
        );
        self.use_source(resolved, false);
    }

    fn refresh_for_active(&self) {
        let path = self.0.editor.get_active_path();
        let text = self.0.editor.get_active_content();
        match (path, text) {
            (Some(p), Some(t)) => self.refresh(Some((&p, &t))),
            _ => self.refresh(None),
        }
    }

    /// Settings changed the global bibliography path.
    pub(super) fn settings_changed(&self, global: Option<PathBuf>) {
        self.0.config.borrow_mut().bib_path = global.clone();
        let effective = self.project_bib().or(global);
        self.0.preview.set_bib_path(effective);
        self.refresh_for_active();
    }

    /// The person picked a source (a file, a Kartoteka vault, or a freshly
    /// created file). Remember it, point the open document at it, and switch.
    pub(super) fn choose(&self, path: PathBuf) {
        {
            let mut cfg = self.0.config.borrow_mut();
            cfg.bib_path = Some(path.clone());
            let _ = cfg.save();
        }
        let effective = self.project_bib().unwrap_or_else(|| path.clone());
        self.0.preview.set_bib_path(Some(effective));
        self.point_active_document_at(&path);
        self.refresh_for_active();
        // If the document names a bibliography of its own that still wins
        // (e.g. a list of several files), `refresh` keeps showing that one.
        // With no document open, `refresh` has resolved to the choice above.
    }

    /// Writes the chosen source into the open document's `#bibliography(...)`
    /// call, as one undoable edit. A call it can't safely edit (several
    /// files, no path) is left exactly as written.
    fn point_active_document_at(&self, source: &Path) {
        let (Some(doc), Some(content)) = (
            self.0.editor.get_active_path(),
            self.0.editor.get_active_content(),
        ) else {
            return;
        };
        let target = bib_source::path_for_document(source, &doc);
        let text = if target.is_absolute() {
            target.to_string_lossy().into_owned()
        } else {
            target.to_string_lossy().replace('\\', "/")
        };
        let updated = crate::styles::set_bibliography_path(&content, &text);
        if updated != content {
            self.0.editor.set_active_content_undoable(&updated);
        }
    }

    /// Re-reads the current source (after the app itself wrote to it).
    pub(super) fn reload(&self) {
        let res = self.0.current.borrow().clone();
        if let Some(r) = res {
            self.show(&r.path);
        }
    }

    fn use_source(&self, resolved: Option<Resolved>, force: bool) {
        if !force && *self.0.current.borrow() == resolved {
            return;
        }
        // Replace the watch first: the old file must stop being listened to
        // before the new one is read.
        *self.0.watch.borrow_mut() = None;
        match &resolved {
            Some(r) => {
                self.show(&r.path);
                *self.0.watch.borrow_mut() = Some(self.start_watch(&r.path));
            }
            None => {
                self.0.editor.set_bib_entries(Vec::new());
                self.0.panel.set_bib_problem(None);
                self.0.panel.load_bib(Vec::new());
                self.0.panel.set_bib_filename(None);
                self.0.refs.clear();
            }
        }
        *self.0.current.borrow_mut() = resolved;
    }

    /// One read, shared by every view.
    fn show(&self, path: &Path) {
        let loaded = bibliography::try_load(path);
        let (entries, problem) = match &loaded {
            Ok(entries) => {
                tracing::info!(
                    "Loaded {} bib entries from {}",
                    entries.len(),
                    path.display()
                );
                (entries.clone(), None)
            }
            Err(why) => {
                tracing::warn!("Couldn't load {}: {why}", path.display());
                (Vec::new(), Some(why.clone()))
            }
        };
        self.0.editor.set_bib_entries(entries.clone());
        self.0.panel.set_bib_problem(problem);
        self.0.panel.load_bib(entries);
        self.0
            .panel
            .set_bib_filename(path.file_name().and_then(|n| n.to_str()));
        self.0.refs.set_source(path, loaded);
    }

    fn start_watch(&self, path: &Path) -> Watch {
        let weak = Rc::downgrade(&self.0);
        let target = path.to_path_buf();
        if bibliography::is_vault_dir(path) {
            let alive = Rc::new(std::cell::Cell::new(true));
            if let Ok((watch, rx)) = fond_vault::watch(path) {
                let alive_t = alive.clone();
                glib::timeout_add_local(Duration::from_millis(300), move || {
                    if !alive_t.get() {
                        return glib::ControlFlow::Break;
                    }
                    let mut changed = false;
                    while let Ok(event) = rx.try_recv() {
                        if let fond_vault::VaultEvent::Changed(_) = event {
                            changed = true;
                        }
                    }
                    if changed {
                        if let Some(inner) = weak.upgrade() {
                            BibManager(inner).show(&target);
                        }
                    }
                    glib::ControlFlow::Continue
                });
                return Watch::Vault {
                    timer_alive: alive,
                    _watch: watch,
                };
            }
        }
        let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let last: Rc<RefCell<Option<SystemTime>>> = Rc::new(RefCell::new(mtime(&target)));
        let id = glib::timeout_add_local(Duration::from_secs(3), move || {
            let now = mtime(&target);
            let changed = *last.borrow() != now;
            if changed {
                *last.borrow_mut() = now;
                if let Some(inner) = weak.upgrade() {
                    BibManager(inner).show(&target);
                }
            }
            glib::ControlFlow::Continue
        });
        Watch::Poll(Some(id))
    }

    /// Whether the source in use was found in the project folder rather than
    /// set by the person, for the one-time "Loaded bibliography" toast.
    pub(super) fn found_in_folder(&self) -> Option<PathBuf> {
        self.current()
            .filter(|r| r.origin == Origin::Found)
            .map(|r| r.path)
    }
}

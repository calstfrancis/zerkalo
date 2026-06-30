# Zerkalo Polish Plan

Eight areas, worked in order. Each phase is self-contained — finish and ship before moving on.

---

## Phase 1 — Undo Reliability

### Diagnosis

GtkSourceBuffer's built-in undo works correctly for normal typing. The breakage comes from four code paths that call `buffer.set_text(...)` directly, which **silently wipes the entire undo stack** as a side effect. The fix pattern already exists in the codebase (`set_active_content_undoable`, `editor_pane.rs:4266`) — it replaces the whole-buffer `set_text` with a `begin_user_action` / delete / insert / `end_user_action` sequence, which is one undoable step.

### The four offenders

#### 1. `apply_style` — `editor_pane.rs:1876`
**Trigger:** user changes citation style or bibliography in Settings.  
**Impact:** HIGH — user loses their full undo history just by changing a style dropdown.  
**Fix:** replace `buffer.set_text(new_content)` with the undoable replace pattern.  
The new state (style applied) becomes one undoable step; Ctrl+Z takes you back to before the style change.

#### 2. `set_content` — `editor_pane.rs:4295`
**Trigger:** autosave recovery accepted by the user (`app_window.rs:2089`).  
**Impact:** MEDIUM — after recovering from a crash, Ctrl+Z does nothing.  
**Fix:** use undoable replace so the recovery itself is undoable (one Ctrl+Z brings back the pre-crash state of the buffer).

#### 3. `splice_preamble` fallback — `editor_pane.rs:2126`
**Trigger:** template update (Update Template Settings) when no `// --- body ---` marker is found.  
**Impact:** MEDIUM — affects any file where the user has removed or never had a body marker.  
**Fix:** apply the same `begin_user_action` / delete / insert / `end_user_action` pattern already used in the happy path (lines 2118–2122). Delete the fallback `set_text` branch.

#### 4. `reload_file` — `editor_pane.rs:2082`
**Trigger:** file watcher detects external change, or git sync pulls remote changes (`app_window.rs:1599, 3543`).  
**Impact:** MEDIUM — undo history is silently cleared whenever the file changes on disk.  
**Decision:** do **not** make this undoable. External changes come from outside the editor; making them undoable would let Ctrl+Z revert someone else's save, which is worse than losing history. Instead:  
- Keep `set_text` here (correct behaviour).  
- Add a `ToastOverlay` notification: *"File reloaded — undo history cleared"* so the user knows why Ctrl+Z stopped working.

### Additional fix

#### 5. Add Ctrl+Y as redo alias
**Location:** `editor_pane.rs:3372–3394` (the `undo_ctrl` Capture-phase handler).  
**Fix:** add `Key::y` (with Ctrl, no Shift) → `buf.redo()` alongside the existing Ctrl+Shift+Z path. One extra `else if` branch.

### What is already correct

- Normal typing undo: works, GtkSourceBuffer handles it.
- Replace-all (`do_replace_all`): wrapped in `begin_user_action`/`end_user_action` — one undoable step. Correct.
- Simple-mode toggle: only manipulates TextTags, never calls `set_text`. Safe.
- Tab switch button sensitivity: updated via `connect_switch_page` + `can_undo_notify`. Correct.
- Toolbar undo/redo buttons: call `buf.undo()` / `buf.redo()` directly. Correct.

### Implementation order

1. Fix `apply_style` (highest impact, trivial change).
2. Fix `splice_preamble` fallback (remove the `set_text` branch).
3. Fix `set_content` (autosave recovery).
4. Add reload toast notification.
5. Add Ctrl+Y redo.

### Test plan

- Open a file, type several paragraphs, change citation style → Ctrl+Z should undo the style change, not wipe history.
- Open a file, type text, force a reload (touch the file externally) → toast appears, Ctrl+Z correctly does nothing (history was external).
- Open a file without a body marker, update template settings → Ctrl+Z should undo the template change.
- Trigger autosave recovery, accept it → Ctrl+Z should undo the recovery.
- Ctrl+Y should redo in all the same places Ctrl+Shift+Z does.

---

## Phase 2 — Crash Recovery

### Diagnosis

The autosave machinery (`auto_save.rs`) exists and covers all modified tabs. The gaps are:

1. **Non-atomic writes** — `auto_save::save` calls `std::fs::write` directly onto `{key}.typ`. A crash mid-write leaves a truncated (corrupt) file. On next open, the corrupt content is offered for recovery.
2. **Autosave never cleared after manual save** — `save_current` and `save_all_modified` in `editor_pane.rs` write the original file but never call `auto_save::clear`. Stale autosave files accumulate indefinitely; the mtime guard prevents false recovery offers but the files are never garbage-collected.
3. **Multiple simultaneous recovery dialogs on session restore** — `set_on_file_opened` fires once per tab during session restore. With 5 open tabs, all 5 recovery dialogs appear simultaneously. The user has no way to handle them in order.

### Fixes

#### 1. Atomic autosave writes — `auto_save.rs:23`
Replace `std::fs::write(dest, content)` with: write to `{key}.typ.tmp`, then `std::fs::rename` to `{key}.typ`. `rename` is atomic on Linux (same filesystem) — a crash mid-write leaves the `.tmp` file; the previous good `.typ` is untouched.

#### 2. Clear autosave after manual save — `editor_pane.rs:4425, 4409`
After a successful `std::fs::write` in both `save_current` and `save_all_modified`, call `crate::auto_save::clear(path)`. This keeps the autosave directory clean without relying on the recovery dialog being shown.

#### 3. Serialise recovery dialogs — `app_window.rs:2038-2094`
Replace the inline dialog creation with a queue (`Rc<RefCell<VecDeque<(PathBuf, String, String)>>>`). When `set_on_file_opened` detects a recovery, push to the queue. If no dialog is currently showing, pop and show immediately. Each dialog's response handler clears the autosave then pops and shows the next item from the queue. This ensures at most one recovery dialog is on screen at a time.

### What is already correct

- All modified tabs are autosaved (not just the active one).
- Recovery check uses mtime comparison — stale autosaves (older than the original) are never offered.
- `auto_save::clear` is called unconditionally on dialog dismiss (both Restore and Discard).
- Recovery for deleted/moved originals: autosave is offered even when the original file is gone (mtime guard is skipped). Edge case — if the original tab was excluded from session restore (because `read_to_string` failed), restoring is a silent no-op. Deferred: low frequency, complex to fix properly.

### Implementation order

1. Atomic writes in `auto_save.rs` (trivial, highest safety value).
2. `auto_save::clear` in `save_current` / `save_all_modified`.
3. Recovery dialog queue in `app_window.rs`.

---

## Phase 3 — Fast Startup

### Diagnosis

Four startup paths were investigated:

1. **Library DB** (`app_window.rs:77–81`) — `Library::open()` opens SQLite, runs WAL pragma, and migrates schema; then `import_directory()` recursively walks the entire work directory and upserts every `.typ` file. Both run synchronously on the GTK main thread **before** `window.present()`. On a large work directory this is the dominant startup cost.
2. **Font install** (`main.rs:86`) — `fonts::ensure_gost_font()` checks if the font file already exists (a single `stat` call); only writes and spawns `fc-cache` on first run. Fast on repeat launches — not a bottleneck.
3. **LSP init** (`app_window.rs:2438`) — already deferred with a 500 ms `timeout_add_local`; does not block first paint.
4. **Typst compiler** (`compiler.rs`) — font scan is a `OnceLock`, first access is on a background `std::thread::spawn`. Does not block the main thread.

The only real bottleneck is the library DB.

### Fix

#### Library DB on background thread — `app_window.rs:77–81`

Replace the blocking init with a `glib::MainContext::channel` pattern:
- Create an in-memory `Library` immediately (no I/O) so the rest of `AppWindow::new()` continues and `window.present()` fires.
- `std::thread::spawn` opens the real DB, runs `import_directory`, and sends the result back.
- The `receiver.attach` callback (runs on the main thread) swaps the in-memory library out once the thread finishes.

Because `LibraryWindow` holds a `Rc<RefCell<Library>>` clone (not a snapshot), it sees the real library automatically on next open — no additional wiring needed.

`rusqlite::Connection` is `Send` (since rusqlite 0.26), so `Library` can cross thread boundaries.

### What is already correct

- LSP: deferred 500 ms, does not block first paint.
- Typst compiler: lazy, always on a background thread.
- Font install: stat-only on repeat runs, effectively free.

### Implementation order

1. Replace blocking library init with thread + channel pattern. ✓

### Test plan

- Launch Zerkalo with a large work directory (100+ `.typ` files) — window should appear before the library is fully scanned.
- Open the Library window immediately on launch — shows empty (in-memory placeholder) and populates once the background thread finishes.
- Library window opened after a few seconds — shows full contents as expected.

---

## Phase 4 — Smooth Scrolling

### Diagnosis

Three areas investigated in `editor_pane.rs`:

1. **Typewriter scroll** (`editor_pane.rs:2639`) — fires via `idle_add_local_once` on every line-boundary crossing while typing. One idle tick is not enough to coalesce rapid crossings (e.g. holding Enter or pasting), so the view can recenter multiple times in quick succession for a single burst of input. Fix: replace `idle_add_local_once` with an 80 ms `timeout_add_local_once` + generation counter — the same pattern used for cursor-moved debounce.

2. **Heading-based preview sync** (`editor_pane.rs:2693`) — fires immediately when the cursor enters a new heading section (guarded by `heading_line != *last_heading_line`). The guard prevents per-keystroke firing within a section, but the preview jumps instantly when the cursor first crosses a section boundary mid-edit. Fix: 200 ms generation-counter debounce, so the preview only jumps after the cursor settles.

3. **Line-fraction preview sync** (`editor_pane.rs:2706`) — already debounced 300 ms with a generation counter. No change needed.

4. **hadjustment snap** (`editor_pane.rs:4043`) — a per-frame tick resets horizontal scroll when GTK snaps it to `left_margin`. This is the only viable workaround for a GtkSourceView5 bug; no upstream API to suppress it. Left as-is.

### Fixes

#### Typewriter scroll debounce — `editor_pane.rs:2639`
Added `typewriter_gen: Rc<Cell<u64>>`. Replaced `idle_add_local_once` with `timeout_add_local_once(80ms)` + generation check. Rapid line crossings now coalesce: only the last one fires.

#### Heading sync debounce — `editor_pane.rs:2693`
Added `heading_sync_gen: Rc<Cell<u64>>`. The `last_heading_line` is still updated immediately (so duplicate headings are suppressed), but the callback into `on_heading_cb` is deferred 200 ms. The preview jumps only when the cursor settles in a new section, not on the instant of boundary crossing.

### What is already correct

- Line-fraction preview sync: 300 ms debounce, generation counter. Correct.
- hadjustment snap tick callback: unavoidable workaround; a no-op on most frames.

### Implementation order

1. Add `typewriter_gen` and `heading_sync_gen` counters. ✓
2. Debounce typewriter scroll (80 ms). ✓
3. Debounce heading sync (200 ms). ✓

---

## Phase 5 — Keyboard Workflow

### Diagnosis

Four areas investigated:

1. **Tab navigation** (`editor_pane.rs:4454`) — `next_tab` / `prev_tab` called `set_current_page` but never moved keyboard focus to the new tab's text view. GTK left focus on the old tab's widget, so the newly-selected tab was non-editable without a mouse click.

2. **Sidebar focus trap** (`file_tree.rs`) — the sidebar `ListBox` participates in GTK's default Tab focus chain. Pressing Tab from a sidebar row cycled through every toolbar button and widget rather than jumping back to the editor. The only escape was `F6` / `Shift+F6`. No Tab override existed.

3. **Command palette focus return** (`command_palette.rs`) — `show()` grabbed focus to the entry field, but on close (Escape or row activation) no `grab_focus()` was called on the editor. GTK returned focus to the parent window's last-focused widget, which could be any toolbar button.

4. **Find-bar Escape** (`editor_pane.rs:993`) — pressing Escape hid the find bar via `set_reveal_child(false)`, but the `on_reveal_changed` callback only updated the toggle button label. GTK dropped focus (the entry was no longer visible) without returning it to the editor text view.

### Fixes

#### Tab navigation — `editor_pane.rs:4460, 4470`
Added `self.grab_focus()` after every `set_current_page` call in `next_tab` and `prev_tab`. The active text view immediately receives keyboard focus.

#### Sidebar Tab key — `file_tree.rs:303`, `app_window.rs`
Added `set_on_tab_out(f)` to `FileTree`: registers a Capture-phase `EventControllerKey` on the `ListBox` that intercepts `Tab` / `ISO_Left_Tab`, calls the callback, and stops propagation. Wired in `app_window.rs` to call `editor_pane.grab_focus()`. Tab now exits the sidebar in one keystroke.

#### Command palette focus return — `command_palette.rs:142`, `app_window.rs`
Added `set_on_close(f)` to `CommandPalette`: connects `window.connect_hide` so the callback fires whenever the palette hides (Escape, row activation, or window-manager close). Wired in `app_window.rs` to call `editor.grab_focus()`.

#### Find-bar Escape focus — `editor_pane.rs:993`
Extended the `set_on_reveal_changed` callback to capture an `EditorPane` clone and call `grab_focus()` when `revealed == false`.

### What is already correct

- `F6` / `Shift+F6`: toggles focus between file tree and editor. Correct.
- Command palette arrow-key navigation: Up/Down moves list selection. Correct.
- Find-bar search-entry focus on open: `toggle()` grabs focus to entry. Correct.

---

## Phase 6 — Stable Project Handling

### Diagnosis

Four areas investigated:

1. **File-watcher deduplication** (`file_watcher.rs:15`) — pending paths were stored in a `Vec<PathBuf>`. A tool writing a file twice within 250 ms would fire `on_change` twice, triggering two compiles. Also, the "is open" guard in `app_window.rs` only compared against the *active* tab, so any background tab being externally modified would still trigger a spurious compile.

2. **Git sync conflict message** (`app_window.rs:4318`) — when a pull rebase failed (merge conflict), the error was shown as "Push Failed" with raw git output. The rebase was correctly aborted (repo left clean), but the message gave no guidance about what happened or what to do.

3. **Tab close without dirty check** (`editor_pane.rs:2322`) — all three close paths (X button, middle-click, context menu "Close tab") called `notebook.remove_page` immediately without checking the `modified` flag. Unsaved work was silently discarded.

4. **Work-dir change** (`app_window.rs:1017`) — changing the work folder in Settings applied the new config but left the file watcher, file tree, and `project_root` pointing at the old directory. No indication was given that a restart was needed.

### Fixes

#### File-watcher dedup — `file_watcher.rs`
Changed `Arc<Mutex<Vec<PathBuf>>>` to `Arc<Mutex<HashSet<PathBuf>>>`. Duplicate writes to the same file within a 250 ms poll window now collapse to one `on_change` call.

#### Watcher "is open" guard — `editor_pane.rs`, `app_window.rs`
Added `pub fn is_file_open(&self, path: &PathBuf) -> bool` to `EditorPane`. Updated the watcher callback to use this instead of `get_active_path()`, so any open tab — not just the active one — suppresses spurious recompile on external write.

#### Git sync conflict message — `app_window.rs:4318`
Parse `push_errors` for `"CONFLICT"` or `"Pull failed"`. When detected, show a targeted dialog: *"Merge conflict — sync aborted"* with a clear explanation that local work is safe and instructions to resolve manually.

#### Tab close dirty check — `editor_pane.rs:2322`
Added `close_tab_with_dirty_check(ep, state, notebook, scroll, path, display_name)` free function. Shows an `AlertDialog` with **Save / Discard / Cancel** when a modified tab is closed. "Save" calls `ep.save_current()` then closes; "Discard" closes immediately; "Cancel" does nothing. Wired into all three close paths: X button, middle-click, and context menu "Close tab".

#### Work-dir restart prompt — `app_window.rs:1049`
After saving settings, compare `new_cfg.work_dir` to `old_cfg.work_dir`. If changed, show an `AlertDialog`: *"Restart required — the work folder change takes effect after restarting Zerkalo."*

### What is already correct

- Autosave covers all modified tabs (not just active). Correct.
- Session restore saves and restores cursor positions. Correct.
- Git rebase abort on conflict: repo left clean. Correct.

---

## Phase 7 — Predictable Compilation Errors

*(planned — not yet detailed)*

Areas to investigate: error panel update timing, error dot placement accuracy, LSP error vs compiler error reconciliation, empty-error-on-save flicker.

---

## Phase 8 — Visual Polish

*(planned — not yet detailed)*

Areas to investigate: spacing and margin consistency, card/compact library visual refinement, status bar layout, welcome window, colour scheme coherence.

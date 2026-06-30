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

*(planned — not yet detailed)*

Areas to investigate: time to first paint, LSP startup latency, library DB open on main thread vs async, Typst compiler init.

---

## Phase 4 — Smooth Scrolling

*(planned — not yet detailed)*

Areas to investigate: typewriter scroll debounce, preview scroll sync, GtkSourceView hadjustment snap (left-margin workaround already exists at `editor_pane.rs:4024`).

---

## Phase 5 — Keyboard Workflow

*(planned — not yet detailed)*

Areas to investigate: tab navigation, sidebar focus traps, command palette keyboard flow, find-bar Escape behaviour.

---

## Phase 6 — Stable Project Handling

*(planned — not yet detailed)*

Areas to investigate: file-watcher reliability, git sync conflict handling, multi-tab close/restore, work-dir change behaviour.

---

## Phase 7 — Predictable Compilation Errors

*(planned — not yet detailed)*

Areas to investigate: error panel update timing, error dot placement accuracy, LSP error vs compiler error reconciliation, empty-error-on-save flicker.

---

## Phase 8 — Visual Polish

*(planned — not yet detailed)*

Areas to investigate: spacing and margin consistency, card/compact library visual refinement, status bar layout, welcome window, colour scheme coherence.

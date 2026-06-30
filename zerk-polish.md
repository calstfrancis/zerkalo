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

*(planned — not yet detailed)*

Areas to investigate: autosave write atomicity, recovery UI flow, what happens on unclean exit with unsaved changes in multiple tabs.

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

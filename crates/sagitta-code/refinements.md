### Refinements plan for 2025-06-15

The following phased plan fixes the five issues reported for `sagitta-code` and its companion crates (`sagitta-cli`, `sagitta-mcp`, `sagitta-search`).  Each phase is designed to be small, test-driven and independently releasable.

---

## Phase 0 – Test scaffolding & safety-nets ✅ COMPLETED
1. ✅ Introduced regression tests for:
   • Copy-button visual feedback (assert state flag toggles on click).
   • `sync_repository` long-running operation (>60 s) with continuous progress.
   • Theme persistence across restarts plus export / import round-trip.
   • CLI `repo add --url` progress stream surfaces in TTY and GUI.
   • All panel hot-keys open/close correctly; every menu entry triggers the same action.
2. ✅ Extended existing mock infrastructure so the new tests are deterministic and CI-friendly (e.g. fake Git clone with sleep loops; fake egui context, etc.).

✅ All 14 tests pass and provide acceptance criteria for the subsequent phases.

---

## Phase 1 – Reactive copy-buttons (Issue #1) ✅ COMPLETED
• ✅ Added transient "✔ Copied" state that lasts 800 ms.
• ✅ Provided subtle colour animation (success_color() feedback).
• ✅ Implemented CopyButtonState with visual feedback system.

✅ Deliverables: UI change + unit test in `gui/chat/view.rs` + integration in app state.

---

## Phase 2 – Reliable long-running `sync_repository` (Issue #2)
• Remove the hard-coded 60 s Tokio timeout.
• Replace it with watchdog logic: terminate only if no `SyncProgress` arrives after **N=120 s**.
• Emit periodic heartbeat (`SyncStage::Idle`) from core `sync::sync_repository` loop so that GPU-bound index phases still count as progress.
• Propagate the watchdog to:
  – `sagitta-cli repo sync`
  – GUI `RepositoryManager::sync_repository`
  – MCP HTTP handler.
• Update CLI progress bar to stay alive until final `Completed` or `Error` stage.

---

## Phase 3 – Theme persistence & sharing (Issue #3)
• Extend `SagittaCodeConfig` (`ui.theme` + `ui.custom_theme_path`).
• When the user changes theme or customises colours:
  – Immediately write back to config (`save_config`).
• Add `Export` / `Import` buttons in the Theme Customiser:
  – Serialise `CustomThemeColors` to JSON (`*.sagitta-theme.json`).
  – Allow drag-&-drop or file picker to load.
• On startup, if `ui.custom_theme_path` is present, load colours and switch to `AppTheme::Custom`.

---

## Phase 4 – Progress feedback for `repo add` (Issue #4)
• Introduce new enum `RepoAddStage` analogous to `SyncStage` (Clone, Fetch, Checkout, Completed, Error).
• Add `AddProgressReporter` trait (or generalise existing `SyncProgressReporter` to cover both scenarios).
• Instrument `sagitta-search::repo_add::handle_repo_add` to emit stages.
• Hook progress into:
  – `sagitta-cli` using `indicatif` progress bar (same UX as `repo sync`).
  – MCP websocket / HTTP stream so GUI progress bar can mirror it.
  – GUI side (`progress.rs`) to render percentage & speed.

---

## Phase 5 – Consistent panel hot-keys & fallback menu (Issue #5)
• Centralise keyboard handling in `handle_keyboard_shortcuts` ensuring every shortcut maps to `PanelManager::toggle_panel` for idempotency.
• Add exhaustive list of hot-key actions inside the "Hotkeys" modal; render them as clickable buttons that call the same toggle logic (guaranteed backup UI path).
• Add integration tests simulating sequential open/close events to ensure no races.

---

## Phase 6 – Documentation & cleanup
• Update `README.md` CHANGELOG with new capabilities (no extra end-user docs were requested).
• Run `cargo clippy --all-targets --all-features` and address warnings.
• Verify all new tests pass.

---

**Estimated effort:** 2–3 days of focussed development including tests and review. 
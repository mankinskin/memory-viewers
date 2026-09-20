## Goal
Make `ticket.upsert` update node visual state without a full workspace layout cache wipe, and fix the version-signal no-op so the intended reactive update actually fires.

## Scope
File: `memory-viewers/ticket-viewer/frontend/dioxus/src/graph_fetch.rs` (`invalidate_workspace`, graph_fetch.rs:160)

Problems:
- `invalidate_workspace` removes the entire workspace cache entry on every `ticket.upsert`, forcing a full layout rebuild + Graph3D re-bootstrap instead of a targeted node update.
- No-op bug: `let mut version = self.version; version += 1;` increments a LOCAL copy of the `Signal` handle; the UI version never bumps. Compare `pump_queue` which uses `svc.version += 1`.

Changes:
- Fix the version bump to mutate the actual signal (`self.version += 1`), matching `pump_queue`.
- Prefer a targeted update path: on `ticket.upsert`, update the affected node's visual fields (state/color/title) in the cached layout instead of evicting the whole workspace entry. Fall back to full invalidation only when the node is absent or topology changed.

## Acceptance Criteria
- [ ] `ticket.upsert` updates node color/label without evicting the full workspace layout cache
- [ ] Version signal actually increments and drives a reactive update
- [ ] Edge upsert/delete reactivity still works (backward compatible)
- [ ] No layout/camera reset on a pure node-state change

## Validation
- `cargo test -p ticket-viewer` (graph reactivity integration test)
- wasm-pack ticket-viewer dioxus tests
- Playwright: change ticket state in detail panel; node recolors without layout reset
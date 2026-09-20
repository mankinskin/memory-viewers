## Goal
Eliminate the per-frame inefficiency that makes DOM node cards visibly lag behind the GPU-drawn edge endpoints while orbiting/panning the camera or dragging nodes in the WebGPU graph. Reactivity must only trigger on real database updates and must not perturb per-frame interaction performance.

## Context (root-cause analysis, already completed)
Camera/node interaction is fully decoupled from Dioxus: raw DOM listeners mutate `RenderState` via `Rc<RefCell<…>>` and a self-rescheduling `requestAnimationFrame` loop drives `render_frame`. The reactivity ticket (111510f4) is NOT the per-frame culprit. The real lag comes from per-frame DOM layout thrashing in `viewer-api/viewer-api/frontend/dioxus/src/graph3d/render.rs`:

1. `collect_dom_node_rects()` (render.rs:220) does a full `getBoundingClientRect()` sweep over every node card (forced synchronous reflow) and is called TWICE per frame — `position_dom_layout_anchors` (render.rs:413) and `position_dom_edges` (render.rs:776).
2. The view-projection matrix is recomputed from scratch THREE times per frame in the DOM helpers (`position_dom_nodes` render.rs:570, `position_dom_layout_anchors` render.rs:388, `position_dom_edges` render.rs:724), on top of the GPU VP in `render_frame`.
3. The SVG edge overlay anchors endpoints to node `getBoundingClientRect` values instead of the already-computed `world_to_screen` projections, coupling edge geometry to the DOM reflow.

Separately, a reactivity hazard exists:
4. `invalidate_workspace` (memory-viewers/ticket-viewer/frontend/dioxus/src/graph_fetch.rs:160) wipes the entire workspace layout cache on every `ticket.upsert`, forcing a full layout rebuild + re-bootstrap, and contains a no-op bug `let mut version = self.version; version += 1;` (increments a local copy of the Signal instead of `self.version += 1`).
5. If a Dioxus re-render lands mid-interaction, `sync_render_state` (viewer-api/viewer-api/frontend/dioxus/src/graph3d/mod.rs:604) can reset `state.layout`/`base_layout` from props and snap back an in-progress drag (drag mutates `state.base_layout`, handlers.rs:363, while props still hold cache coordinates).

## Child slices
- Per-frame DOM thrashing fix (compute VP once, collect node rects once, share across helpers)
- SVG edge endpoints from world_to_screen instead of getBoundingClientRect
- Guard sync_render_state against resetting layout during active drag/camera interaction
- Targeted node-field update on ticket.upsert + fix invalidate_workspace version-signal no-op

## Acceptance Criteria
- [ ] Node cards stay visually anchored to edge endpoints during camera orbit/pan and node drag (no trailing)
- [ ] `getBoundingClientRect` reflow over node cards happens at most once per frame
- [ ] VP matrix computed once per frame and shared by GPU + DOM positioning
- [ ] Reactive re-renders never reset an in-progress drag/camera interaction
- [ ] `ticket.upsert` updates node visual state without a full layout cache wipe; version signal bump actually fires
- [ ] Browser/Playwright validation against an external Chromium browser confirms no trailing during drag

## Validation
- `wasm-pack test --headless --chrome memory-viewers/ticket-viewer/frontend/dioxus` (and viewer-api graph3d tests)
- Manual + Playwright drag/orbit verification in external Chromium with screenshots
- `cargo check --target wasm32-unknown-unknown` for affected crates
<!-- aligned-structure:v1 -->

# Summary

`spec-viewer` is a single-process, GPU-accelerated web application for **reading and navigating** the specification database.  It follows the exact same architecture as `ticket-viewer`: a Rust backend (Axum) that embeds `spec-http`'s router, serving a Dioxus/WASM SPA on a shared port (default **4002**).  The WebGPU canvas from `viewer-api-dioxus` provides the animated background; the SPA overlay renders spec content.

## Behavior Story

`spec-viewer` is a single-process, GPU-accelerated web application for **reading and navigating** the specification database.  It follows the exact same architecture as `ticket-viewer`: a Rust backend (Axum) that embeds `spec-http`'s router, serving a Dioxus/WASM SPA on a shared port (default **4002**).  The WebGPU canvas from `viewer-api-dioxus` provides the animated background; the SPA overlay renders spec content.

## Provided Surface Contracts

- Define provided contracts for this behavior slice.

## Required Validation

- Triangulate behavior with executable checks, natural-language clauses, and code/schema/API references when available.

## Related Implementation Tickets

- No related implementation ticket is linked yet.

## Background Knowledge References

- Prefer entity references and context rendering over embedding fully expanded payloads in this spec body.

## Legacy Content (Preserved)

# spec-viewer

`spec-viewer` is a single-process, GPU-accelerated web application for **reading and
navigating** the specification database.  It follows the exact same architecture as
`ticket-viewer`: a Rust backend (Axum) that embeds `spec-http`'s router, serving a
Dioxus/WASM SPA on a shared port (default **4002**).  The WebGPU canvas from
`viewer-api-dioxus` provides the animated background; the SPA overlay renders spec
content.

---

## Goals

1. **Read-first UX** — browse the spec hierarchy, read body + sections as rendered
   Markdown, inspect CodeRefs.
2. **GPU-accelerated presentation** — animated background via `ViewerShell`; future
   3-D tree visualization via a dedicated WebGPU render pass (similar to
   `ticket-viewer`'s `graph3d`).
3. **Live updates via SSE** — spec changes made through CLI or spec-editor reflect in
   the viewer without a page reload.
4. **Zero separate process** — the viewer binary embeds `spec-http` and `viewer-api`
   in one `cargo run -p spec-viewer` invocation.

---

## Architecture

```
spec-viewer (binary)
├── src/main.rs          — CLI args, ServerConfig, mounts spec-http router
└── frontend/dioxus/     — Dioxus WASM SPA
    ├── src/main.rs      — launches App with ViewerShell
    ├── src/routes.rs    — Route enum + page components
    ├── src/api.rs       — gloo-net HTTP client for spec-http REST endpoints
    ├── src/store.rs     — reactive state (selected spec, filter, scroll pos)
    ├── src/sse.rs       — SSE subscription for live spec change events
    ├── src/types.rs     — client-side SpecSummary, SpecDetail mirror types
    └── src/components/
        ├── spec_tree.rs     — collapsible hierarchy tree (parent/child specs)
        ├── spec_detail.rs   — body.md + sections rendered as Markdown
        ├── spec_card.rs     — compact row in the list/tree views
        ├── code_ref_list.rs — CodeRef table with file/line/kind display
        ├── state_badge.rs   — coloured pill for draft/reviewed/approved/...
        ├── search_bar.rs    — full-text search input calling GET /api/specs/search
        └── health_panel.rs  — displays spec health results (completeness, staleness)
```

### Backend (`src/main.rs`)

- Accept `--port`, `--workspace`, `--index-root`, `--static-dir` CLI flags.
- Use `viewer-api`'s `ServerConfig`, `init_tracing_full`, `default_cors`,
  `with_static_files`.
- Mount `spec-http` router at `/api/` — identical to how `ticket-viewer` mounts
  `ticket-http`.
- Serve built SPA from `tools/viewer/spec-viewer/frontend/dioxus/dist` (trunk output) or fallback
  `static/`.

### Frontend SPA

**Routes:**

| Route | Page |
|---|---|
| `/` | Redirect → `/workspace/default` |
| `/workspace/:ws` | `SpecListPage` — search bar + flat list of all specs |
| `/workspace/:ws/tree` | `SpecTreePage` — collapsible hierarchy rooted at any spec |
| `/workspace/:ws/spec/:slug` | `SpecDetailPage` — body + sections + CodeRefs + health |

**State management:**

- `SpecListStore` — holds the current filter, sort field, and list of `SpecSummary`
  items.  Persists filter state in `localStorage` per workspace.
- `use_sse` hook — subscribes to `GET /api/specs/events` for push notifications;
  triggers a list refresh on `spec.created`, `spec.updated`, `spec.deleted`.
- Individual spec detail fetches are done on navigation and cached in a reactive
  `Signal<Option<SpecDetail>>`.

**3-D tree (future):**

A `SpecGraph3D` WebGPU render pass (modelled on `graph3d.rs` in ticket-viewer) will
visualise the spec hierarchy as a force-directed graph.  Nodes are coloured by state;
edges represent parent→child relationships.  This phase is deferred but the
architecture must leave room for it: the WebGPU canvas from `ViewerShell` is already
present in the DOM and available for this render pass.

---

## Integration Points

- **`spec-http`** — all data reads go through the REST API; no direct `SpecStore`
  access from the frontend.
- **`viewer-api-dioxus`** — `ViewerShell`, `Header`, `Sidebar`, `Layout` shared
  components.
- **`spec-editor`** (future) — viewer pages will link to edit actions that deep-link
  into the editor; the two tools can run concurrently.

---

## Non-Goals (for this spec)

- Editing — handled by `spec-editor`.
- State machine transitions — read-only badge display only.
- CodeRef inline source preview — deferred to a later spec.
- Authentication / multi-user — out of scope for local tooling.

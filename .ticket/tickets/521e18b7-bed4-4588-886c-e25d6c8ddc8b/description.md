Design the next navigation model for `spec-viewer` so users can reach the important UI surfaces by clicking through the app and can switch specs without losing the current view context.

Today the viewer still depends too heavily on direct URL knowledge (`/specs`, `/specs/graph`, `/specs/:id`, legacy `/specs/tree`) and the selected detail tab is not modeled strongly enough as a first-class dimension of navigation.

The target direction is the same general pattern used by `log-viewer-dioxus`: navigation should be expressible as a 2-dimensional state of `entry + view`, where the active view persists while the selected entry changes.

## Interview Outcomes

- Primary click-through destinations must include:
  - `/specs`
  - `/specs/graph`
  - `/specs/:id`
- The standalone tree route should be removed as a primary destination.
- The current list route should become the main root-level tree/list page that shows root-level entries in a list.
- The navigation model should move toward an `entry × view` structure rather than keeping the current route graph as-is.
- When switching from one spec to another, preserve the current tab globally.
- If the destination cannot show the globally selected tab, fall back to the last active tab remembered for that spec.
- Verification must explicitly cover:
  - reaching primary routes by clicks only
  - preserving the active tab while switching specs
  - browser back/forward restoring both entry and view
  - deep links continuing to work
  - coherent cross-links between list, graph, detail, and the tree functionality folded into list

## Scope

- Specify the fully reachable page graph for `spec-viewer`.
- Define the canonical navigation state model for:
  - selected spec entry
  - selected view/tab
  - collection-level destinations such as graph
- Define how URL state, browser history, and in-app clicks all map onto that same model.
- Define how the current `/specs/tree` behavior is folded into `/specs`, including migration or redirect behavior for the legacy route.
- Define how list, detail, and graph interactions change `entry` and/or `view` without forcing the user to manually edit URLs.
- Define the fallback rules when a requested view is unavailable for the newly selected spec.
- Define whether graph remains a collection-level destination or becomes part of the entry × view model, and how that decision affects URL design.

## Acceptance Criteria

- The design names the primary destinations in the app and the click paths that make each one reachable from within the UI.
- The design defines a 2-dimensional navigation state of `entry` and `view`, including URL encoding and browser history semantics.
- Switching specs from the main browsing flow preserves the currently selected detail tab when possible.
- When the globally selected tab is unavailable, the design specifies a deterministic fallback order and persistence behavior.
- `/specs` is the root-level browse page and absorbs the current tree-view browsing role.
- The design specifies how legacy `/specs/tree` links are handled.
- Deep links to a specific spec/view remain supported.
- The design explains how graph, list, and detail pages cross-link without dead ends or URL-only destinations.

## Verification Plan

- Browser flow: open the app at `/specs` and reach the graph view and a specific detail view by clicks only.
- Browser flow: choose a non-default detail tab such as `Sections`, then switch to another spec and confirm the same tab remains active or the specified fallback rule triggers.
- Browser flow: use browser back/forward after changing both spec and tab, and confirm that both dimensions restore correctly.
- Browser flow: open a deep link for a specific spec/view and confirm the viewer lands on the requested state.
- Browser flow: verify that the root-level browsing page exposes the former tree-browsing capability without requiring a separate URL-only route.

## Concrete Model Draft

- Detailed URL/state design: `agents/designs/20260510_DESIGN_SPEC_VIEWER_ENTRY_VIEW_NAVIGATION.md`

## Implementation Breakdown

- `a76ce0b4-e906-4ecd-8513-0cb763ec305c` — `[spec-viewer][nav-1] canonical entry×view route contract + URL normalization`
- `8a3fe2eb-511a-4d2d-9e98-c17b9b812399` — `[spec-viewer][nav-2] fold tree into /specs + restore click-through reachability`
- `57db7e0f-1189-4b06-8cd2-718f8d9beace` — `[spec-viewer][nav-3] preserve active view while switching specs`
- `bf250aa9-ca23-4686-83ca-c1395b1e3d1e` — `[spec-viewer][nav-4] browser verification for reachable page graph + entry×view`

## Nearby Context

- Existing URL routing ticket: `[spec-viewer][P5.6] URL routing via PathCodec + tree expand` (`4f69b73e-8352-4b4a-8a8b-93ad6b65c056`) solved URL round-tripping for the active spec id, but not the broader reachable page graph or entry × view model.
- Current spec-viewer route graph lives in `memory-viewers/spec-viewer/frontend/dioxus/src/routes.rs`.
- Current spec tab synchronization lives in `memory-viewers/spec-viewer/frontend/dioxus/src/routes/list/effects.rs`.
- The reference pattern for preserving `entry + tab` state is in `tools/viewer/log-viewer/frontend/dioxus/src/app.rs`.

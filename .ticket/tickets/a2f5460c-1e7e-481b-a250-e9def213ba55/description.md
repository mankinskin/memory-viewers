# Problem

Research against the current Dioxus ticket-viewer shows the explorer still has three concrete gaps:

1. Combined search + state filtering is incorrect. `GET /api/tickets` drops `state` whenever `query` is present, so the sidebar explorer cannot reliably narrow search results under an active state filter.
2. State chips are still single-select. The current store persists `state_filter` as a single string and the sidebar header renders one active chip at a time.
3. Ticket navigation is still mouse-first. The sidebar ticket tree and the quick-search overlay do not provide consistent arrow-key focus movement plus Enter activation.

## Evidence

- `memory-viewers/ticket-viewer/frontend/dioxus/src/store.rs` persists `state_filter` as `String`.
- `memory-viewers/ticket-viewer/frontend/dioxus/src/routes/list/page.rs` sends only one `state` value to `list_tickets(...)`.
- `memory-viewers/ticket-viewer/frontend/dioxus/src/components/ticket_tree/header.rs` compares chips against a single active value.
- `memory-viewers/memory-api/tools/http/ticket-http/src/serve/handlers/tickets/read.rs` takes the `query` branch and ignores `params.state` entirely.
- `memory-viewers/ticket-viewer/frontend/dioxus/src/components/search/results.rs` supports hover/click only; `Enter` does not activate a keyboard-focused result because no keyboard focus model exists.

## Scope

This parent tracks the concrete explorer hardening work only:

- fix combined query + state filtering
- support multi-select state chips in the explorer
- add keyboard navigation for the sidebar tree and quick-search result list

Viewer-wide keyboard support beyond those local flows is tracked separately.

## Deliverables

1. The ticket list API returns correct results for combined query and state filters.
2. The sidebar explorer supports selecting multiple states at once with clear active/reset behavior.
3. Users can move through tickets with Up/Down and open the focused ticket with Enter in the sidebar tree and quick-search results.
4. Focus behavior remains visible and predictable after filtering, refresh, and search-result changes.
5. Regression coverage exists at the API layer and the browser UX layer.

## Execution Notes

- Treat the filter bug as the root-cause fix before or alongside the multi-state work.
- Keep the broader shortcut system out of this ticket so implementation stays reviewable.
- Browser verification and Playwright coverage are required because this changes a browser-facing viewer flow.

# ticket-viewer: explorer interactions

Canonical specification for the ticket-viewer navigation surfaces that let a user find and open tickets: the sidebar explorer and the quick-search overlay.

## Sidebar explorer

The sidebar explorer combines:

- a free-text query input
- state filter chips
- a sorted ticket list / tree surface
- optional expanded file rows under a ticket

### Filtering contract

1. Query text and state filters combine with AND semantics.
2. Multiple selected states combine with OR semantics.
3. The empty state-filter set means "All states".
4. Explorer persistence includes query text, selected states, sort key, and selected ticket.

### Transport contract

The explorer issues `GET /api/tickets` requests with:

- `workspace=<name>`
- optional `query=<text>`
- zero or more repeated `state=<value>` parameters
- optional `limit=<n>`

Repeated `state` parameters are the canonical multi-state format. A single `state` parameter remains valid as the one-state case.

## Keyboard behavior

### Sidebar explorer

When the sidebar tree itself owns focus:

- `ArrowDown` / `ArrowUp` move between visible ticket rows.
- Phase 1 ticket navigation applies to ticket rows, not expanded file child rows.
- `Enter` opens the focused ticket.
- Text inputs in the explorer own their own keystrokes and suppress row-navigation shortcuts while focused.

### Quick-search overlay

When quick-search is open:

- `ArrowDown` / `ArrowUp` move a highlighted result index through the visible result set.
- `Enter` opens the highlighted result.
- `Escape` closes the overlay.
- Typing in the search box keeps focus in the input and does not trigger background viewer shortcuts.

## Acceptance behavior

- The sidebar can show tickets from multiple workflow states at the same time.
- A query combined with selected states only returns tickets that match the query and at least one selected state.
- The sidebar and quick-search both expose deterministic visible keyboard focus.
- Reloading the page restores the persisted explorer filter state without corrupting older saved state.
- If `/api/tickets` or `/api/stream` fails, the sidebar keeps rendering its error or empty-state copy without panicking the WASM app.
- After those offline failures, clicking or reconnect retries must not crash the explorer; the route stays interactive and can recover when the backend returns.

## Related shared specs

- `viewer-api/components/tree-view`
- `viewer-api/keyboard-interaction-model`

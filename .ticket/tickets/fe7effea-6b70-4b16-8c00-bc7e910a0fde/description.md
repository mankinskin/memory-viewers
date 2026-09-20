## Parent Ticket
Depends on: [111510f4 Fix graph reactivity: ticket state changes don't update graph nodes](C:/Users/linus_behrbohm/git/SECOND_CHECKOUT/graph_app/context-engine/memory-viewers/.ticket/tickets/111510f4-c74b-4819-800b-d68ab013a73c/ticket.toml)

## Problem
Need to validate that graph reactivity fixes work correctly and don't introduce regressions. The implementation needs comprehensive testing to ensure:
1. Graph nodes update when ticket state changes
2. Performance remains acceptable
3. No breaking changes to existing functionality

## Testing Tasks

### 1. Manual Testing
- Open ticket-viewer in browser
- Navigate to graph view
- Open ticket details panel for a node
- Change ticket state (e.g., `new` → `in-implementation`)
- Verify graph node updates color immediately
- Verify graph node label updates if title changed
- Test with both 2D and 3D graph modes

### 2. Automated Testing
- Add unit test for updated `subscribe_sse` function
- Test that `fetch_trigger` increments on `ticket.upsert` events
- Test cache invalidation behavior
- Verify backward compatibility with `edge.*` events

### 3. Performance Testing
- Monitor network requests during ticket updates
- Ensure no excessive re-fetching
- Verify cache hit rates remain good
- Test with large graphs (100+ nodes)

### 4. Edge Cases
- Multiple rapid ticket updates
- Concurrent edge and ticket updates
- Network disconnection/reconnection
- Workspace switching
- Graph layout mode changes during updates

## Test Scenarios

### Scenario 1: Basic State Update
1. Select ticket node in graph
2. Open details panel
3. Change state from `new` to `in-implementation`
4. Verify node color changes from blue to orange
5. Verify graph re-fetches layout once

### Scenario 2: Title Update
1. Select ticket node
2. Edit title in details panel
3. Verify node label updates in graph
4. Verify layout preserves node positions

### Scenario 3: Concurrent Updates
1. Have graph open in one tab
2. Update ticket via CLI in another terminal
3. Verify graph updates via SSE
4. Verify no duplicate network requests

### Scenario 4: Error Handling
1. Simulate network error during re-fetch
2. Verify error state displayed
3. Verify retry mechanism works
4. Verify cache doesn't get corrupted

## Files to Create/Modify
- `memory-viewers/ticket-viewer/frontend/dioxus/src/components/dep_graph/state.rs` (unit tests)
- `memory-viewers/ticket-viewer/tests/graph_reactivity.rs` (new test file)
- Playwright E2E tests for browser verification

## Acceptance Criteria
- [ ] Manual test passes: ticket state changes update graph nodes
- [ ] Manual test passes: ticket title changes update graph labels
- [ ] Unit test: `subscribe_sse` handles `ticket.upsert` events
- [ ] Unit test: cache invalidation works correctly
- [ ] Performance: no excessive re-fetching (>1 request per update)
- [ ] Backward compatibility: `edge.*` events still work
- [ ] Error handling: network errors don't break graph
- [ ] Documentation: update Graph3D component docs with reactivity behavior
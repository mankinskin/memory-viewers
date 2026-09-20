## Parent Ticket
Depends on: [111510f4 Fix graph reactivity: ticket state changes don't update graph nodes](C:/Users/linus_behrbohm/git/SECOND_CHECKOUT/graph_app/context-engine/memory-viewers/.ticket/tickets/111510f4-c74b-4819-800b-d68ab013a73c/ticket.toml)

## Problem
When `ticket.upsert` events trigger graph re-fetch, the `GraphFetchService` cache may still contain stale layout data. The cache needs to be invalidated when ticket data changes to ensure fresh data is fetched.

## Current Implementation
`GraphFetchService` in `graph_fetch.rs`:
- Caches workspace graph layouts in LRU cache
- Uses `workspace_cache_key()` for cache keys
- No invalidation mechanism for ticket updates

## Implementation Tasks

### 1. Add Cache Invalidation Method
Add a public method to `GraphFetchService` to invalidate workspace cache:
```rust
pub fn invalidate_workspace(&self, workspace: &str) {
    let cache_key = workspace_cache_key(workspace);
    self.cache.remove(&cache_key);
    let mut inner = self.inner.borrow_mut();
    inner.errors.remove(&cache_key);
    inner.in_flight.remove(&cache_key);
}
```

### 2. Integrate with SSE Handler
Call `invalidate_workspace` when `ticket.upsert` events are received:
- Option A: Call from `subscribe_sse` closure (requires passing `GraphFetchService` reference)
- Option B: Let cache miss trigger fresh fetch (simpler, recommended)

**Recommended Approach**: Let cache miss trigger fresh fetch. When `fetch_trigger` increments, the graph component will re-fetch. If cache is stale, fresh data will be fetched automatically.

### 3. Alternative: Selective Cache Update
For more sophisticated approach (optional):
- Parse ticket ID from SSE event
- Update specific node in cached layout
- Requires modifying `GraphLayout` to support partial updates

**Recommend simple cache miss approach** for Phase 1.

## Files to Modify
- `memory-viewers/ticket-viewer/frontend/dioxus/src/graph_fetch.rs` (add invalidation method)
- `memory-viewers/ticket-viewer/frontend/dioxus/src/components/dep_graph/page.rs` (optional integration)

## Code Changes
```rust
// In graph_fetch.rs, add to GraphFetchService impl:
pub fn invalidate_workspace(&self, workspace: &str) {
    let cache_key = workspace_cache_key(workspace);
    self.cache.remove(&cache_key);
    let mut inner = self.inner.borrow_mut();
    inner.errors.remove(&cache_key);
    inner.in_flight.remove(&cache_key);
    // Also bump version to trigger UI updates
    self.version += 1;
}
```

## Acceptance Criteria
- [ ] `GraphFetchService` has `invalidate_workspace` method
- [ ] Cache is cleared when workspace data becomes stale
- [ ] Graph re-fetches fresh layout after invalidation
- [ ] No memory leaks or dangling references
- [ ] Works with existing `GraphFetchService` state management
- [ ] Manual test: ticket update triggers fresh graph fetch
## Problem
When a ticket state is changed in the details panel, the graph nodes don't update their visual representation (color, label, etc.). The graph only listens to `edge.*` SSE events, not `ticket.*` events.

## Current Behavior
1. Ticket detail panel emits `ticket.upsert` SSE events on state changes
2. List view listens to `ticket.upsert` and refreshes
3. Graph only listens to `edge.upsert`/`edge.delete` events
4. Graph nodes remain visually stale

## Expected Behavior
Graph nodes should update their visual state (color based on state, labels, etc.) when ticket data changes, not just when edges change.

## Root Cause Analysis
1. **SSE Subscription Gap**: `subscribe_sse` in `dep_graph/state.rs` only registers listeners for `edge.upsert` and `edge.delete`, not `ticket.upsert`.
2. **Cache Invalidation**: `GraphFetchService` caches workspace graph layouts but doesn't invalidate cache when ticket data changes.
3. **Node Visual Updates**: Graph3D component uses cached layout data; when layout updates, nodes should reflect new ticket state/color.

## Solution Design

### 1. Update SSE Subscription
Modify `subscribe_sse` in `dep_graph/state.rs` to also listen for `ticket.upsert` events:
```rust
pub(super) fn subscribe_sse(
    workspace: &str,
    fetch_trigger: Signal<u32>,
) -> Option<DepSseHandle> {
    let url = format!("/api/stream?workspace={workspace}");
    let event_source = web_sys::EventSource::new(&url).ok()?;
    let mut trigger_upsert = fetch_trigger;
    let mut trigger_delete = fetch_trigger;
    let mut trigger_ticket = fetch_trigger;  // NEW
    
    let ticket_listener = gloo_events::EventListener::new(
        &event_source,
        "ticket.upsert",
        move |_| {
            trigger_ticket.with_mut(|value| *value += 1);
        },
    );
    let upsert_listener = gloo_events::EventListener::new(
        &event_source,
        "edge.upsert",
        move |_| {
            trigger_upsert.with_mut(|value| *value += 1);
        },
    );
    let delete_listener = gloo_events::EventListener::new(
        &event_source,
        "edge.delete",
        move |_| {
            trigger_delete.with_mut(|value| *value += 1);
        },
    );

    Some(DepSseHandle {
        es: event_source,
        _listeners: [ticket_listener, upsert_listener, delete_listener],  // Updated
    })
}
```

### 2. Cache Invalidation Strategy
Option A: Invalidate workspace cache on `ticket.upsert`:
- When `ticket.upsert` received, remove workspace cache entry
- Next graph render triggers fresh fetch

Option B: Selective update (more complex):
- Parse ticket ID from SSE event
- Update specific node in cached layout
- Requires modifying `GraphLayout` to support partial updates

**Recommend Option A** for simplicity and consistency with existing patterns.

### 3. Graph3D Node Visual Updates
The Graph3D component already re-renders when layout changes (via `version` signal). When cache is invalidated and fresh data fetched:
- New layout contains updated ticket state/title
- Node colors update via `ticket_card::state_color()`
- Labels update from new ticket data

## Implementation Plan

### Phase 1: Update SSE Subscription
1. Modify `dep_graph/state.rs` to listen for `ticket.upsert`
2. Update `DepSseHandle` to store 3 listeners instead of 2
3. Test that `fetch_trigger` increments on ticket updates

### Phase 2: Cache Invalidation
1. Add method to `GraphFetchService` to invalidate workspace cache:
   ```rust
   pub fn invalidate_workspace(&self, workspace: &str) {
       let cache_key = workspace_cache_key(workspace);
       self.cache.remove(&cache_key);
       self.inner.borrow_mut().errors.remove(&cache_key);
   }
   ```
2. Call this from SSE handler when `ticket.upsert` received
3. Alternative: Just let cache miss trigger fresh fetch

### Phase 3: Testing
1. Manual test: Change ticket state in detail panel, verify graph updates
2. Add unit test for SSE subscription updates
3. Verify no performance regression

## Files to Modify
- `memory-viewers/ticket-viewer/frontend/dioxus/src/components/dep_graph/state.rs`
- `memory-viewers/ticket-viewer/frontend/dioxus/src/graph_fetch.rs` (optional cache invalidation)
- `memory-viewers/ticket-viewer/frontend/dioxus/src/components/dep_graph/page.rs` (if needed)

## Acceptance Criteria
- [ ] Graph nodes update color when ticket state changes
- [ ] Graph nodes update labels when ticket title changes
- [ ] No performance regression (efficient updates)
- [ ] Works with both 2D and 3D graph modes
- [ ] Test with ticket state transitions in detail panel
- [ ] Edge updates still work (backward compatibility)
## Parent Ticket
Depends on: [111510f4 Fix graph reactivity: ticket state changes don't update graph nodes](C:/Users/linus_behrbohm/git/SECOND_CHECKOUT/graph_app/context-engine/memory-viewers/.ticket/tickets/111510f4-c74b-4819-800b-d68ab013a73c/ticket.toml)

## Problem
The graph component only listens to `edge.upsert` and `edge.delete` SSE events, but not `ticket.upsert` events. When a ticket state changes in the details panel, the graph nodes remain visually stale because they don't receive updates.

## Current Implementation
`subscribe_sse` in `dep_graph/state.rs`:
```rust
pub(super) fn subscribe_sse(
    workspace: &str,
    fetch_trigger: Signal<u32>,
) -> Option<DepSseHandle> {
    let url = format!("/api/stream?workspace={workspace}");
    let event_source = web_sys::EventSource::new(&url).ok()?;
    let mut trigger_upsert = fetch_trigger;
    let mut trigger_delete = fetch_trigger;
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
        _listeners: [upsert_listener, delete_listener],
    })
}
```

## Implementation Tasks

### 1. Update SSE Subscription
Modify `subscribe_sse` to also listen for `ticket.upsert` events:
- Add a third listener for `ticket.upsert`
- Update `DepSseHandle` to store 3 listeners instead of 2
- Ensure `fetch_trigger` increments when `ticket.upsert` received

### 2. Update Data Structures
- Update `DepSseHandle._listeners` from `[gloo_events::EventListener; 2]` to `[gloo_events::EventListener; 3]`
- Update array initialization to include all three listeners

### 3. Test Implementation
- Verify that `fetch_trigger` increments when `ticket.upsert` events arrive
- Test that graph re-fetches layout when ticket data changes
- Ensure backward compatibility with existing `edge.*` events

## Files to Modify
- `memory-viewers/ticket-viewer/frontend/dioxus/src/components/dep_graph/state.rs`

## Code Changes
```rust
pub(super) fn subscribe_sse(
    workspace: &str,
    fetch_trigger: Signal<u32>,
) -> Option<DepSseHandle> {
    let url = format!("/api/stream?workspace={workspace}");
    let event_source = web_sys::EventSource::new(&url).ok()?;
    let mut trigger_ticket = fetch_trigger;
    let mut trigger_upsert = fetch_trigger;
    let mut trigger_delete = fetch_trigger;
    
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
        _listeners: [ticket_listener, upsert_listener, delete_listener],
    })
}
```

## Acceptance Criteria
- [ ] Graph listens for `ticket.upsert` SSE events
- [ ] `fetch_trigger` increments when ticket data changes
- [ ] Graph re-fetches layout on ticket updates
- [ ] Existing `edge.*` event handling remains unchanged
- [ ] No compilation errors or warnings
- [ ] Manual test: change ticket state in details panel, verify graph updates
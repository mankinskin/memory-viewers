//! `use_sse` — Dioxus hook for live ticket updates via Server-Sent Events.
//!
//! Connects to `GET /api/stream?workspace={workspace}` and keeps the provided
//! `tickets` [`Signal`] up-to-date with incoming events:
//!
//! | SSE event       | Action                                             |
//! |-----------------|----------------------------------------------------|
//! | `ticket.upsert` | Update state/title in-place or append a new entry. |
//! | `ticket.delete` | Remove the entry by ID.                            |
//! | `edge.*`        | Acknowledged, no ticket-list mutation.             |
//! | `open`          | Reset backoff to the initial value.                |
//! | `error`         | Close the connection, schedule reconnect w/ backoff. |
//!
//! Reconnect uses exponential backoff: 1 s → 2 s → 4 s → … capped at 30 s.
//!
//! # Auth note
//!
//! `EventSource` cannot send custom request headers, so the `/api/stream`
//! endpoint intentionally carries no authentication middleware.  No changes
//! are needed here.

use dioxus::prelude::*;
use gloo_events::EventListener;
use serde::Deserialize;
use wasm_bindgen::JsCast;

use crate::types::TicketSummary;

// ── SSE payload types (frontend deserialisation) ───────────────────────────

/// Minimal ticket snapshot embedded in `ticket.upsert` events.
#[derive(Debug, Deserialize)]
struct SseTicket {
    id: String,
    state: Option<String>,
    title: Option<String>,
}

/// Data payload for `ticket.upsert` SSE events.
#[derive(Debug, Deserialize)]
struct UpsertPayload {
    ticket: SseTicket,
}

/// Data payload for `ticket.delete` SSE events.
#[derive(Debug, Deserialize)]
struct DeletePayload {
    /// UUID of the deleted ticket, serialised as a string by the server.
    id: String,
}

// ── SseHandle ─────────────────────────────────────────────────────────────

/// RAII owner of a live SSE session.
///
/// Dropping this value:
/// 1. Calls [`web_sys::EventSource::close()`], preventing the browser from
///    attempting its own auto-reconnect.
/// 2. Drops each [`EventListener`], which removes the JS event listeners from
///    the `EventTarget` (no `Closure::forget()` calls anywhere in this file).
struct SseHandle {
    es: web_sys::EventSource,
    /// Must be held alive — gloo removes listeners from the DOM on drop.
    _listeners: Vec<EventListener>,
}

impl Drop for SseHandle {
    fn drop(&mut self) {
        self.es.close();
    }
}

// ── Backoff parameters ─────────────────────────────────────────────────────

const BACKOFF_INITIAL_MS: u32 = 1_000;
const BACKOFF_MAX_MS: u32 = 30_000;

// ── Browser sleep via JS setTimeout ───────────────────────────────────────

/// Suspend the current async task for `ms` milliseconds.
///
/// Uses a [`js_sys::Promise`] resolved by `window.setTimeout` so the JS event
/// loop stays unblocked during the wait.
async fn sleep_ms(ms: u32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        web_sys::window()
            .expect("no window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32)
            .expect("setTimeout failed");
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

// ── Hook ───────────────────────────────────────────────────────────────────

/// Dioxus hook: open an SSE connection to `/api/stream?workspace={workspace}`
/// and live-update `tickets` from incoming events.
///
/// Call this once at the top of the component that owns the ticket list, e.g.
/// `TicketListPage`.  The connection is torn down automatically when the
/// component unmounts.
///
/// # Arguments
///
/// * `workspace` — name of the workspace to subscribe to (used as a query-
///   string parameter).
/// * `tickets` — signal that holds the displayed ticket list; the hook mutates
///   it in response to `ticket.upsert` and `ticket.delete` events.
pub fn use_sse(workspace: String, tickets: Signal<Vec<TicketSummary>>) {
    // Monotonic counter — incrementing this value re-runs the effect, which
    // drops the old `SseHandle` (closing the EventSource) and opens a fresh one.
    let mut reconnect_gen: Signal<u32> = use_signal(|| 0_u32);

    // Current reconnect delay in milliseconds; doubled after each error.
    let mut backoff_ms: Signal<u32> = use_signal(|| BACKOFF_INITIAL_MS);

    // Keeps the live `SseHandle` alive for the component's lifetime.
    // The Option allows us to drop it explicitly before creating the next one.
    let mut handle: Signal<Option<SseHandle>> = use_signal(|| None);

    use_effect(move || {
        // Reading `reconnect_gen` subscribes this effect to its changes, so
        // the effect body re-runs each time we increment it for a reconnect.
        let _gen = reconnect_gen();

        // Drop the previous SseHandle before opening the next connection.
        // `SseHandle::drop` calls `es.close()`, preventing the browser's own
        // auto-reconnect from racing with ours.
        handle.with_mut(|h| *h = None);

        let url = format!("/api/stream?workspace={workspace}");
        let es = match web_sys::EventSource::new(&url) {
            Ok(es) => es,
            Err(e) => {
                tracing::error!("EventSource::new failed for {workspace}: {:?}", e);
                return;
            }
        };

        // ── ticket.upsert ──────────────────────────────────────────────────
        // Update state/title in-place for known tickets, or append a minimal
        // stub for tickets that arrived before the initial HTTP list completed.
        let mut tix_up = tickets;
        let l_upsert = EventListener::new(&es, "ticket.upsert", move |event| {
            let data = msg_data(event);
            match serde_json::from_str::<UpsertPayload>(&data) {
                Ok(p) => tix_up.with_mut(|v| {
                    if let Some(existing) = v.iter_mut().find(|t| t.id == p.ticket.id) {
                        existing.state = p.ticket.state;
                        if p.ticket.title.is_some() {
                            existing.title = p.ticket.title;
                        }
                    } else {
                        v.push(TicketSummary {
                            id: p.ticket.id,
                            title: p.ticket.title,
                            state: p.ticket.state,
                            ticket_type: None,
                            created_at: String::new(),
                            updated_at: String::new(),
                            fields: serde_json::Value::Null,
                        });
                    }
                }),
                Err(e) => tracing::warn!("ticket.upsert parse error: {e}"),
            }
        });

        // ── ticket.delete ──────────────────────────────────────────────────
        let mut tix_del = tickets;
        let l_delete = EventListener::new(&es, "ticket.delete", move |event| {
            let data = msg_data(event);
            match serde_json::from_str::<DeletePayload>(&data) {
                Ok(p) => tix_del.with_mut(|v| v.retain(|t| t.id != p.id)),
                Err(e) => tracing::warn!("ticket.delete parse error: {e}"),
            }
        });

        // ── edge events ────────────────────────────────────────────────────
        // Acknowledged without mutating the ticket list.  The dep-graph
        // component has its own SSE subscription that handles edge events.
        let l_edge_up = EventListener::new(&es, "edge.upsert", move |_| {});
        let l_edge_del = EventListener::new(&es, "edge.delete", move |_| {});

        // ── open — reset backoff ───────────────────────────────────────────
        let mut boff_open = backoff_ms;
        let l_open = EventListener::new(&es, "open", move |_| {
            boff_open.set(BACKOFF_INITIAL_MS);
            tracing::debug!("SSE connected; backoff reset");
        });

        // ── error — close and schedule reconnect with exponential backoff ──
        //
        // Clone the JsValue-backed EventSource so the closure can call
        // `close()` immediately, preventing the browser's own automatic
        // reconnect from competing with our backoff schedule.
        let es_close = es.clone();
        let mut rgen = reconnect_gen;
        let mut boff_err = backoff_ms;
        let l_error = EventListener::new(&es, "error", move |_| {
            // Prevent the browser's built-in SSE reconnect.
            es_close.close();

            let delay = *boff_err.peek();
            let next_delay = (delay * 2).min(BACKOFF_MAX_MS);
            boff_err.set(next_delay);
            tracing::debug!("SSE error; reconnecting in {delay} ms (next backoff {next_delay} ms)");

            spawn(async move {
                sleep_ms(delay).await;
                // Incrementing `reconnect_gen` re-triggers the enclosing
                // `use_effect`, which drops the old SseHandle and opens a
                // fresh EventSource.
                rgen.with_mut(|v| *v += 1);
            });
        });

        handle.with_mut(|h| {
            *h = Some(SseHandle {
                es,
                _listeners: vec![
                    l_upsert, l_delete, l_edge_up, l_edge_del, l_open, l_error,
                ],
            });
        });
    });
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Extract the `data` string from a DOM `MessageEvent`.
///
/// Returns an empty string if the event is not a `MessageEvent` or if the
/// data field is not a plain string (we never expect binary payloads here).
fn msg_data(event: &web_sys::Event) -> String {
    event
        .dyn_ref::<web_sys::MessageEvent>()
        .and_then(|msg| msg.data().as_string())
        .unwrap_or_default()
}

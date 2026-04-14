//! Ticket detail sidebar panel with inline click-to-edit fields.
//!
//! Renders as a fixed left panel (300 px) showing all editable fields from
//! the tracker-improvement schema.  Each field row supports click-to-edit:
//!
//! * Optimistic update gate — further edits are blocked until the PATCH
//!   request returns.
//! * Rollback on server error — local state reverts to the last known-good
//!   value by re-fetching the ticket.
//! * SSE conflict detection — if the server pushes a `ticket.upsert` event
//!   for this ticket while the user is editing a field, a conflict dialog
//!   appears showing the server value vs. the local draft.

use dioxus::prelude::*;
use gloo_events::EventListener;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use wasm_bindgen::JsCast as _;

use crate::backend::{HttpTicketBackend, TicketBackend, TicketPatch, TypeSchema};

// ── Known enum values ─────────────────────────────────────────────────────

const PRIORITY_OPTIONS: &[&str] = &["", "none", "low", "medium", "high", "critical"];
const RISK_LEVEL_OPTIONS: &[&str] = &["", "none", "low", "medium", "high", "critical"];

fn static_options(key: &str) -> &'static [&'static str] {
    match key {
        "priority" => PRIORITY_OPTIONS,
        "risk_level" => RISK_LEVEL_OPTIONS,
        _ => &[],
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn field_str(fields: &serde_json::Value, key: &str) -> String {
    fields
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn field_bool(fields: &serde_json::Value, key: &str) -> bool {
    fields
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

// ── StateBadge ────────────────────────────────────────────────────────────

fn state_colors(state: &str) -> (&'static str, &'static str) {
    match state {
        "new" => ("#2d2d4a", "#a0a0c8"),
        "ready" => ("#1a3d28", "#86efac"),
        "in-implementation" => ("#3d2e1a", "#fbbf24"),
        "in-review" => ("#361a4a", "#c084fc"),
        "done" => ("#1a3d28", "#4ade80"),
        "cancelled" => ("#3d1a1a", "#f87171"),
        _ => ("#2a2a3a", "#9ca3af"),
    }
}

#[component]
fn StateBadge(state: String) -> Element {
    let (bg, fg) = state_colors(&state);
    rsx! {
        span {
            style: "
                display: inline-block;
                padding: 3px 10px;
                border-radius: 12px;
                font-size: 11px;
                font-weight: 600;
                letter-spacing: 0.04em;
                background: {bg};
                color: {fg};
                white-space: nowrap;
            ",
            "{state}"
        }
    }
}

// ── ConflictDialog ────────────────────────────────────────────────────────

#[component]
fn ConflictDialog(
    field_label: String,
    server_value: String,
    my_draft: String,
    on_discard: EventHandler<()>,
    on_keep: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            style: "
                position: fixed; inset: 0; z-index: 1000;
                background: rgba(0,0,0,0.6);
                display: flex; align-items: center; justify-content: center;
            ",
            div {
                style: "
                    background: #1e1e2d; border: 1px solid #4b4b6a;
                    border-radius: 8px; padding: 1.5rem; width: 360px;
                    color: #e0e0e8; font-family: sans-serif;
                ",
                h3 {
                    style: "margin: 0 0 0.75rem; font-size: 14px; color: #f59e0b;",
                    "Concurrent edit conflict"
                }
                p {
                    style: "font-size: 13px; margin: 0 0 0.75rem;",
                    "\"",
                    "{field_label}",
                    "\" was updated on the server while you were editing."
                }
                div {
                    style: "
                        font-size: 12px; background: #13131f;
                        border-radius: 4px; padding: 0.5rem; margin-bottom: 1rem;
                    ",
                    p { style: "margin: 0 0 0.25rem; color: #a0a0c0;", "Server value:" }
                    p { style: "margin: 0 0 0.5rem; font-weight: 600;", "{server_value}" }
                    p { style: "margin: 0 0 0.25rem; color: #a0a0c0;", "Your draft:" }
                    p { style: "margin: 0; font-weight: 600;", "{my_draft}" }
                }
                div {
                    style: "display: flex; gap: 0.5rem; justify-content: flex-end;",
                    button {
                        style: "
                            padding: 6px 14px; border-radius: 4px; border: none;
                            background: #3c3c5a; color: #e0e0e8; cursor: pointer;
                            font-size: 12px;
                        ",
                        onclick: move |_| on_keep.call(()),
                        "Keep editing"
                    }
                    button {
                        style: "
                            padding: 6px 14px; border-radius: 4px; border: none;
                            background: #ef4444; color: white; cursor: pointer;
                            font-size: 12px;
                        ",
                        onclick: move |_| on_discard.call(()),
                        "Discard my changes"
                    }
                }
            }
        }
    }
}

// ── FieldRow — controlled single-field editor ─────────────────────────────

#[derive(Props, Clone, PartialEq)]
struct FieldRowProps {
    field_key: String,
    label: String,
    current_value: String,
    /// Non-empty → render as `<select>` with these options.
    options: Vec<String>,
    is_editing: bool,
    draft: String,
    is_pending: bool,
    on_start_edit: EventHandler<String>,
    on_draft_change: EventHandler<String>,
    /// Called with the new value to save and apply.
    on_save: EventHandler<String>,
    on_cancel: EventHandler<()>,
}

#[component]
fn FieldRow(props: FieldRowProps) -> Element {
    let FieldRowProps {
        field_key,
        label,
        current_value,
        options,
        is_editing,
        draft,
        is_pending,
        on_start_edit,
        on_draft_change,
        on_save,
        on_cancel,
    } = props;

    // Pre-compute values used in the display branch.
    let display_opacity = if is_pending { "0.5" } else { "1" };

    rsx! {
        div {
            style: "margin-bottom: 0.5rem;",
            div {
                style: "
                    font-size: 10px; color: #6b7280; text-transform: uppercase;
                    letter-spacing: 0.05em; margin-bottom: 2px;
                ",
                "{label}"
            }

            if is_editing {
                if !options.is_empty() {
                    // Enum field — immediate save on change.
                    select {
                        style: "
                            width: 100%; background: #13131f; color: #e0e0e8;
                            border: 1px solid #4b4b6a; border-radius: 4px;
                            padding: 4px 6px; font-size: 13px; cursor: pointer;
                        ",
                        disabled: is_pending,
                        onchange: move |evt: Event<FormData>| {
                            on_save.call(evt.value().to_string());
                        },
                        for opt in options.iter() {
                            option {
                                key: "{opt}",
                                value: "{opt}",
                                selected: *opt == current_value,
                                "{opt}"
                            }
                        }
                    }
                } else {
                    // Free-text field — Enter/blur to save, Escape to cancel.
                    div {
                        style: "display: flex; gap: 4px;",
                        input {
                            r#type: "text",
                            value: "{draft}",
                            autofocus: true,
                            style: "
                                flex: 1; background: #13131f; color: #e0e0e8;
                                border: 1px solid #4b4b6a; border-radius: 4px;
                                padding: 4px 6px; font-size: 13px; min-width: 0;
                            ",
                            disabled: is_pending,
                            oninput: move |evt| on_draft_change.call(evt.value().to_string()),
                            onkeydown: {
                                let draft2 = draft.clone();
                                move |evt: Event<KeyboardData>| match evt.key() {
                                    Key::Enter => on_save.call(draft2.clone()),
                                    Key::Escape => on_cancel.call(()),
                                    _ => {}
                                }
                            },
                        }
                        button {
                            style: "
                                padding: 3px 8px; border-radius: 4px; border: none;
                                background: #3b82f6; color: white;
                                cursor: pointer; font-size: 11px;
                            ",
                            disabled: is_pending,
                            onclick: {
                                let draft3 = draft.clone();
                                move |_| on_save.call(draft3.clone())
                            },
                            "✓"
                        }
                        button {
                            style: "
                                padding: 3px 8px; border-radius: 4px; border: none;
                                background: #3c3c5a; color: #e0e0e8;
                                cursor: pointer; font-size: 11px;
                            ",
                            onclick: move |_| on_cancel.call(()),
                            "✕"
                        }
                    }
                }
            } else {
                // Display mode — click to start editing.
                div {
                    style: "
                        font-size: 13px; color: #c0c0d8;
                        padding: 4px 6px; border-radius: 4px;
                        border: 1px solid transparent; min-height: 26px;
                        cursor: pointer; opacity: {display_opacity};
                    ",
                    onclick: {
                        let cv = current_value.clone();
                        let fk = field_key.clone();
                        move |_| {
                            if !is_pending {
                                on_start_edit.call(fk.clone());
                                on_draft_change.call(cv.clone());
                            }
                        }
                    },
                    if current_value.is_empty() {
                        span { style: "color: #4b4b6a; font-style: italic;", "—" }
                    } else {
                        "{current_value}"
                    }
                }
            }
        }
    }
}

// ── TicketDetail ──────────────────────────────────────────────────────────

/// Fixed left-sidebar showing all editable fields for the given ticket.
#[component]
pub fn TicketDetail(workspace: String, id: String) -> Element {
    let backend = HttpTicketBackend::new(None);

    // ── Data signals ──────────────────────────────────────────────────
    let mut ticket_fields: Signal<serde_json::Value> = use_signal(|| serde_json::Value::Null);
    let mut load_error: Signal<Option<String>> = use_signal(|| None);

    // ── Inline-editing state ──────────────────────────────────────────
    /// Which field key is currently open for editing, if any.
    let mut editing_field: Signal<Option<String>> = use_signal(|| None);
    /// Live draft value being typed.
    let mut draft_value: Signal<String> = use_signal(String::new);
    /// True while a PATCH request is in-flight (gate further edits).
    let mut save_pending: Signal<bool> = use_signal(|| false);
    /// Last save error message, if any.
    let mut save_error: Signal<Option<String>> = use_signal(|| None);

    // ── State-machine transition state ────────────────────────────────
    /// Schema for the ticket's type (drives transition options).
    let mut schema: Signal<Option<TypeSchema>> = use_signal(|| None);
    /// States that have appeared in this ticket's history (for required_states check).
    let mut visited_states: Signal<Vec<String>> = use_signal(Vec::new);
    /// True while a state transition PATCH is in-flight.
    let mut transition_pending: Signal<bool> = use_signal(|| false);
    /// Last transition error, if any.
    let mut transition_error: Signal<Option<String>> = use_signal(|| None);

    // ── Conflict detection ────────────────────────────────────────────
    /// (field_key, server_value, my_draft) when a conflict is detected.
    let mut conflict: Signal<Option<(String, String, String)>> = use_signal(|| None);

    // ── SSE handle (kept alive) ───────────────────────────────────────
    let mut _sse: Signal<Option<(web_sys::EventSource, EventListener)>> = use_signal(|| None);

    // ── Fetch ticket, schema, and history on mount ────────────────────
    {
        let b = backend.clone();
        let ws = workspace.clone();
        let tid = id.clone();
        use_effect(move || {
            let b = b.clone();
            let ws = ws.clone();
            let tid = tid.clone();
            spawn(async move {
                match b.get_ticket(&ws, &tid).await {
                    Ok(resp) => {
                        let fields = resp.ticket.fields.clone();
                        let type_id = fields
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        ticket_fields.set(fields);
                        // Fetch schema now that we know the type.
                        if !type_id.is_empty() {
                            if let Ok(sr) = b.get_schema_by_type(&ws, &type_id).await {
                                schema.set(Some(sr.schema));
                            }
                        }
                    }
                    Err(e) => load_error.set(Some(e)),
                }
                // Fetch history to know visited states.
                if let Ok(hr) = b.get_ticket_history(&ws, &tid).await {
                    let mut seen = std::collections::HashSet::new();
                    let unique: Vec<String> = hr
                        .entries
                        .iter()
                        .filter_map(|e| {
                            e.fields
                                .get("state")
                                .and_then(|v| v.as_str())
                                .map(str::to_string)
                        })
                        .filter(|s| seen.insert(s.clone()))
                        .collect();
                    visited_states.set(unique);
                }
            });
        });
    }

    // ── SSE for conflict detection ────────────────────────────────────
    {
        let ws = workspace.clone();
        let my_id = id.clone();
        use_effect(move || {
            let enc_ws = utf8_percent_encode(&ws, NON_ALPHANUMERIC).to_string();
            let url = format!("/api/stream?workspace={enc_ws}");
            if let Ok(es) = web_sys::EventSource::new(&url) {
                let my_id_inner = my_id.clone();
                let listener = gloo_events::EventListener::new(
                    &es,
                    "ticket.upsert",
                    move |evt| {
                        let msg = match evt.dyn_ref::<web_sys::MessageEvent>() {
                            Some(m) => m,
                            None => return,
                        };
                        let data_str = match msg.data().as_string() {
                            Some(s) => s,
                            None => return,
                        };
                        let parsed: serde_json::Value =
                            match serde_json::from_str(&data_str) {
                                Ok(v) => v,
                                Err(_) => return,
                            };
                        // Only react to this specific ticket.
                        let evt_id = parsed
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if !evt_id.starts_with(&my_id_inner[..8]) && !my_id_inner.starts_with(evt_id) {
                            return;
                        }
                        let server_fields =
                            parsed.get("fields").cloned().unwrap_or(serde_json::Value::Null);
                        // If currently editing, check for a conflict.
                        let fk = editing_field();
                        if let Some(fk) = fk {
                            let server_val = field_str(&server_fields, &fk);
                            let my_draft = draft_value();
                            if server_val != my_draft {
                                conflict.set(Some((fk, server_val, my_draft)));
                                return; // don't overwrite local fields while editing
                            }
                        }
                        // Silently update fields when not editing.
                        if editing_field().is_none() {
                            ticket_fields.set(server_fields);
                        }
                    },
                );
                _sse.set(Some((es, listener)));
            }
        });
    }

    // ── Save a field value via PATCH ──────────────────────────────────
    let do_save = {
        let b = backend.clone();
        let ws = workspace.clone();
        let tid = id.clone();
        move |field_key: String, new_value: String| {
            // Optimistic update.
            ticket_fields.with_mut(|f| {
                if let serde_json::Value::Object(ref mut map) = f {
                    map.insert(field_key.clone(), serde_json::Value::String(new_value.clone()));
                }
            });
            editing_field.set(None);
            save_pending.set(true);
            save_error.set(None);

            let b = b.clone();
            let ws = ws.clone();
            let tid = tid.clone();
            spawn(async move {
                let patch = TicketPatch {
                    workspace: ws.clone(),
                    state: None,
                    title: if field_key == "title" {
                        Some(new_value.clone())
                    } else {
                        None
                    },
                    fields: if field_key != "title" {
                        let mut map = serde_json::Map::new();
                        map.insert(field_key, serde_json::Value::String(new_value));
                        Some(serde_json::Value::Object(map))
                    } else {
                        None
                    },
                };
                match b.patch_ticket(&ws, &tid, &patch).await {
                    Ok(resp) => {
                        ticket_fields.set(resp.ticket.fields);
                        save_pending.set(false);
                    }
                    Err(e) => {
                        save_pending.set(false);
                        save_error.set(Some(e));
                        // Rollback: refetch the ticket.
                        if let Ok(resp) = b.get_ticket(&ws, &tid).await {
                            ticket_fields.set(resp.ticket.fields);
                        }
                    }
                }
            });
        }
    };

    // ── Save boolean field via PATCH ──────────────────────────────────
    let do_save_bool = {
        let b = backend.clone();
        let ws = workspace.clone();
        let tid = id.clone();
        move |field_key: String, checked: bool| {
            ticket_fields.with_mut(|f| {
                if let serde_json::Value::Object(ref mut map) = f {
                    map.insert(field_key.clone(), serde_json::Value::Bool(checked));
                }
            });
            save_pending.set(true);
            save_error.set(None);

            let b = b.clone();
            let ws = ws.clone();
            let tid = tid.clone();
            spawn(async move {
                let mut fmap = serde_json::Map::new();
                fmap.insert(field_key.clone(), serde_json::Value::Bool(checked));
                let patch = TicketPatch {
                    workspace: ws.clone(),
                    state: None,
                    title: None,
                    fields: Some(serde_json::Value::Object(fmap)),
                };
                match b.patch_ticket(&ws, &tid, &patch).await {
                    Ok(resp) => {
                        ticket_fields.set(resp.ticket.fields);
                        save_pending.set(false);
                    }
                    Err(e) => {
                        save_pending.set(false);
                        save_error.set(Some(e));
                        if let Ok(resp) = b.get_ticket(&ws, &tid).await {
                            ticket_fields.set(resp.ticket.fields);
                        }
                    }
                }
            });
        }
    };

    // ── State transition via PATCH ────────────────────────────────────
    let do_transition = {
        let b = backend.clone();
        let ws = workspace.clone();
        let tid = id.clone();
        move |new_state: String| {
            // Optimistic update: flip state immediately.
            let prev_state = ticket_fields()
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ticket_fields.with_mut(|f| {
                if let serde_json::Value::Object(ref mut map) = f {
                    map.insert("state".to_string(), serde_json::Value::String(new_state.clone()));
                }
            });
            transition_pending.set(true);
            transition_error.set(None);

            let b = b.clone();
            let ws = ws.clone();
            let tid = tid.clone();
            let new_state_cap = new_state.clone();
            spawn(async move {
                let patch = TicketPatch {
                    workspace: ws.clone(),
                    state: Some(new_state_cap.clone()),
                    title: None,
                    fields: None,
                };
                match b.patch_ticket(&ws, &tid, &patch).await {
                    Ok(resp) => {
                        ticket_fields.set(resp.ticket.fields.clone());
                        transition_pending.set(false);
                        // Record the new state as visited.
                        let reached = resp.ticket.fields
                            .get("state")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if !reached.is_empty() {
                            visited_states.with_mut(|vs| {
                                if !vs.contains(&reached) {
                                    vs.push(reached);
                                }
                            });
                        }
                    }
                    Err(e) => {
                        // Revert optimistic state change.
                        ticket_fields.with_mut(|f| {
                            if let serde_json::Value::Object(ref mut map) = f {
                                map.insert("state".to_string(), serde_json::Value::String(prev_state.clone()));
                            }
                        });
                        transition_pending.set(false);
                        transition_error.set(Some(e));
                    }
                }
            });
        }
    };

    // ── Undo last transition ──────────────────────────────────────────
    let do_undo = {
        let b = backend.clone();
        let ws = workspace.clone();
        let tid = id.clone();
        move || {
            transition_pending.set(true);
            transition_error.set(None);

            let b = b.clone();
            let ws = ws.clone();
            let tid = tid.clone();
            spawn(async move {
                match b.undo_ticket(&ws, &tid).await {
                    Ok(resp) => {
                        ticket_fields.set(resp.ticket.fields);
                        transition_pending.set(false);
                        // Refresh visited states from history after undo.
                        if let Ok(hr) = b.get_ticket_history(&ws, &tid).await {
                            let mut seen = std::collections::HashSet::new();
                            let unique: Vec<String> = hr
                                .entries
                                .iter()
                                .filter_map(|e| {
                                    e.fields
                                        .get("state")
                                        .and_then(|v| v.as_str())
                                        .map(str::to_string)
                                })
                                .filter(|s| seen.insert(s.clone()))
                                .collect();
                            visited_states.set(unique);
                        }
                    }
                    Err(e) => {
                        transition_pending.set(false);
                        transition_error.set(Some(e));
                    }
                }
            });
        }
    };

    // ── Conflict resolution ───────────────────────────────────────────
    let on_discard = move |_| {
        if let Some((fk, server_val, _)) = conflict() {
            ticket_fields.with_mut(|f| {
                if let serde_json::Value::Object(ref mut map) = f {
                    map.insert(fk.clone(), serde_json::Value::String(server_val.clone()));
                }
            });
        }
        conflict.set(None);
        editing_field.set(None);
    };

    let on_keep = move |_| {
        // Re-open editing for the contested field.
        let fk = conflict().map(|(k, _, _)| k);
        conflict.set(None);
        editing_field.set(fk);
    };

    // ── Read current state for render ─────────────────────────────────
    let fields = ticket_fields();
    let cur_editing = editing_field();
    let cur_draft = draft_value();
    let is_pending = save_pending();

    // State machine derived state (computed before rsx! for cleaner templates).
    let cur_state = fields
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cur_schema = schema();
    let visited = visited_states();
    let is_trans_pending = transition_pending();
    let trans_error = transition_error();

    // Compute valid next-state transitions from the schema.
    let valid_nexts: Vec<String> = cur_schema
        .as_ref()
        .map(|s| {
            s.transitions
                .iter()
                .filter(|t| t.from == cur_state)
                .map(|t| t.to.clone())
                .collect()
        })
        .unwrap_or_default();

    // States required by schema that haven't been visited yet.
    let missing_required: Vec<String> = cur_schema
        .as_ref()
        .map(|s| {
            s.required_states
                .iter()
                .filter(|rs| !visited.contains(rs))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    // Terminal states according to the schema.
    let terminal_states: Vec<String> = cur_schema
        .as_ref()
        .map(|s| s.terminal_states.clone())
        .unwrap_or_default();

    // Field specs: (key, display label).
    let string_fields: &[(&str, &str)] = &[
        ("title", "Title"),
        ("priority", "Priority"),
        ("component", "Component"),
        ("risk_level", "Risk Level"),
        ("tags", "Tags"),
        ("workflow_stage", "Workflow Stage"),
        ("phase", "Phase"),
        ("doc_category", "Doc Category"),
        ("release_target", "Release Target"),
        ("release_version", "Release Version"),
        ("rollout_stage", "Rollout Stage"),
    ];

    rsx! {
        // ── Conflict dialog ──────────────────────────────────────────
        if let Some((fk, sv, md)) = conflict() {
            ConflictDialog {
                field_label: fk.clone(),
                server_value: sv.clone(),
                my_draft: md.clone(),
                on_discard: on_discard,
                on_keep: on_keep,
            }
        }

        // ── Sidebar panel ────────────────────────────────────────────
        div {
            style: "
                position: fixed; left: 0; top: 0; bottom: 0; width: 300px;
                z-index: 200; overflow-y: auto; overflow-x: hidden;
                background: rgba(15, 15, 28, 0.93);
                border-right: 1px solid #2a2a45;
                padding: 1rem;
                font-family: sans-serif;
            ",

            if let Some(err) = load_error() {
                p { style: "color: #ef4444; font-size: 12px;", "Error: {err}" }
            }

            if let Some(err) = save_error() {
                div {
                    style: "
                        background: rgba(239,68,68,0.15); border: 1px solid #ef4444;
                        border-radius: 4px; padding: 0.5rem; margin-bottom: 0.75rem;
                        font-size: 12px; color: #ef4444;
                    ",
                    "Save failed: {err}"
                }
            }

            // ── State machine section ────────────────────────────────
            div {
                style: "
                    margin-bottom: 1rem;
                    padding-bottom: 0.75rem;
                    border-bottom: 1px solid #2a2a45;
                ",

                // State badge row.
                div {
                    style: "margin-bottom: 0.75rem;",
                    div {
                        style: "
                            font-size: 10px; color: #6b7280;
                            text-transform: uppercase; letter-spacing: 0.05em;
                            margin-bottom: 4px;
                        ",
                        "State"
                    }
                    StateBadge { state: cur_state.clone() }
                }

                // Transition buttons.
                if !valid_nexts.is_empty() {
                    div {
                        style: "margin-bottom: 0.5rem;",
                        div {
                            style: "
                                font-size: 10px; color: #6b7280;
                                text-transform: uppercase; letter-spacing: 0.05em;
                                margin-bottom: 4px;
                            ",
                            "Advance to"
                        }
                        div {
                            style: "display: flex; flex-wrap: wrap; gap: 4px;",
                            for next_state in valid_nexts.iter() {
                                {
                                    let next = next_state.clone();
                                    let is_terminal = terminal_states.contains(&next);
                                    let is_blocked = is_terminal && !missing_required.is_empty();
                                    let tooltip = if is_blocked {
                                        format!(
                                            "Must visit first: {}",
                                            missing_required.join(", ")
                                        )
                                    } else {
                                        String::new()
                                    };
                                    let btn_style = if is_blocked {
                                        "padding: 4px 10px; border-radius: 4px;
                                         border: 1px solid #3a3a5a; background: #1e1e2d;
                                         color: #4b4b6a; font-size: 11px; cursor: not-allowed;
                                         opacity: 0.5;"
                                    } else {
                                        "padding: 4px 10px; border-radius: 4px;
                                         border: 1px solid #6366f1;
                                         background: rgba(99,102,241,0.15);
                                         color: #a5b4fc; font-size: 11px; cursor: pointer;"
                                    };
                                    let mut dt = do_transition.clone();
                                    rsx! {
                                        button {
                                            key: "{next}",
                                            title: "{tooltip}",
                                            disabled: is_blocked || is_trans_pending,
                                            style: "{btn_style}",
                                            onclick: move |_| {
                                                if !is_blocked && !is_trans_pending {
                                                    dt(next.clone());
                                                }
                                            },
                                            "{next_state}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Undo button.
                div {
                    style: "margin-bottom: 0.25rem;",
                    button {
                        disabled: is_trans_pending,
                        style: "
                            padding: 4px 10px; border-radius: 4px;
                            border: 1px solid #4b4b6a;
                            background: transparent; color: #a0a0c8;
                            font-size: 11px; cursor: pointer;
                        ",
                        onclick: {
                            let mut du = do_undo.clone();
                            move |_| du()
                        },
                        "↩ Undo"
                    }
                }

                // Transition error banner.
                if let Some(ref err) = trans_error {
                    div {
                        style: "
                            background: rgba(239,68,68,0.15);
                            border: 1px solid #ef4444;
                            border-radius: 4px; padding: 0.5rem;
                            margin-top: 0.4rem;
                            font-size: 12px; color: #ef4444;
                        ",
                        "Transition failed: {err}"
                    }
                }
            }

            // ── String / enum fields ─────────────────────────────────
            for (fk, lbl) in string_fields.iter() {
                {
                    let fk = fk.to_string();
                    let lbl = lbl.to_string();
                    let current = field_str(&fields, &fk);
                    let is_ed = cur_editing.as_deref() == Some(&fk);
                    let opts: Vec<String> = static_options(&fk)
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    rsx! {
                        FieldRow {
                            key: "{fk}",
                            field_key: fk.clone(),
                            label: lbl.clone(),
                            current_value: current.clone(),
                            options: opts,
                            is_editing: is_ed,
                            draft: if is_ed { cur_draft.clone() } else { current.clone() },
                            is_pending: is_pending,
                            on_start_edit: {
                                move |key: String| {
                                    editing_field.set(Some(key));
                                }
                            },
                            on_draft_change: move |v: String| { draft_value.set(v); },
                            on_save: {
                                let fk2 = fk.clone();
                                let mut do_save2 = do_save.clone();
                                move |v: String| do_save2(fk2.clone(), v)
                            },
                            on_cancel: move |_| { editing_field.set(None); },
                        }
                    }
                }
            }

            // ── bootstrap_blocker checkbox ───────────────────────────
            {
                let blocker = field_bool(&fields, "bootstrap_blocker");
                rsx! {
                    div {
                        style: "margin-top: 0.5rem;",
                        label {
                            style: "
                                display: flex; align-items: center; gap: 0.5rem;
                                cursor: pointer; font-size: 12px; color: #c0c0d8;
                            ",
                            input {
                                r#type: "checkbox",
                                checked: blocker,
                                disabled: is_pending,
                                onchange: {
                                    let mut dsb = do_save_bool.clone();
                                    move |evt: Event<FormData>| {
                                        dsb("bootstrap_blocker".to_string(), evt.value() == "true");
                                    }
                                },
                            }
                            "Bootstrap Blocker"
                        }
                    }
                }
            }
        }
    }
}

//! CreateTicketModal — dialog for composing and submitting a new ticket.
//!
//! # Features
//!
//! * Type selector populated from `GET /api/schema`.
//! * Dynamic field section driven by the selected type's schema definition:
//!   required fields show a red asterisk; optional fields are collapsible.
//! * Static fields always visible: title (text), description (textarea),
//!   priority (select), component (text).
//! * Auto-saves draft to `localStorage["draft_new_ticket"]` on every
//!   keystroke; clears on successful submit or cancel.
//! * Submits via `POST /api/tickets` (`HttpTicketBackend::create_ticket`).
//! * Shows inline server-validation errors.
//! * Redirects to the new ticket's detail page on success.
//! * `prefill: Option<TicketSummary>` prop pre-populates all fields (template
//!   mode).

use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::backend::{CreateTicketRequest, HttpTicketBackend, SchemaListResponse, TicketBackend, TicketSummary, TypeSchema};
use crate::routes::Route;

// ── Constants ─────────────────────────────────────────────────────────────────

const DRAFT_KEY: &str = "draft_new_ticket";
const PRIORITY_OPTIONS: &[&str] = &["", "none", "low", "medium", "high", "critical"];

// ── localStorage helpers ──────────────────────────────────────────────────────

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn save_draft(draft: &DraftState) {
    if let Some(store) = local_storage() {
        if let Ok(json) = serde_json::to_string(draft) {
            let _ = store.set_item(DRAFT_KEY, &json);
        }
    }
}

fn load_draft() -> Option<DraftState> {
    let store = local_storage()?;
    let raw = store.get_item(DRAFT_KEY).ok().flatten()?;
    serde_json::from_str(&raw).ok()
}

fn clear_draft() {
    if let Some(store) = local_storage() {
        let _ = store.remove_item(DRAFT_KEY);
    }
}

// ── DraftState ────────────────────────────────────────────────────────────────

/// All mutable form fields kept in a single serialisable value so we can
/// persist the entire draft in one `localStorage.setItem` call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
struct DraftState {
    type_id: String,
    title: String,
    description: String,
    priority: String,
    component: String,
    /// Extra schema-driven fields keyed by field name.
    extra: BTreeMap<String, String>,
}

impl Default for DraftState {
    fn default() -> Self {
        Self {
            type_id: String::new(),
            title: String::new(),
            description: String::new(),
            priority: String::new(),
            component: String::new(),
            extra: BTreeMap::new(),
        }
    }
}

impl DraftState {
    fn from_prefill(prefill: &TicketSummary) -> Self {
        let fields = &prefill.fields;
        Self {
            type_id: fields.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            title: prefill.title.clone().unwrap_or_default(),
            description: String::new(), // description is fetched separately if needed
            priority: fields.get("priority").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            component: fields.get("component").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            extra: fields
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .filter(|(k, _)| !matches!(k.as_str(), "type" | "priority" | "component" | "title" | "state"))
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct CreateTicketModalProps {
    pub workspace: String,
    /// When set, pre-populates the form (template mode).
    #[props(default)]
    pub prefill: Option<TicketSummary>,
    /// Called when the user cancels without submitting.
    pub on_cancel: EventHandler<()>,
}

// ── Component ─────────────────────────────────────────────────────────────────

/// Full-screen modal dialog for creating a new ticket.
#[component]
pub fn CreateTicketModal(props: CreateTicketModalProps) -> Element {
    let workspace = props.workspace.clone();
    let nav = use_navigator();
    let backend = HttpTicketBackend::new(None);

    // ── Schema data ───────────────────────────────────────────────────
    let mut schema_list: Signal<Option<SchemaListResponse>> = use_signal(|| None);
    let mut schema_err: Signal<Option<String>> = use_signal(|| None);

    // ── Form state ────────────────────────────────────────────────────
    // Initialise from prefill → then from localStorage draft → then default.
    let initial_draft = {
        if let Some(pf) = &props.prefill {
            DraftState::from_prefill(pf)
        } else {
            load_draft().unwrap_or_default()
        }
    };
    let mut draft: Signal<DraftState> = use_signal(|| initial_draft);

    // Collapsible optional fields visibility toggle.
    let mut show_optional: Signal<bool> = use_signal(|| false);

    // Submission state.
    let mut submitting: Signal<bool> = use_signal(|| false);
    let mut submit_err: Signal<Option<String>> = use_signal(|| None);

    // ── Load schemas on mount ─────────────────────────────────────────
    {
        let b = backend.clone();
        let ws = workspace.clone();
        use_effect(move || {
            let b = b.clone();
            let ws = ws.clone();
            spawn(async move {
                match b.list_schemas(&ws).await {
                    Ok(resp) => schema_list.set(Some(resp)),
                    Err(e) => schema_err.set(Some(e)),
                }
            });
        });
    }

    // ── Derived: selected type schema ─────────────────────────────────
    let current_type_id = draft.read().type_id.clone();
    let maybe_type_schema: Option<TypeSchema> = schema_list.read().as_ref().and_then(|sl| {
        sl.types.iter().find(|t| t.type_id == current_type_id).cloned()
    });

    // ── Draft-change helper ──────────────────────────────────────────
    let persist_draft = |d: &DraftState| {
        save_draft(d);
    };

    // ── Submit handler ────────────────────────────────────────────────
    let on_submit = {
        let b = backend.clone();
        let ws = workspace.clone();
        move |_: MouseEvent| {
            if submitting() { return; }

            let d = draft.read().clone();
            if d.title.trim().is_empty() {
                submit_err.set(Some("Title is required.".to_string()));
                return;
            }
            if d.type_id.is_empty() {
                submit_err.set(Some("Please select a ticket type.".to_string()));
                return;
            }

            submitting.set(true);
            submit_err.set(None);

            let b = b.clone();
            let ws_c = ws.clone();
            let nav_c = nav.clone();
            spawn(async move {
                let d = draft.read().clone();
                let mut fields: BTreeMap<String, serde_json::Value> = BTreeMap::new();
                if !d.priority.is_empty() {
                    fields.insert("priority".to_string(), serde_json::Value::String(d.priority.clone()));
                }
                if !d.component.is_empty() {
                    fields.insert("component".to_string(), serde_json::Value::String(d.component.clone()));
                }
                for (k, v) in &d.extra {
                    if !v.is_empty() {
                        fields.insert(k.clone(), serde_json::Value::String(v.clone()));
                    }
                }

                let body = CreateTicketRequest {
                    type_id: d.type_id.clone(),
                    title: if d.title.is_empty() { None } else { Some(d.title.clone()) },
                    description: if d.description.is_empty() { None } else { Some(d.description.clone()) },
                    fields: if fields.is_empty() { None } else { Some(fields) },
                };

                match b.create_ticket(&ws_c, &body).await {
                    Ok(resp) => {
                        clear_draft();
                        let new_id = resp.ticket.id.clone();
                        nav_c.push(Route::TicketDetailPage {
                            workspace: ws_c.clone(),
                            id: new_id,
                        });
                    }
                    Err(e) => {
                        submit_err.set(Some(e));
                        submitting.set(false);
                    }
                }
            });
        }
    };

    let on_cancel = {
        let handler = props.on_cancel.clone();
        move |_: MouseEvent| {
            clear_draft();
            handler.call(());
        }
    };

    // ── Render ────────────────────────────────────────────────────────

    rsx! {
        // Backdrop
        div {
            style: "
                position: fixed; inset: 0; z-index: 200;
                background: rgba(0,0,0,0.65);
                display: flex; align-items: center; justify-content: center;
                font-family: sans-serif;
            ",
            // Modal box
            div {
                style: "
                    background: #1a1a2e; border: 1px solid #3b3b5a;
                    border-radius: 10px; width: 560px; max-width: 96vw;
                    max-height: 90vh; display: flex; flex-direction: column;
                    color: #e0e0e8; box-shadow: 0 8px 40px rgba(0,0,0,0.6);
                ",

                // ── Header ──────────────────────────────────────────────
                div {
                    style: "
                        padding: 1.25rem 1.5rem 0.75rem;
                        border-bottom: 1px solid #2d2d4a;
                        display: flex; align-items: center; justify-content: space-between;
                    ",
                    h2 {
                        style: "margin: 0; font-size: 16px; font-weight: 700;",
                        "New Ticket"
                    }
                    button {
                        style: "
                            background: none; border: none; color: #8080a0;
                            cursor: pointer; font-size: 18px; line-height: 1;
                        ",
                        onclick: on_cancel.clone(),
                        "✕"
                    }
                }

                // ── Scrollable body ──────────────────────────────────────
                div {
                    style: "
                        padding: 1.25rem 1.5rem; overflow-y: auto; flex: 1;
                        display: flex; flex-direction: column; gap: 1rem;
                    ",

                    // Schema load error
                    if let Some(e) = schema_err.read().as_deref() {
                        div {
                            style: "
                                background: #3b1a1a; border: 1px solid #ef4444;
                                border-radius: 6px; padding: 0.5rem 0.75rem;
                                font-size: 13px; color: #fca5a5;
                            ",
                            "Failed to load schemas: {e}"
                        }
                    }

                    // Type selector
                    div {
                        label {
                            style: "
                                display: block; font-size: 11px; color: #6b7280;
                                text-transform: uppercase; letter-spacing: 0.05em;
                                margin-bottom: 4px;
                            ",
                            "Type "
                            span { style: "color: #ef4444;", "*" }
                        }
                        if let Some(sl) = schema_list.read().as_ref() {
                            select {
                                style: "
                                    width: 100%; background: #13131f; color: #e0e0e8;
                                    border: 1px solid #3b3b5a; border-radius: 6px;
                                    padding: 6px 8px; font-size: 14px;
                                ",
                                onchange: move |evt: Event<FormData>| {
                                    draft.with_mut(|d| {
                                        d.type_id = evt.value().to_string();
                                        d.extra.clear();
                                        persist_draft(d);
                                    });
                                },
                                option { value: "", disabled: true, selected: current_type_id.is_empty(), "— select type —" }
                                for ts in sl.types.iter() {
                                    option {
                                        key: "{ts.type_id}",
                                        value: "{ts.type_id}",
                                        selected: ts.type_id == current_type_id,
                                        "{ts.type_id}"
                                    }
                                }
                            }
                        } else {
                            div {
                                style: "font-size: 13px; color: #6b7280; padding: 6px 0;",
                                "Loading types…"
                            }
                        }
                    }

                    // ── Static fields ─────────────────────────────────────
                    // Title
                    {
                        let title_val = draft.read().title.clone();
                        rsx! {
                            div {
                                label {
                                    style: "
                                        display: block; font-size: 11px; color: #6b7280;
                                        text-transform: uppercase; letter-spacing: 0.05em;
                                        margin-bottom: 4px;
                                    ",
                                    "Title "
                                    span { style: "color: #ef4444;", "*" }
                                }
                                input {
                                    r#type: "text",
                                    value: "{title_val}",
                                    placeholder: "Short summary of the ticket",
                                    style: "
                                        width: 100%; background: #13131f; color: #e0e0e8;
                                        border: 1px solid #3b3b5a; border-radius: 6px;
                                        padding: 6px 8px; font-size: 14px;
                                        box-sizing: border-box;
                                    ",
                                    oninput: move |evt| {
                                        draft.with_mut(|d| {
                                            d.title = evt.value().to_string();
                                            persist_draft(d);
                                        });
                                    },
                                }
                            }
                        }
                    }

                    // Description (markdown textarea)
                    {
                        let desc_val = draft.read().description.clone();
                        rsx! {
                            div {
                                label {
                                    style: "
                                        display: block; font-size: 11px; color: #6b7280;
                                        text-transform: uppercase; letter-spacing: 0.05em;
                                        margin-bottom: 4px;
                                    ",
                                    "Description "
                                    span {
                                        style: "color: #4b5563; font-size: 10px; text-transform: none; letter-spacing: 0;",
                                        "(markdown)"
                                    }
                                }
                                textarea {
                                    placeholder: "Detailed description (markdown supported)…",
                                    rows: 5,
                                    style: "
                                        width: 100%; background: #13131f; color: #e0e0e8;
                                        border: 1px solid #3b3b5a; border-radius: 6px;
                                        padding: 6px 8px; font-size: 14px; font-family: inherit;
                                        resize: vertical; box-sizing: border-box;
                                    ",
                                    value: "{desc_val}",
                                    oninput: move |evt| {
                                        draft.with_mut(|d| {
                                            d.description = evt.value().to_string();
                                            persist_draft(d);
                                        });
                                    },
                                }
                            }
                        }
                    }

                    // Priority
                    {
                        let prio_val = draft.read().priority.clone();
                        rsx! {
                            div {
                                label {
                                    style: "
                                        display: block; font-size: 11px; color: #6b7280;
                                        text-transform: uppercase; letter-spacing: 0.05em;
                                        margin-bottom: 4px;
                                    ",
                                    "Priority"
                                }
                                select {
                                    style: "
                                        width: 100%; background: #13131f; color: #e0e0e8;
                                        border: 1px solid #3b3b5a; border-radius: 6px;
                                        padding: 6px 8px; font-size: 14px;
                                    ",
                                    onchange: move |evt: Event<FormData>| {
                                        draft.with_mut(|d| {
                                            d.priority = evt.value().to_string();
                                            persist_draft(d);
                                        });
                                    },
                                    for opt in PRIORITY_OPTIONS {
                                        option {
                                            key: "{opt}",
                                            value: "{opt}",
                                            selected: *opt == prio_val.as_str(),
                                            if opt.is_empty() { "— none —" } else { "{opt}" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Component
                    {
                        let comp_val = draft.read().component.clone();
                        rsx! {
                            div {
                                label {
                                    style: "
                                        display: block; font-size: 11px; color: #6b7280;
                                        text-transform: uppercase; letter-spacing: 0.05em;
                                        margin-bottom: 4px;
                                    ",
                                    "Component"
                                }
                                input {
                                    r#type: "text",
                                    value: "{comp_val}",
                                    placeholder: "e.g. ticket-viewer",
                                    style: "
                                        width: 100%; background: #13131f; color: #e0e0e8;
                                        border: 1px solid #3b3b5a; border-radius: 6px;
                                        padding: 6px 8px; font-size: 14px;
                                        box-sizing: border-box;
                                    ",
                                    oninput: move |evt| {
                                        draft.with_mut(|d| {
                                            d.component = evt.value().to_string();
                                            persist_draft(d);
                                        });
                                    },
                                }
                            }
                        }
                    }

                    // ── Schema-driven fields ──────────────────────────────
                    if let Some(ts) = &maybe_type_schema {
                        {
                            let required_fields: Vec<(String, crate::backend::FieldDef)> = ts.fields.iter()
                                .filter(|(k, fd)| {
                                    fd.required
                                        && !matches!(k.as_str(), "title" | "description" | "priority" | "component" | "type" | "state")
                                })
                                .map(|(k, fd)| (k.clone(), fd.clone()))
                                .collect();

                            let optional_fields: Vec<(String, crate::backend::FieldDef)> = ts.fields.iter()
                                .filter(|(k, fd)| {
                                    !fd.required
                                        && !matches!(k.as_str(), "title" | "description" | "priority" | "component" | "type" | "state")
                                })
                                .map(|(k, fd)| (k.clone(), fd.clone()))
                                .collect();

                            rsx! {
                                // Required schema fields
                                for (field_key, _field_def) in required_fields.iter() {
                                    {
                                        let fk = field_key.clone();
                                        let fk2 = field_key.clone();
                                        let val = draft.read().extra.get(field_key).cloned().unwrap_or_default();
                                        rsx! {
                                            div {
                                                key: "req-{fk}",
                                                label {
                                                    style: "
                                                        display: block; font-size: 11px; color: #6b7280;
                                                        text-transform: uppercase; letter-spacing: 0.05em;
                                                        margin-bottom: 4px;
                                                    ",
                                                    "{fk} "
                                                    span { style: "color: #ef4444;", "*" }
                                                }
                                                input {
                                                    r#type: "text",
                                                    value: "{val}",
                                                    style: "
                                                        width: 100%; background: #13131f; color: #e0e0e8;
                                                        border: 1px solid #ef4444; border-radius: 6px;
                                                        padding: 6px 8px; font-size: 14px;
                                                        box-sizing: border-box;
                                                    ",
                                                    oninput: move |evt| {
                                                        let key = fk2.clone();
                                                        draft.with_mut(|d| {
                                                            d.extra.insert(key, evt.value().to_string());
                                                            persist_draft(d);
                                                        });
                                                    },
                                                }
                                            }
                                        }
                                    }
                                }

                                // Optional schema fields (collapsible)
                                if !optional_fields.is_empty() {
                                    div {
                                        button {
                                            r#type: "button",
                                            style: "
                                                background: none; border: none; color: #6b7280;
                                                cursor: pointer; font-size: 12px; padding: 0;
                                                display: flex; align-items: center; gap: 4px;
                                            ",
                                            onclick: move |_| show_optional.set(!show_optional()),
                                            if show_optional() { "▾" } else { "▸" }
                                            " Optional fields ({optional_fields.len()})"
                                        }

                                        if show_optional() {
                                            div {
                                                style: "
                                                    margin-top: 0.75rem;
                                                    display: flex; flex-direction: column; gap: 0.75rem;
                                                ",
                                                for (field_key, _field_def) in optional_fields.iter() {
                                                    {
                                                        let fk = field_key.clone();
                                                        let fk2 = field_key.clone();
                                                        let val = draft.read().extra.get(field_key).cloned().unwrap_or_default();
                                                        rsx! {
                                                            div {
                                                                key: "opt-{fk}",
                                                                label {
                                                                    style: "
                                                                        display: block; font-size: 11px; color: #6b7280;
                                                                        text-transform: uppercase; letter-spacing: 0.05em;
                                                                        margin-bottom: 4px;
                                                                    ",
                                                                    "{fk}"
                                                                }
                                                                input {
                                                                    r#type: "text",
                                                                    value: "{val}",
                                                                    style: "
                                                                        width: 100%; background: #13131f; color: #e0e0e8;
                                                                        border: 1px solid #3b3b5a; border-radius: 6px;
                                                                        padding: 6px 8px; font-size: 14px;
                                                                        box-sizing: border-box;
                                                                    ",
                                                                    oninput: move |evt| {
                                                                        let key = fk2.clone();
                                                                        draft.with_mut(|d| {
                                                                            d.extra.insert(key, evt.value().to_string());
                                                                            persist_draft(d);
                                                                        });
                                                                    },
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Submit error banner
                    if let Some(e) = submit_err.read().as_deref() {
                        div {
                            style: "
                                background: #3b1a1a; border: 1px solid #ef4444;
                                border-radius: 6px; padding: 0.5rem 0.75rem;
                                font-size: 13px; color: #fca5a5;
                            ",
                            "{e}"
                        }
                    }
                } // end scrollable body

                // ── Footer ───────────────────────────────────────────────
                div {
                    style: "
                        padding: 1rem 1.5rem;
                        border-top: 1px solid #2d2d4a;
                        display: flex; gap: 0.75rem; justify-content: flex-end;
                    ",
                    button {
                        r#type: "button",
                        style: "
                            padding: 8px 18px; border-radius: 6px; border: none;
                            background: #2a2a42; color: #a0a0c0;
                            cursor: pointer; font-size: 14px;
                        ",
                        disabled: submitting(),
                        onclick: on_cancel.clone(),
                        "Cancel"
                    }
                    {
                        let submit_opacity = if submitting() { "0.6" } else { "1" };
                        rsx! {
                            button {
                                r#type: "button",
                                style: "
                                    padding: 8px 18px; border-radius: 6px; border: none;
                                    background: #3b82f6; color: white;
                                    cursor: pointer; font-size: 14px; font-weight: 600;
                                    opacity: {submit_opacity};
                                ",
                                disabled: submitting(),
                                onclick: on_submit.clone(),
                                if submitting() { "Creating…" } else { "Create Ticket" }
                            }
                        }
                    }
                }
            }
        }
    }
}

//! TicketContent — tabbed panel showing description, TOML, and an optional
//! Markdown editor for the currently selected ticket.
//!
//! Tabs:
//!   - "Description" (default) — fetches description.md via the HTTP backend
//!     and renders it as Markdown HTML using `FileContentViewer`.
//!   - "TOML"  — generates a TOML-like representation from the ticket's fields
//!     and displays it in a monospace `<pre>` block.
//!   - "Edit" (auth-gated) — split-pane Markdown editor: raw textarea on the
//!     left, live pulldown-cmark preview on the right.  Saves on blur or via
//!     the explicit Save button.  Surfaces success/error inline.

use dioxus::prelude::*;
use viewer_api_dioxus::FileContentViewer;

use crate::api::{HttpTicketBackend, TicketBackend};
use crate::components::history::HistoryPanel;
use crate::types::HistoryEntry;

// ── Tab identifier ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Description,
    Toml,
    Edit,
    History,
}

// ── TOML helper ───────────────────────────────────────────────────────────────

/// Convert the ticket fields JSON value to a TOML-like textual representation.
///
/// Mirrors the TypeScript `fieldsToToml` function in `TicketContent.tsx`.
fn fields_to_toml(id: &str, fields: &serde_json::Value) -> String {
    let mut lines: Vec<String> = vec![format!("# {id}"), String::new()];

    if let Some(map) = fields.as_object() {
        for (key, value) in map {
            let formatted = match value {
                serde_json::Value::String(s) => {
                    // Escape inner double-quotes.
                    let escaped = s.replace('"', "\\\"");
                    format!("{key} = \"{escaped}\"")
                }
                serde_json::Value::Null => format!("{key} = \"\""),
                serde_json::Value::Bool(b) => format!("{key} = {b}"),
                serde_json::Value::Number(n) => format!("{key} = {n}"),
                other => {
                    // Arrays / objects: fall back to JSON representation.
                    format!("{key} = {other}")
                }
            };
            lines.push(formatted);
        }
    }

    lines.join("\n")
}

// ── Shared tab-button style constants ─────────────────────────────────────────

const TAB_BASE_STYLE: &str = "
    padding: 8px 16px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: #9ca3af;
    font-size: 12px;
    font-family: sans-serif;
    cursor: pointer;
    transition: color 0.1s, border-color 0.1s;
";

const TAB_ACTIVE_STYLE: &str = "
    padding: 8px 16px;
    background: transparent;
    border: none;
    border-bottom: 2px solid #6366f1;
    color: #a5b4fc;
    font-size: 12px;
    font-family: sans-serif;
    cursor: pointer;
";

// ── Component ─────────────────────────────────────────────────────────────────

/// Tabbed content panel for a single ticket.
///
/// Props:
/// - `workspace`  — workspace the ticket belongs to.
/// - `ticket_id`  — ticket identifier used to fetch the description.
/// - `fields`     — ticket fields value (from the list/detail API response)
///                  used to generate the TOML tab text.
#[component]
pub fn TicketContent(
    workspace: String,
    ticket_id: String,
    fields: serde_json::Value,
) -> Element {
    // ── Tab selection ──────────────────────────────────────────────────
    let mut active_tab: Signal<Tab> = use_signal(|| Tab::Description);

    // ── Description async state ────────────────────────────────────────
    let mut desc_loading: Signal<bool> = use_signal(|| true);
    let mut desc_text: Signal<Option<String>> = use_signal(|| None);
    let mut desc_error: Signal<Option<String>> = use_signal(|| None);
    // ── History async state ────────────────────────────────────────────
    let mut history_entries: Signal<Vec<HistoryEntry>> = use_signal(|| vec![]);
    // Increment to trigger a re-fetch (e.g. after a revert).
    let mut history_refresh_key: Signal<u32> = use_signal(|| 0);
    // ── Edit-tab state ─────────────────────────────────────────────────
    // Draft markdown in the textarea, initialised from the fetched content.
    let mut edit_draft: Signal<String> = use_signal(String::new);
    // True while a PATCH request is in-flight.
    let mut edit_pending: Signal<bool> = use_signal(|| false);
    // True after a successful save (cleared on next input).
    let mut edit_success: Signal<bool> = use_signal(|| false);
    // Last save error, if any.
    let mut edit_error: Signal<Option<String>> = use_signal(|| None);

    // ── Fetch description on mount ─────────────────────────────────────
    // Note: description.md content is sourced from internal ticket files
    // managed by the ticket-api; it is rendered as HTML via FileContentViewer
    // (pulldown-cmark) which is safe for this trusted source.
    {
        let ws = workspace.clone();
        let tid = ticket_id.clone();
        use_effect(move || {
            let ws = ws.clone();
            let tid = tid.clone();
            desc_loading.set(true);
            desc_text.set(None);
            desc_error.set(None);
            // Reset edit state when ticket changes.
            edit_draft.set(String::new());
            edit_success.set(false);
            edit_error.set(None);
            spawn(async move {
                let backend = HttpTicketBackend::new(None);
                match backend.get_ticket_description(&ws, &tid).await {
                    Ok(resp) => {
                        let content = resp.description.unwrap_or_default();
                        edit_draft.set(content.clone());
                        desc_text.set(if content.is_empty() { None } else { Some(content) });
                        desc_loading.set(false);
                    }
                    Err(e) => {
                        desc_loading.set(false);
                        desc_error.set(Some(e));
                    }
                }
            });
        });
    }

    // ── Whether the Edit tab should be visible ────────────────────────
    let show_edit_tab = HttpTicketBackend::has_auth_token();

    // ── Fetch history (and re-fetch on revert) ────────────────────────
    {
        let ws = workspace.clone();
        let tid = ticket_id.clone();
        use_effect(move || {
            let _key = history_refresh_key(); // subscribe so revert triggers re-fetch
            let ws = ws.clone();
            let tid = tid.clone();
            history_entries.set(vec![]);
            spawn(async move {
                let backend = HttpTicketBackend::new(None);
                if let Ok(resp) = backend.get_ticket_history(&ws, &tid).await {
                    history_entries.set(resp.entries);
                }
            });
        });
    }

    // ── TOML text (computed from props, no async needed) ──────────────
    let toml_text = fields_to_toml(&ticket_id, &fields);

    // ── Capture workspace / ticket_id for save closures ───────────────
    let save_ws = workspace.clone();
    let save_tid = ticket_id.clone();

    // ── Pre-computed values used in the RSX ───────────────────────────
    let edit_btn_opacity = if edit_pending() { "0.6" } else { "1" };

    // ── Render ────────────────────────────────────────────────────────
    rsx! {
        div {
            "data-testid": "ticket-content",
            style: "
                display: flex;
                flex-direction: column;
                height: 100%;
                font-family: sans-serif;
                color: #e0e0e8;
                overflow: hidden;
            ",

            // ── Tab bar ───────────────────────────────────────────────
            div {
                role: "tablist",
                "data-testid": "ticket-content-tabs",
                style: "
                    display: flex;
                    border-bottom: 1px solid #3a3a4a;
                    flex-shrink: 0;
                ",

                // Description tab
                button {
                    role: "tab",
                    "aria-selected": if active_tab() == Tab::Description { "true" } else { "false" },
                    "data-testid": "tab-description",
                    style: if active_tab() == Tab::Description { TAB_ACTIVE_STYLE } else { TAB_BASE_STYLE },
                    onclick: move |_| active_tab.set(Tab::Description),
                    "Description"
                }

                // TOML tab
                button {
                    role: "tab",
                    "aria-selected": if active_tab() == Tab::Toml { "true" } else { "false" },
                    "data-testid": "tab-toml",
                    style: if active_tab() == Tab::Toml { TAB_ACTIVE_STYLE } else { TAB_BASE_STYLE },
                    onclick: move |_| active_tab.set(Tab::Toml),
                    "TOML"
                }

                // Edit tab — only shown when an auth token is available.
                if show_edit_tab {
                    button {
                        role: "tab",
                        "aria-selected": if active_tab() == Tab::Edit { "true" } else { "false" },
                        "data-testid": "tab-edit",
                        style: if active_tab() == Tab::Edit { TAB_ACTIVE_STYLE } else { TAB_BASE_STYLE },
                        onclick: move |_| active_tab.set(Tab::Edit),
                        "Edit"
                    }
                }

                // History tab — only visible when the ticket has more than one
                // revision (i.e. something has changed since creation).
                if history_entries().len() > 1 {
                    button {
                        role: "tab",
                        "aria-selected": if active_tab() == Tab::History { "true" } else { "false" },
                        "data-testid": "tab-history",
                        style: if active_tab() == Tab::History { TAB_ACTIVE_STYLE } else { TAB_BASE_STYLE },
                        onclick: move |_| active_tab.set(Tab::History),
                        "History"
                    }
                }
            }

            // ── Tab panels ────────────────────────────────────────────
            div {
                "data-testid": "ticket-content-body",
                style: if active_tab() == Tab::History {
                    // History panel manages its own scroll and layout.
                    "flex: 1; overflow: hidden;"
                } else {
                    "flex: 1; overflow-y: auto; padding: 20px 24px;"
                },

                // Description panel
                if active_tab() == Tab::Description {
                    if desc_loading() {
                        div {
                            "data-testid": "desc-loading",
                            style: "color: #6b7280; font-size: 13px;",
                            "Loading…"
                        }
                    } else if let Some(err) = desc_error() {
                        div {
                            "data-testid": "desc-error",
                            style: "color: #f87171; font-size: 13px;",
                            strong { "Error: " }
                            "{err}"
                        }
                    } else if let Some(content) = desc_text() {
                        // FileContentViewer detects `.md` extension and renders
                        // the content as Markdown HTML via pulldown-cmark.
                        // Content comes from trusted internal ticket files
                        // managed by ticket-api; pulldown-cmark output is safe.
                        div {
                            "data-testid": "desc-markdown",
                            FileContentViewer {
                                content,
                                filename: "description.md".to_string(),
                            }
                        }
                    } else {
                        // Ticket exists but has no description.md.
                        em {
                            "data-testid": "desc-empty",
                            style: "color: #6b7280; font-size: 13px;",
                            "No description.md found for this ticket."
                        }
                    }
                }

                // TOML panel
                if active_tab() == Tab::Toml {
                    pre {
                        "data-testid": "toml-content",
                        style: "
                            font-family: 'Cascadia Code', 'Fira Code', monospace;
                            font-size: 12.5px;
                            line-height: 1.6;
                            color: #e0e0e8;
                            margin: 0;
                            white-space: pre;
                            overflow-x: auto;
                        ",
                        "{toml_text}"
                    }
                }

                // History panel
                if active_tab() == Tab::History {
                    HistoryPanel {
                        workspace: workspace.clone(),
                        ticket_id: ticket_id.clone(),
                        entries: history_entries(),
                        on_refresh: move |_| {
                            history_refresh_key.set(history_refresh_key() + 1);
                        },
                    }
                }

                // Edit panel — split layout: textarea (left) + live preview (right).
                if active_tab() == Tab::Edit {
                    div {
                        "data-testid": "edit-panel",
                        style: "
                            display: flex;
                            gap: 16px;
                            min-height: 480px;
                            height: 100%;
                        ",

                        // ── Left: Markdown textarea ───────────────────
                        div {
                            style: "
                                flex: 1;
                                display: flex;
                                flex-direction: column;
                                gap: 8px;
                                min-width: 0;
                            ",

                            textarea {
                                "data-testid": "edit-textarea",
                                "aria-label": "Markdown editor",
                                style: "
                                    flex: 1;
                                    min-height: 380px;
                                    width: 100%;
                                    box-sizing: border-box;
                                    background: #13131f;
                                    color: #e0e0e8;
                                    border: 1px solid #3a3a4a;
                                    border-radius: 4px;
                                    padding: 10px 12px;
                                    font-family: 'Cascadia Code', 'Fira Code', monospace;
                                    font-size: 12.5px;
                                    line-height: 1.6;
                                    resize: vertical;
                                    outline: none;
                                ",
                                disabled: edit_pending(),
                                value: "{edit_draft()}",
                                oninput: move |evt: Event<FormData>| {
                                    edit_draft.set(evt.value().to_string());
                                    edit_success.set(false);
                                    edit_error.set(None);
                                },
                                onblur: {
                                    let ws = save_ws.clone();
                                    let tid = save_tid.clone();
                                    move |_| {
                                        if edit_pending() { return; }
                                        let text = edit_draft();
                                        let ws = ws.clone();
                                        let tid = tid.clone();
                                        edit_pending.set(true);
                                        edit_error.set(None);
                                        edit_success.set(false);
                                        spawn(async move {
                                            let token = HttpTicketBackend::read_auth_token();
                                            let backend = HttpTicketBackend::new(token);
                                            match backend.update_ticket_description(&ws, &tid, &text).await {
                                                Ok(()) => {
                                                    desc_text.set(if text.is_empty() { None } else { Some(text) });
                                                    edit_pending.set(false);
                                                    edit_success.set(true);
                                                }
                                                Err(e) => {
                                                    // Rollback: restore draft to last saved value.
                                                    edit_draft.set(desc_text().unwrap_or_default());
                                                    edit_pending.set(false);
                                                    edit_error.set(Some(e));
                                                }
                                            }
                                        });
                                    }
                                },
                            }

                            // ── Footer: status + Save button ──────────
                            div {
                                style: "
                                    display: flex;
                                    align-items: center;
                                    gap: 10px;
                                    flex-shrink: 0;
                                ",

                                button {
                                    "data-testid": "edit-save-btn",
                                    style: "
                                        padding: 5px 16px;
                                        border-radius: 4px;
                                        border: none;
                                        background: #6366f1;
                                        color: white;
                                        cursor: pointer;
                                        font-size: 12px;
                                        font-weight: 600;
                                        opacity: {edit_btn_opacity};
                                    ",
                                    disabled: edit_pending(),
                                    onclick: {
                                        let ws = save_ws.clone();
                                        let tid = save_tid.clone();
                                        move |_| {
                                            if edit_pending() { return; }
                                            let text = edit_draft();
                                            let ws = ws.clone();
                                            let tid = tid.clone();
                                            edit_pending.set(true);
                                            edit_error.set(None);
                                            edit_success.set(false);
                                            spawn(async move {
                                                let token = HttpTicketBackend::read_auth_token();
                                                let backend = HttpTicketBackend::new(token);
                                                match backend.update_ticket_description(&ws, &tid, &text).await {
                                                    Ok(()) => {
                                                        desc_text.set(if text.is_empty() { None } else { Some(text) });
                                                        edit_pending.set(false);
                                                        edit_success.set(true);
                                                    }
                                                    Err(e) => {
                                                        edit_draft.set(desc_text().unwrap_or_default());
                                                        edit_pending.set(false);
                                                        edit_error.set(Some(e));
                                                    }
                                                }
                                            });
                                        }
                                    },
                                    if edit_pending() { "Saving…" } else { "Save" }
                                }

                                if edit_success() {
                                    span {
                                        "data-testid": "edit-success",
                                        style: "font-size: 12px; color: #4ade80;",
                                        "✓ Saved"
                                    }
                                }

                                if let Some(err) = edit_error() {
                                    span {
                                        "data-testid": "edit-error",
                                        style: "font-size: 12px; color: #f87171;",
                                        "Error: {err}"
                                    }
                                }
                            }
                        }

                        // ── Right: live Markdown preview ──────────────
                        div {
                            style: "
                                flex: 1;
                                overflow-y: auto;
                                min-width: 0;
                                border-left: 1px solid #3a3a4a;
                                padding-left: 16px;
                            ",
                            if edit_draft().is_empty() {
                                em {
                                    "data-testid": "edit-preview-empty",
                                    style: "color: #6b7280; font-size: 13px;",
                                    "Preview will appear here…"
                                }
                            } else {
                                div {
                                    "data-testid": "edit-preview",
                                    FileContentViewer {
                                        content: edit_draft(),
                                        filename: "description.md".to_string(),
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

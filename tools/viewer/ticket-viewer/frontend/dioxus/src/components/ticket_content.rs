//! TicketContent — two-tab panel showing description.md and ticket.toml for
//! the currently selected ticket.
//!
//! Tabs:
//!   - "Description" (default) — fetches description.md via the HTTP backend
//!     and renders it as Markdown HTML using `FileContentViewer`.
//!   - "TOML"  — generates a TOML-like representation from the ticket's fields
//!     and displays it in a monospace `<pre>` block.

use dioxus::prelude::*;
use viewer_api_dioxus::FileContentViewer;

use crate::api::{HttpTicketBackend, TicketBackend};

// ── Tab identifier ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Description,
    Toml,
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
            spawn(async move {
                let backend = HttpTicketBackend::new(None);
                match backend.get_ticket_description(&ws, &tid).await {
                    Ok(resp) => {
                        desc_loading.set(false);
                        desc_text.set(resp.description);
                    }
                    Err(e) => {
                        desc_loading.set(false);
                        desc_error.set(Some(e));
                    }
                }
            });
        });
    }

    // ── TOML text (computed from props, no async needed) ──────────────
    let toml_text = fields_to_toml(&ticket_id, &fields);

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
            }

            // ── Tab panels ────────────────────────────────────────────
            div {
                "data-testid": "ticket-content-body",
                style: "
                    flex: 1;
                    overflow-y: auto;
                    padding: 20px 24px;
                ",

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
            }
        }
    }
}

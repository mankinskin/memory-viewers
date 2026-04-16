//! TicketTree — sidebar widget that lists tickets with filter controls.
//!
//! Renders a filter input, state-chip bar, and a scrollable sorted list of
//! ticket buttons.  Clicking a ticket calls `on_select`.

use dioxus::prelude::*;

use crate::types::{state_colors, TicketSummary};

// ── Props ──────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct TicketTreeProps {
    /// Full (unfiltered) ticket list — filtering and sorting happen server-side
    /// or in the parent; this component just renders.
    pub tickets: Vec<TicketSummary>,
    pub loading: bool,
    pub error: Option<String>,

    /// Current filter text (two-way binding via signals in the parent).
    pub filter: String,
    pub on_filter_change: EventHandler<String>,

    /// Active state-chip value (empty string = "All").
    pub state_filter: String,
    pub on_state_filter_change: EventHandler<String>,

    /// Current sort key used to order the list.
    pub sort_key: String,

    /// ID of the selected ticket (highlighted row).
    pub selected_id: Option<String>,
    /// Called when a ticket row is clicked.
    pub on_select: EventHandler<String>,
}

// ── State filter chip definitions ──────────────────────────────────────────

const STATE_CHIPS: &[(&str, &str)] = &[
    ("All", ""),
    ("new", "new"),
    ("ready", "ready"),
    ("impl", "in-implementation"),
    ("review", "in-review"),
    ("done", "done"),
    ("cancelled", "cancelled"),
];

// ── Component ──────────────────────────────────────────────────────────────

#[component]
pub fn TicketTree(props: TicketTreeProps) -> Element {
    let filter = props.filter.clone();
    let state_filter_val = props.state_filter.clone();

    rsx! {
        // ── Filter controls ────────────────────────────────────────────
        div {
            style: "
                padding: 8px 12px;
                border-bottom: 1px solid var(--border-subtle);
                display: flex;
                flex-direction: column;
                gap: 6px;
            ",
            input {
                r#type: "text",
                placeholder: "Filter tickets…",
                style: "
                    width: 100%;
                    padding: 6px 10px;
                    border-radius: 6px;
                    border: 1px solid var(--border-subtle);
                    background: var(--bg-secondary);
                    color: var(--text-primary);
                    font-size: 12px;
                    box-sizing: border-box;
                    outline: none;
                ",
                value: "{filter}",
                oninput: move |e| props.on_filter_change.call(e.value()),
            }
            div {
                style: "display: flex; flex-wrap: wrap; gap: 4px;",
                for &(lab, val) in STATE_CHIPS.iter() {
                    {
                        let is_active = state_filter_val.as_str() == val;
                        let chip_bg = if is_active { "var(--accent-blue)" } else { "var(--bg-secondary)" };
                        let v = val.to_string();
                        rsx! {
                            button {
                                key: "{val}",
                                style: "
                                    padding: 2px 8px;
                                    border-radius: 10px;
                                    border: 1px solid var(--border-subtle);
                                    background: {chip_bg};
                                    color: var(--text-primary);
                                    font-size: 10px;
                                    cursor: pointer;
                                    font-weight: 600;
                                ",
                                onclick: move |_| props.on_state_filter_change.call(v.clone()),
                                "{lab}"
                            }
                        }
                    }
                }
            }
        }

        // ── Loading state ──────────────────────────────────────────────
        if props.loading {
            div {
                class: "sidebar-loading",
                "Loading tickets…"
            }
        }

        // ── Error state ────────────────────────────────────────────────
        if let Some(ref err) = props.error {
            div {
                style: "padding: 12px; color: var(--error); font-size: 12px;",
                "Failed to load: {err}"
            }
        }

        // ── Ticket list ────────────────────────────────────────────────
        if !props.loading {
            if props.tickets.is_empty() {
                div {
                    class: "sidebar-empty",
                    "No tickets in this workspace."
                }
            }
            {
                let mut sorted = props.tickets.clone();
                match props.sort_key.as_str() {
                    "title" => sorted.sort_by(|a, b| a.title.cmp(&b.title)),
                    "state" => sorted.sort_by(|a, b| a.state.cmp(&b.state)),
                    "created_at" => sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
                    _ => sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
                }
                rsx! {
                    for ticket in sorted {
                        {
                            let tid = ticket.id.clone();
                            let tid_click = tid.clone();
                            let title = ticket.title.clone().unwrap_or_else(|| "Untitled".into());
                            let state = ticket.state.clone().unwrap_or_else(|| "new".into());
                            let is_selected = props.selected_id.as_deref() == Some(tid.as_str());

                            let (state_bg_raw, state_fg) = state_colors(&state);
                            // Convert the opaque hex state_bg to a low-opacity variant for the chip.
                            let state_bg = format!("{}20", state_bg_raw);

                            let row_bg = if is_selected {
                                "var(--bg-active)"
                            } else {
                                "transparent"
                            };

                            rsx! {
                                button {
                                    key: "{tid}",
                                    style: "
                                        display: flex;
                                        flex-direction: column;
                                        gap: 4px;
                                        width: 100%;
                                        padding: 10px 14px;
                                        border: none;
                                        border-bottom: 1px solid var(--border-subtle);
                                        background: {row_bg};
                                        color: var(--text-primary);
                                        cursor: pointer;
                                        text-align: left;
                                        min-height: 44px;
                                    ",
                                    onclick: move |_| {
                                        props.on_select.call(tid_click.clone());
                                    },
                                    span {
                                        style: "font-size: 13px; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                        "{title}"
                                    }
                                    span {
                                        style: "
                                            display: inline-block;
                                            font-size: 10px;
                                            font-weight: 600;
                                            padding: 1px 7px;
                                            border-radius: 10px;
                                            background: {state_bg};
                                            color: {state_fg};
                                        ",
                                        "{state}"
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

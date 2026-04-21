//! `SpecTree` — collapsible hierarchical sidebar widget.
//!
//! Renders a filter input, state chip bar, and a scrollable list of
//! `SpecCard` buttons.  Clicking a card calls `on_select`.

use dioxus::prelude::*;
use crate::types::SpecSummary;
use super::search_bar::SearchBar;
use super::spec_card::SpecCard;
use super::state_badge::StateBadge;

// ── State filter chips ────────────────────────────────────────────────────────

const STATE_CHIPS: &[(&str, &str)] = &[
    ("All", ""),
    ("draft", "draft"),
    ("ready", "ready"),
    ("reviewed", "reviewed"),
    ("approved", "approved"),
    ("archived", "archived"),
];

// ── Props ─────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct SpecTreeProps {
    pub specs: Vec<SpecSummary>,
    pub loading: bool,
    pub error: Option<String>,

    pub filter: String,
    pub on_filter_change: EventHandler<String>,

    pub state_filter: String,
    pub on_state_filter_change: EventHandler<String>,

    pub selected_id: Option<String>,
    pub on_select: EventHandler<String>,
}

// ── Component ─────────────────────────────────────────────────────────────────

/// Collapsible hierarchical spec list with filter controls.
#[component]
pub fn SpecTree(props: SpecTreeProps) -> Element {
    rsx! {
        div {
            style: "
                display: flex;
                flex-direction: column;
                height: 100%;
                overflow: hidden;
                background: #111827;
                border-right: 1px solid #1f2937;
            ",

            // ── Filter row ────────────────────────────────────────────────
            div {
                style: "padding: 10px 12px; border-bottom: 1px solid #1f2937;",
                SearchBar {
                    value: props.filter.clone(),
                    on_change: move |v| props.on_filter_change.call(v),
                }
            }

            // ── State chips ───────────────────────────────────────────────
            div {
                style: "
                    display: flex;
                    flex-wrap: wrap;
                    gap: 4px;
                    padding: 6px 12px;
                    border-bottom: 1px solid #1f2937;
                ",
                for (label, value) in STATE_CHIPS.iter() {
                    {
                        let value_str = value.to_string();
                        let label_str = label.to_string();
                        let is_active = props.state_filter == *value;
                        let bg = if is_active { "#1e40af" } else { "#1f2937" };
                        let fg = if is_active { "#bfdbfe" } else { "#9ca3af" };
                        rsx! {
                            button {
                                key: "{value_str}",
                                style: "
                                    padding: 3px 8px;
                                    border: none;
                                    border-radius: 10px;
                                    background: {bg};
                                    color: {fg};
                                    font-size: 11px;
                                    cursor: pointer;
                                ",
                                onclick: move |_| props.on_state_filter_change.call(value_str.clone()),
                                "{label_str}"
                            }
                        }
                    }
                }
            }

            // ── Spec list ─────────────────────────────────────────────────
            div {
                style: "flex: 1; overflow-y: auto;",
                if props.loading {
                    p {
                        style: "padding: 16px; color: #6b7280; font-size: 13px;",
                        "Loading…"
                    }
                } else if let Some(err) = &props.error {
                    p {
                        style: "padding: 16px; color: #f87171; font-size: 13px;",
                        "{err}"
                    }
                } else if props.specs.is_empty() {
                    p {
                        style: "padding: 16px; color: #6b7280; font-size: 13px;",
                        "No specs found."
                    }
                } else {
                    for spec in props.specs.iter() {
                        {
                            let selected = props.selected_id.as_deref() == Some(&spec.id);
                            rsx! {
                                SpecCard {
                                    key: "{spec.id}",
                                    spec: spec.clone(),
                                    selected,
                                    on_click: move |id| props.on_select.call(id),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

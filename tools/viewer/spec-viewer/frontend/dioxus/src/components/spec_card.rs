//! `SpecCard` — compact row in the spec list / tree sidebar.

use dioxus::prelude::*;
use crate::types::{SpecSummary, state_accent};
use super::state_badge::StateBadge;

#[derive(Props, Clone, PartialEq)]
pub struct SpecCardProps {
    pub spec: SpecSummary,
    pub selected: bool,
    pub on_click: EventHandler<String>,
    /// Indentation level for tree display (0 = root).
    #[props(default = 0)]
    pub indent: usize,
}

#[component]
pub fn SpecCard(props: SpecCardProps) -> Element {
    let accent = state_accent(props.spec.state.as_deref());
    let bg = if props.selected { "#1e293b" } else { "transparent" };
    let title = props.spec.title.as_deref().unwrap_or("Untitled");
    let slug = props.spec.slug.as_deref().unwrap_or("-");
    let state = props.spec.state.clone().unwrap_or_default();
    let id = props.spec.id.clone();
    let indent_px = props.indent * 16;

    rsx! {
        button {
            style: "
                display: flex;
                flex-direction: column;
                align-items: flex-start;
                gap: 3px;
                width: 100%;
                text-align: left;
                padding: 8px 12px 8px {indent_px + 12}px;
                border: none;
                border-left: 3px solid {accent};
                background: {bg};
                color: #e5e7eb;
                cursor: pointer;
                transition: background 0.1s;
            ",
            onclick: move |_| props.on_click.call(id.clone()),
            span {
                style: "font-size: 13px; font-weight: 500; color: #f9fafb; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%;",
                "{title}"
            }
            div {
                style: "display: flex; align-items: center; gap: 6px;",
                span {
                    style: "font-size: 11px; color: #6b7280; font-family: monospace;",
                    "{slug}"
                }
                if !state.is_empty() {
                    StateBadge { state }
                }
            }
        }
    }
}

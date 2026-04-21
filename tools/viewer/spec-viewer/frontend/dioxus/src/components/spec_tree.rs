//! `SpecTree` — sidebar widget that renders spec items using viewer-api
//! [`FileTree`] and [`FilterDef`] components.
//!
//! Specs are displayed as flat leaf nodes, each with a [`NodeIcon::Doc`] icon.
//! State filter chips are provided as [`FilterDef`] buttons, and a search
//! input sits above the tree.

use dioxus::prelude::*;
use viewer_api_dioxus::{FileTree, FilterDef, NodeIcon, SortKey, TreeNode};

use crate::types::SpecSummary;

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

// ── Helpers ───────────────────────────────────────────────────────────────────

const ALL_STATES: &[&str] = &["draft", "ready", "reviewed", "approved", "archived"];

fn count_state(specs: &[SpecSummary], state: &str) -> usize {
    specs
        .iter()
        .filter(|s| s.state.as_deref() == Some(state))
        .count()
}

// ── Component ─────────────────────────────────────────────────────────────────

/// Sidebar spec list using viewer-api FileTree with state filter chips.
#[component]
pub fn SpecTree(props: SpecTreeProps) -> Element {
    // Build FilterDef list from available states.
    let filter_defs: Vec<FilterDef> = ALL_STATES
        .iter()
        .filter_map(|&state| {
            let count = count_state(&props.specs, state);
            if count == 0 {
                return None;
            }
            let color = match state {
                "draft"    => Some("var(--text-muted)".to_string()),
                "ready"    => Some("var(--accent-blue)".to_string()),
                "reviewed" => Some("var(--accent-green)".to_string()),
                "approved" => Some("var(--accent-green)".to_string()),
                "archived" => Some("var(--text-muted)".to_string()),
                _          => None,
            };
            Some(FilterDef {
                key: state.to_string(),
                label: state.to_string(),
                count,
                color,
            })
        })
        .collect();

    let active_filters: Vec<String> = if props.state_filter.is_empty() {
        vec![]
    } else {
        vec![props.state_filter.clone()]
    };

    // Convert matching specs to TreeNode items.
    let nodes: Vec<TreeNode> = props
        .specs
        .iter()
        .filter(|s| {
            props.state_filter.is_empty()
                || s.state.as_deref() == Some(props.state_filter.as_str())
        })
        .map(|s| {
            let label = s
                .title
                .as_deref()
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| s.slug.as_deref().unwrap_or("Untitled"))
                .to_string();
            let state = s.state.clone().unwrap_or_default();
            let badge_color = match state.as_str() {
                "draft"    => Some("var(--text-muted)".to_string()),
                "ready"    => Some("var(--accent-blue)".to_string()),
                "reviewed" => Some("var(--accent-green)".to_string()),
                "approved" => Some("var(--accent-green)".to_string()),
                "archived" => Some("var(--text-muted)".to_string()),
                _          => None,
            };
            TreeNode {
                id: s.id.clone(),
                label,
                badge: if state.is_empty() { None } else { Some(state) },
                badge_color,
                tooltip: s.slug.clone(),
                is_dir: false,
                icon: NodeIcon::Doc,
                children: vec![],
            }
        })
        .collect();

    let sort_keys = vec![SortKey::new("title", "Title")];

    let on_select = props.on_select.clone();
    let on_state_filter = props.on_state_filter_change.clone();
    let on_filter = props.on_filter_change.clone();
    let filter_val = props.filter.clone();
    let active_filters_closure = active_filters.clone();

    rsx! {
        div {
            style: "display: flex; flex-direction: column; height: 100%; overflow: hidden;",

            // ── Search input ──────────────────────────────────────────────
            div {
                class: "sidebar-search",
                input {
                    r#type: "text",
                    value: "{filter_val}",
                    placeholder: "Search specs…",
                    oninput: move |e| on_filter.call(e.value()),
                }
            }

            // ── FileTree with state filters ───────────────────────────────
            FileTree {
                nodes,
                sort_keys,
                filters: filter_defs,
                active_filters,
                loading: props.loading,
                selected_id: props.selected_id.clone(),
                on_select: move |id: String| on_select.call(id),
                on_filter: move |key: String| {
                    // Toggle: if already active clear it, else set it.
                    let is_active = active_filters_closure.contains(&key);
                    on_state_filter.call(if is_active { String::new() } else { key });
                },
            }
        }
    }
}


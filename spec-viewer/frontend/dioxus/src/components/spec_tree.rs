//! `SpecTree` — sidebar widget that renders spec items using viewer-api
//! [`FileTree`] and [`FilterDef`] components.
//!
//! Specs are grouped by their `component` field into folder nodes. Specs
//! without a component appear at the root level. Each spec leaf uses a
//! [`NodeIcon::Doc`] icon. State filter chips are provided as [`FilterDef`]
//! buttons, and a search input sits above the tree.

use std::collections::BTreeMap;

use dioxus::prelude::*;
use viewer_api_dioxus::{
    FileTree,
    FilterDef,
    NodeIcon,
    SortKey,
    TreeNode,
};

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

    /// Folder ids that should start expanded.  Driven from URL state in the
    /// host component so deep-linked specs auto-reveal their ancestor folder.
    #[props(default)]
    pub initially_expanded: Vec<String>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const ALL_STATES: &[&str] =
    &["draft", "ready", "reviewed", "approved", "archived"];

fn count_state(
    specs: &[SpecSummary],
    state: &str,
) -> usize {
    specs
        .iter()
        .filter(|s| s.state.as_deref() == Some(state))
        .count()
}

fn badge_color(state: &str) -> Option<String> {
    match state {
        "draft" => Some("var(--text-muted)".to_string()),
        "ready" => Some("var(--accent-blue)".to_string()),
        "reviewed" => Some("var(--accent-green)".to_string()),
        "approved" => Some("var(--accent-green)".to_string()),
        "archived" => Some("var(--text-muted)".to_string()),
        _ => None,
    }
}

/// Build a `TreeNode` leaf for a single spec.
fn spec_leaf(s: &SpecSummary) -> TreeNode {
    let label = s
        .title
        .as_deref()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| s.slug.as_deref().unwrap_or("Untitled"))
        .to_string();
    let state = s.state.clone().unwrap_or_default();
    TreeNode {
        id: s.id.clone(),
        label,
        badge: if state.is_empty() {
            None
        } else {
            Some(state.clone())
        },
        badge_color: badge_color(&state),
        tooltip: s.slug.clone(),
        tooltip_render: None,
        is_dir: false,
        icon: NodeIcon::Doc,
        children: vec![],
    }
}

/// Build nested `TreeNode`s from a flat spec list, grouping by `component`.
///
/// Specs with a non-empty `component` value are placed inside a folder node
/// named after the component. Specs without a component appear at the root.
fn build_nodes(specs: &[SpecSummary]) -> Vec<TreeNode> {
    let mut folders: BTreeMap<String, Vec<TreeNode>> = BTreeMap::new();
    let mut root_leaves: Vec<TreeNode> = Vec::new();

    for s in specs {
        let leaf = spec_leaf(s);
        if let Some(comp) = s.component.as_deref().filter(|c| !c.is_empty()) {
            folders.entry(comp.to_string()).or_default().push(leaf);
        } else {
            root_leaves.push(leaf);
        }
    }

    // Build folder nodes sorted by component name.
    let mut nodes: Vec<TreeNode> = folders
        .into_iter()
        .map(|(comp, children)| {
            TreeNode::dir(format!("__folder__{comp}"), comp, children)
        })
        .collect();

    // Append root-level specs after folders.
    nodes.extend(root_leaves);
    nodes
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
            Some(FilterDef {
                key: state.to_string(),
                label: state.to_string(),
                count,
                color: badge_color(state),
            })
        })
        .collect();

    let active_filters: Vec<String> = if props.state_filter.is_empty() {
        vec![]
    } else {
        vec![props.state_filter.clone()]
    };

    // Filter specs by active state, then build nested nodes.
    let filtered: Vec<&SpecSummary> = props
        .specs
        .iter()
        .filter(|s| {
            props.state_filter.is_empty()
                || s.state.as_deref() == Some(props.state_filter.as_str())
        })
        .collect();

    let nodes = build_nodes(&filtered.into_iter().cloned().collect::<Vec<_>>());

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

            if let Some(err) = props.error.as_deref() {
                div {
                    style: "padding: 12px 14px; color: var(--level-error-text); font-size: 13px;",
                    "Failed to load specifications: {err}"
                }
            } else if !props.loading && nodes.is_empty() {
                div {
                    style: "padding: 12px 14px; color: var(--text-muted); font-size: 13px; line-height: 1.45;",
                    if props.filter.trim().is_empty() && props.state_filter.is_empty() {
                        "No specifications are available yet."
                    } else {
                        "No specifications match the current search or state filter."
                    }
                }
            }

            // ── FileTree with state filters ───────────────────────────────
            if props.loading || !nodes.is_empty() {
                FileTree {
                    nodes,
                    sort_keys,
                    filters: filter_defs,
                    active_filters,
                    loading: props.loading,
                    selected_id: props.selected_id.clone(),
                    initially_expanded: props.initially_expanded.clone(),
                    on_select: move |id: String| {
                        // Ignore clicks on folder pseudo-nodes.
                        if !id.starts_with("__folder__") {
                            on_select.call(id);
                        }
                    },
                    on_filter: move |key: String| {
                        // Toggle: if already active clear it, else set it.
                        let is_active = active_filters_closure.contains(&key);
                        on_state_filter.call(if is_active { String::new() } else { key });
                    },
                }
            }
        }
    }
}

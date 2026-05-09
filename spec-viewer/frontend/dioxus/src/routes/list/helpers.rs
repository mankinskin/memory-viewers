use std::collections::HashSet;

use dioxus::prelude::*;
use viewer_api_dioxus::{
    expand_path_to,
    is_mobile_sidebar_viewport,
};

use crate::types::SpecSummary;

pub(super) fn sidebar_button_state(
    sidebar_collapsed: Signal<bool>,
    mobile_sidebar_open: Signal<bool>,
) -> (bool, &'static str) {
    if is_mobile_sidebar_viewport() {
        let open = *mobile_sidebar_open.read();
        return (
            open,
            if open {
                "Close specifications sidebar"
            } else {
                "Open specifications sidebar"
            },
        );
    }

    let collapsed = *sidebar_collapsed.read();
    (
        !collapsed,
        if collapsed {
            "Open specifications sidebar"
        } else {
            "Collapse specifications sidebar"
        },
    )
}

pub(super) fn toggle_sidebar(
    mut sidebar_collapsed: Signal<bool>,
    mut mobile_sidebar_open: Signal<bool>,
) {
    if is_mobile_sidebar_viewport() {
        let next = !*mobile_sidebar_open.read();
        mobile_sidebar_open.set(next);
    } else {
        sidebar_collapsed.toggle();
    }
}

pub(super) fn close_or_toggle_sidebar(
    mut sidebar_collapsed: Signal<bool>,
    mut mobile_sidebar_open: Signal<bool>,
) {
    if is_mobile_sidebar_viewport() {
        mobile_sidebar_open.set(false);
    } else {
        let next = !*sidebar_collapsed.read();
        sidebar_collapsed.set(next);
    }
}

pub(super) fn compute_prefetch_targets(
    specs: &[SpecSummary],
    active: &str,
) -> Vec<String> {
    let active_pos = specs.iter().position(|spec| spec.id == active);
    let active_component = active_pos
        .and_then(|index| specs.get(index))
        .and_then(|spec| spec.component.clone())
        .filter(|component| !component.is_empty());

    let mut targets = Vec::new();
    let mut push = |id: String| {
        if id != active && !targets.contains(&id) {
            targets.push(id);
        }
    };

    if let Some(component) = active_component.as_deref() {
        for spec in specs {
            if spec.component.as_deref() == Some(component) {
                push(spec.id.clone());
            }
        }
    }

    if let Some(index) = active_pos {
        if index > 0 {
            push(specs[index - 1].id.clone());
        }
        if index + 1 < specs.len() {
            push(specs[index + 1].id.clone());
        }
    }

    targets
}

pub(super) fn label_for(
    specs: &[SpecSummary],
    id: &str,
) -> String {
    specs
        .iter()
        .find(|spec| spec.id == id)
        .and_then(|spec| spec.title.clone())
        .unwrap_or_else(|| id.to_string())
}

pub(super) fn initial_expanded_for(
    specs: &[SpecSummary],
    active_id: Option<&str>,
) -> Vec<String> {
    let Some(id) = active_id else {
        return Vec::new();
    };
    let Some(component) = specs
        .iter()
        .find(|spec| spec.id == id)
        .and_then(|spec| spec.component.clone())
        .filter(|component| !component.is_empty())
    else {
        return Vec::new();
    };

    let folder_id = format!("__folder__{component}");
    let segments: Vec<&str> = folder_id.split(':').collect();
    let mut expanded: HashSet<String> = HashSet::new();
    expand_path_to(&mut expanded, &segments);
    expanded.into_iter().collect()
}

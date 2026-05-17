use std::collections::{
    HashMap,
    HashSet,
};

use dioxus::prelude::*;

use crate::types::TicketFileEntry;

use super::{
    header::render_filter_controls,
    rows::render_ticket_rows,
    TicketTreeProps,
};

#[component]
pub fn TicketTree(props: TicketTreeProps) -> Element {
    let filter = props.filter.clone();
    let state_filter_val = props.state_filter.clone();
    let expanded_ids: Signal<HashSet<String>> = use_signal(HashSet::new);
    let file_cache: Signal<HashMap<String, Vec<TicketFileEntry>>> =
        use_signal(HashMap::new);
    let loading_files: Signal<HashSet<String>> = use_signal(HashSet::new);
    let mut focused_ticket_id: Signal<Option<String>> = use_signal(|| None);
    let sorted = sorted_tickets(props.tickets.clone(), &props.sort_key);
    let displayed_ticket_ids = sorted
        .iter()
        .map(|ticket| ticket.id.clone())
        .collect::<Vec<_>>();
    {
        let displayed_ticket_ids = displayed_ticket_ids.clone();
        let selected_id = props.selected_id.clone();
        use_effect(move || {
            let current_focus = focused_ticket_id.read().clone();
            let next_focus = resolve_focused_ticket(
                &displayed_ticket_ids,
                current_focus.as_deref(),
                selected_id.as_deref(),
            );
            if current_focus != next_focus {
                focused_ticket_id.set(next_focus);
            }
        });
    }
    let all_checked = props.show_checkboxes
        && !sorted.is_empty()
        && sorted
            .iter()
            .all(|ticket| props.selected_ids.contains(&ticket.id));
    let is_empty = props.tickets.is_empty();

    rsx! {
        {render_filter_controls(
            props.clone(),
            filter,
            state_filter_val,
            all_checked,
            displayed_ticket_ids,
            focused_ticket_id,
        )}
        if props.loading {
            div {
                class: "sidebar-loading",
                "Loading tickets…"
            }
        }
        if let Some(ref error) = props.error {
            div {
                style: "padding: 12px; color: var(--error); font-size: 12px;",
                "Failed to load: {error}"
            }
        }
        if !props.loading {
            if is_empty {
                div {
                    class: "sidebar-empty",
                    "No tickets in this workspace."
                }
            }
            {render_ticket_rows(
                props,
                sorted,
                expanded_ids,
                file_cache,
                loading_files,
                focused_ticket_id,
            )}
        }
    }
}

fn sorted_tickets(
    mut tickets: Vec<crate::types::TicketSummary>,
    sort_key: &str,
) -> Vec<crate::types::TicketSummary> {
    match sort_key {
        "title" => tickets.sort_by(|a, b| a.title.cmp(&b.title)),
        "state" => tickets.sort_by(|a, b| a.state.cmp(&b.state)),
        "created_at" => tickets.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        _ => tickets.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
    }
    tickets
}

fn resolve_focused_ticket(
    displayed_ticket_ids: &[String],
    current_focus: Option<&str>,
    selected_id: Option<&str>,
) -> Option<String> {
    if displayed_ticket_ids.is_empty() {
        return None;
    }

    current_focus
        .filter(|id| displayed_ticket_ids.iter().any(|candidate| candidate == id))
        .map(ToString::to_string)
        .or_else(|| {
            selected_id
                .filter(|id| displayed_ticket_ids.iter().any(|candidate| candidate == id))
                .map(ToString::to_string)
        })
        .or_else(|| displayed_ticket_ids.first().cloned())
}

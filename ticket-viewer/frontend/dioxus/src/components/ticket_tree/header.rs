use dioxus::prelude::*;

use crate::types::{
    TicketRef,
    TicketSummary,
};

use super::{
    TicketTreeProps,
    STATE_CHIPS,
};

pub(super) fn render_filter_controls(
    props: TicketTreeProps,
    filter: String,
    state_filter_val: String,
    all_checked: bool,
    displayed_ticket_ids: Vec<String>,
    mut focused_ticket_id: Signal<Option<String>>,
) -> Element {
    let displayed_ticket_ids_for_focus = displayed_ticket_ids.clone();
    let displayed_ticket_ids_for_keydown = displayed_ticket_ids.clone();
    let selected_id_for_focus = props.selected_id.clone();
    let selected_id_for_keydown = props.selected_id.clone();
    let on_select_for_keydown = props.on_select.clone();

    rsx! {
        div {
            style: "
                padding: 8px 12px;
                border-bottom: 1px solid var(--border-subtle);
                display: flex;
                flex-direction: column;
                gap: 6px;
            ",
            input {
                "data-testid": "ticket-tree-filter",
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
                oninput: move |event| props.on_filter_change.call(event.value()),
                onfocus: move |_| {
                    if let Some(ticket_id) = active_ticket_id(
                        &displayed_ticket_ids_for_focus,
                        focused_ticket_id,
                        selected_id_for_focus.clone(),
                    ) {
                        focused_ticket_id.set(Some(ticket_id));
                    }
                },
                onkeydown: move |event| match event.key() {
                    Key::ArrowDown => {
                        event.prevent_default();
                        move_ticket_focus(
                            &displayed_ticket_ids_for_keydown,
                            focused_ticket_id,
                            selected_id_for_keydown.clone(),
                            1,
                        );
                    },
                    Key::ArrowUp => {
                        event.prevent_default();
                        move_ticket_focus(
                            &displayed_ticket_ids_for_keydown,
                            focused_ticket_id,
                            selected_id_for_keydown.clone(),
                            -1,
                        );
                    },
                    Key::Enter => {
                        if let Some(ticket_id) = active_ticket_id(
                            &displayed_ticket_ids_for_keydown,
                            focused_ticket_id,
                            selected_id_for_keydown.clone(),
                        ) {
                            event.prevent_default();
                            on_select_for_keydown.call(resolve_ticket_ref(
                                &props.tickets,
                                &props.workspace,
                                &ticket_id,
                            ));
                        }
                    },
                    _ => {},
                },
            }
            div {
                style: "display: flex; flex-wrap: wrap; align-items: center; gap: 4px;",
                for &(label, value) in STATE_CHIPS.iter() {
                    {
                        let is_active = state_filter_val.as_str() == value;
                        let chip_class = if is_active {
                            "chip chip--active"
                        } else {
                            "chip chip--neutral"
                        };
                        let value = value.to_string();
                        rsx! {
                            button {
                                key: "{value}",
                                "data-testid": if value.is_empty() {
                                    "ticket-tree-state-chip-all".to_string()
                                } else {
                                    format!("ticket-tree-state-chip-{value}")
                                },
                                class: "{chip_class}",
                                aria_pressed: if is_active { "true" } else { "false" },
                                onclick: move |_| props.on_state_filter_change.call(value.clone()),
                                "{label}"
                            }
                        }
                    }
                }
                div { style: "flex: 1;" }
                if props.show_checkboxes {
                    input {
                        r#type: "checkbox",
                        checked: all_checked,
                        style: "width: 14px; height: 14px; cursor: pointer; accent-color: var(--accent-blue); flex-shrink: 0;",
                        aria_label: "Select all tickets",
                        onchange: move |event| {
                            if let Some(ref handler) = props.on_select_all {
                                handler.call(event.value() == "true");
                            }
                        },
                    }
                }
                if let Some(ref on_batch) = props.on_toggle_batch {
                    button {
                        class: if props.show_checkboxes { "btn btn-secondary btn-sm btn-active" } else { "btn btn-secondary btn-sm" },
                        style: "font-size: 11px; padding: 3px 8px; min-height: 24px;",
                        aria_label: "Toggle batch selection",
                        aria_pressed: if props.show_checkboxes { "true" } else { "false" },
                        onclick: {
                            let on_batch = on_batch.clone();
                            move |_| on_batch.call(())
                        },
                        "☑"
                    }
                }
                if let Some(ref on_new) = props.on_new_ticket {
                    button {
                        class: "btn btn-primary btn-sm",
                        style: "font-size: 11px; padding: 3px 8px; min-height: 24px;",
                        aria_label: "Create new ticket",
                        onclick: {
                            let on_new = on_new.clone();
                            move |_| on_new.call(())
                        },
                        "+ New"
                    }
                }
            }
        }
    }
}

fn active_ticket_id(
    displayed_ticket_ids: &[String],
    focused_ticket_id: Signal<Option<String>>,
    selected_id: Option<String>,
) -> Option<String> {
    let focused = focused_ticket_id.read().clone();
    focused
        .filter(|id| displayed_ticket_ids.iter().any(|candidate| candidate == id))
        .or_else(|| {
            selected_id.filter(|id| {
                displayed_ticket_ids.iter().any(|candidate| candidate == id)
            })
        })
        .or_else(|| displayed_ticket_ids.first().cloned())
}

fn move_ticket_focus(
    displayed_ticket_ids: &[String],
    mut focused_ticket_id: Signal<Option<String>>,
    selected_id: Option<String>,
    delta: i32,
) {
    if displayed_ticket_ids.is_empty() {
        focused_ticket_id.set(None);
        return;
    }

    let current_id = active_ticket_id(
        displayed_ticket_ids,
        focused_ticket_id,
        selected_id,
    );
    let current_index = current_id
        .as_ref()
        .and_then(|id| {
            displayed_ticket_ids
                .iter()
                .position(|candidate| candidate == id)
        })
        .unwrap_or(0);
    let next_index = if delta.is_negative() {
        current_index.saturating_sub(1)
    } else {
        (current_index + 1).min(displayed_ticket_ids.len().saturating_sub(1))
    };

    focused_ticket_id.set(Some(displayed_ticket_ids[next_index].clone()));
}

fn resolve_ticket_ref(
    tickets: &[TicketSummary],
    workspace: &str,
    ticket_id: &str,
) -> TicketRef {
    tickets
        .iter()
        .find(|ticket| ticket.id == ticket_id)
        .map(|ticket| ticket.resolved_ticket_ref(workspace))
        .unwrap_or_else(|| TicketRef::new(workspace, ticket_id.to_string()))
}

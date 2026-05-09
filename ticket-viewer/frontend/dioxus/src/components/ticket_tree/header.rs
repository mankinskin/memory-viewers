use dioxus::prelude::*;

use super::{TicketTreeProps, STATE_CHIPS};

pub(super) fn render_filter_controls(
    props: TicketTreeProps,
    filter: String,
    state_filter_val: String,
    all_checked: bool,
) -> Element {
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
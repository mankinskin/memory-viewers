use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;

use crate::api::{HttpTicketBackend, TicketBackend};
use crate::types::{TicketFileEntry, TicketSummary};

use super::TicketTreeProps;

pub(super) fn render_ticket_rows(
    props: TicketTreeProps,
    sorted: Vec<TicketSummary>,
    expanded_ids: Signal<HashSet<String>>,
    file_cache: Signal<HashMap<String, Vec<TicketFileEntry>>>,
    loading_files: Signal<HashSet<String>>,
) -> Element {
    rsx! {
        for ticket in sorted {
            {render_ticket_entry(
                props.clone(),
                ticket,
                expanded_ids,
                file_cache,
                loading_files,
            )}
        }
    }
}

fn render_ticket_entry(
    props: TicketTreeProps,
    ticket: TicketSummary,
    expanded_ids: Signal<HashSet<String>>,
    file_cache: Signal<HashMap<String, Vec<TicketFileEntry>>>,
    loading_files: Signal<HashSet<String>>,
) -> Element {
    let ticket_id = ticket.id.clone();
    let ticket_id_toggle = ticket_id.clone();
    let ticket_id_expand = ticket_id.clone();
    let ticket_id_select = ticket_id.clone();
    let title = ticket.title.clone().unwrap_or_else(|| "Untitled".into());
    let state = ticket.state.clone().unwrap_or_else(|| "new".into());
    let is_selected = props.selected_id.as_deref() == Some(ticket_id.as_str());
    let is_checked = props.selected_ids.contains(&ticket_id);
    let is_expanded = expanded_ids.read().contains(&ticket_id);
    let dot_color = crate::types::state_accent(Some(&state));
    let row_bg = if is_selected {
        "var(--bg-active)"
    } else {
        "transparent"
    };
    let row_border = if is_selected {
        format!("border-left: 2px solid {dot_color};")
    } else {
        "border-left: 2px solid transparent;".to_string()
    };
    let arrow = if is_expanded { "▾" } else { "▸" };
    let on_toggle_select = props.on_toggle_select.clone();
    let on_select = props.on_select.clone();
    let on_select_file = props.on_select_file.clone();
    let selected_file = props.selected_file.clone();
    let workspace = props.workspace.clone();

    rsx! {
        div { key: "{ticket_id}",
            div {
                style: "
                    display: flex;
                    align-items: center;
                    background: {row_bg};
                    {row_border}
                ",
                if props.show_checkboxes {
                    div {
                        style: "display: flex; align-items: center; padding: 0 6px; flex-shrink: 0;",
                        input {
                            r#type: "checkbox",
                            checked: is_checked,
                            style: "width: 12px; height: 12px; cursor: pointer; accent-color: var(--accent-blue);",
                            aria_label: "Select ticket",
                            onclick: move |event| event.stop_propagation(),
                            onchange: move |_| {
                                if let Some(ref handler) = on_toggle_select {
                                    handler.call(ticket_id_toggle.clone());
                                }
                            },
                        }
                    }
                }
                button {
                    style: "
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        width: 20px;
                        height: 100%;
                        min-height: 28px;
                        padding: 0;
                        border: none;
                        background: transparent;
                        color: var(--text-muted);
                        cursor: pointer;
                        font-size: 16px;
                        flex-shrink: 0;
                    ",
                    aria_label: if is_expanded { "Collapse ticket files" } else { "Expand ticket files" },
                    onclick: move |event| {
                        event.stop_propagation();
                        toggle_ticket_expansion(
                            ticket_id_expand.clone(),
                            workspace.clone(),
                            expanded_ids,
                            file_cache,
                            loading_files,
                        );
                    },
                    "{arrow}"
                }
                button {
                    style: "
                        display: flex;
                        align-items: center;
                        gap: 6px;
                        flex: 1;
                        padding: 5px 8px 5px 4px;
                        border: none;
                        background: transparent;
                        color: var(--text-primary);
                        cursor: pointer;
                        text-align: left;
                        min-height: 0;
                        overflow: hidden;
                        white-space: nowrap;
                    ",
                    onclick: move |_| on_select.call(ticket_id_select.clone()),
                    span {
                        style: "
                            width: 7px;
                            height: 7px;
                            border-radius: 50%;
                            background: {dot_color};
                            flex-shrink: 0;
                        ",
                        aria_hidden: "true",
                    }
                    span {
                        style: "
                            font-size: 12px;
                            font-weight: 400;
                            overflow: hidden;
                            text-overflow: ellipsis;
                            white-space: nowrap;
                            flex: 1;
                            min-width: 0;
                        ",
                        "{title}"
                    }
                    span {
                        style: "
                            font-size: 10px;
                            color: var(--text-muted);
                            flex-shrink: 0;
                            white-space: nowrap;
                        ",
                        "{state}"
                    }
                }
            }
            if is_expanded {
                {render_expanded_files(
                    ticket.id,
                    selected_file,
                    on_select_file,
                    file_cache,
                    loading_files,
                )}
            }
        }
    }
}

fn render_expanded_files(
    ticket_id: String,
    selected_file: Option<(String, String)>,
    on_select_file: Option<EventHandler<(String, String)>>,
    file_cache: Signal<HashMap<String, Vec<TicketFileEntry>>>,
    loading_files: Signal<HashSet<String>>,
) -> Element {
    let is_loading_files = loading_files.read().contains(&ticket_id);
    let cached_files = file_cache.read().get(&ticket_id).cloned();

    rsx! {
        if is_loading_files {
            div {
                style: "
                    padding: 3px 8px 3px 36px;
                    font-size: 11px;
                    color: var(--text-muted);
                ",
                "Loading…"
            }
        } else if let Some(files) = cached_files {
            if files.is_empty() {
                div {
                    style: "
                        padding: 3px 8px 3px 36px;
                        font-size: 11px;
                        color: var(--text-muted);
                        font-style: italic;
                    ",
                    "No files"
                }
            } else {
                for file in files {
                    {
                        let file_ticket_id = ticket_id.clone();
                        let file_path = file.path.clone();
                        let file_name = file.name.clone();
                        let is_active = selected_file
                            .as_ref()
                            .map(|(ticket, path)| ticket == &file_ticket_id && path == &file_path)
                            .unwrap_or(false);
                        let file_bg = if is_active {
                            "background: var(--bg-active);"
                        } else {
                            ""
                        };
                        rsx! {
                            button {
                                key: "{file_path}",
                                style: "
                                    display: flex;
                                    align-items: center;
                                    gap: 5px;
                                    width: 100%;
                                    padding: 3px 8px 3px 36px;
                                    border: none;
                                    background: transparent;
                                    color: var(--text-secondary);
                                    cursor: pointer;
                                    text-align: left;
                                    font-size: 11px;
                                    white-space: nowrap;
                                    overflow: hidden;
                                    text-overflow: ellipsis;
                                    {file_bg}
                                ",
                                onclick: move |_| {
                                    if let Some(ref handler) = on_select_file {
                                        handler.call((file_ticket_id.clone(), file_path.clone()));
                                    }
                                },
                                span { aria_hidden: "true", "📄" }
                                span {
                                    style: "overflow: hidden; text-overflow: ellipsis;",
                                    "{file_name}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn toggle_ticket_expansion(
    ticket_id: String,
    workspace: String,
    mut expanded_ids: Signal<HashSet<String>>,
    mut file_cache: Signal<HashMap<String, Vec<TicketFileEntry>>>,
    mut loading_files: Signal<HashSet<String>>,
) {
    let already_expanded = expanded_ids.read().contains(&ticket_id);
    if already_expanded {
        expanded_ids.write().remove(&ticket_id);
        return;
    }

    expanded_ids.write().insert(ticket_id.clone());
    if file_cache.read().contains_key(&ticket_id) || loading_files.read().contains(&ticket_id) {
        return;
    }

    loading_files.write().insert(ticket_id.clone());
    spawn(async move {
        let backend = HttpTicketBackend::new(None);
        if let Ok(response) = backend.list_ticket_files(&workspace, &ticket_id).await {
            file_cache.write().insert(ticket_id.clone(), response.files);
        }
        loading_files.write().remove(&ticket_id);
    });
}
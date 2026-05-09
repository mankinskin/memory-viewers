use dioxus::prelude::*;
use viewer_api_dioxus::FileContentViewer;

use crate::components::history::HistoryPanel;
use crate::types::HistoryEntry;

use super::{Tab, TAB_ACTIVE_STYLE, TAB_BASE_STYLE};

pub(super) fn render_tab_bar(
    mut active_tab: Signal<Tab>,
    content_tab_label: String,
    show_edit_tab: bool,
    history_count: usize,
) -> Element {
    rsx! {
        div {
            role: "tablist",
            "data-testid": "ticket-content-tabs",
            style: "
                display: flex;
                border-bottom: 1px solid #3a3a4a;
                flex-shrink: 0;
            ",
            button {
                role: "tab",
                "aria-selected": if active_tab() == Tab::Description { "true" } else { "false" },
                "data-testid": "tab-description",
                style: if active_tab() == Tab::Description { TAB_ACTIVE_STYLE } else { TAB_BASE_STYLE },
                onclick: move |_| active_tab.set(Tab::Description),
                "{content_tab_label}"
            }
            button {
                role: "tab",
                "aria-selected": if active_tab() == Tab::Toml { "true" } else { "false" },
                "data-testid": "tab-toml",
                style: if active_tab() == Tab::Toml { TAB_ACTIVE_STYLE } else { TAB_BASE_STYLE },
                onclick: move |_| active_tab.set(Tab::Toml),
                "TOML"
            }
            if show_edit_tab {
                button {
                    role: "tab",
                    "aria-selected": if active_tab() == Tab::Edit { "true" } else { "false" },
                    "data-testid": "tab-edit",
                    style: if active_tab() == Tab::Edit { TAB_ACTIVE_STYLE } else { TAB_BASE_STYLE },
                    onclick: move |_| active_tab.set(Tab::Edit),
                    "Edit"
                }
            }
            if history_count > 1 {
                button {
                    role: "tab",
                    "aria-selected": if active_tab() == Tab::History { "true" } else { "false" },
                    "data-testid": "tab-history",
                    style: if active_tab() == Tab::History { TAB_ACTIVE_STYLE } else { TAB_BASE_STYLE },
                    onclick: move |_| active_tab.set(Tab::History),
                    "History"
                }
            }
        }
    }
}

pub(super) fn render_description_panel(
    desc_loading: bool,
    desc_error: Option<String>,
    desc_text: Option<String>,
    asset_path: Option<String>,
) -> Element {
    if desc_loading {
        return rsx! {
            div {
                "data-testid": "desc-loading",
                style: "color: #6b7280; font-size: 13px;",
                "Loading…"
            }
        };
    }

    if let Some(error) = desc_error {
        return rsx! {
            div {
                "data-testid": "desc-error",
                style: "color: #f87171; font-size: 13px;",
                strong { "Error: " }
                "{error}"
            }
        };
    }

    if let Some(content) = desc_text {
        let display_filename = asset_path
            .as_deref()
            .unwrap_or("description.md")
            .to_string();
        return rsx! {
            div {
                "data-testid": "desc-markdown",
                FileContentViewer {
                    content,
                    filename: display_filename,
                }
            }
        };
    }

    rsx! {
        em {
            "data-testid": "desc-empty",
            style: "color: #6b7280; font-size: 13px;",
            "No content found."
        }
    }
}

pub(super) fn render_toml_panel(toml_text: String) -> Element {
    rsx! {
        pre {
            "data-testid": "toml-content",
            style: "
                font-family: 'Cascadia Code', 'Fira Code', monospace;
                font-size: 12.5px;
                line-height: 1.6;
                color: #e0e0e8;
                margin: 0;
                white-space: pre;
                overflow-x: auto;
            ",
            "{toml_text}"
        }
    }
}

pub(super) fn render_history_panel(
    workspace: String,
    ticket_id: String,
    entries: Vec<HistoryEntry>,
    mut history_refresh_key: Signal<u32>,
) -> Element {
    rsx! {
        HistoryPanel {
            workspace,
            ticket_id,
            entries,
            on_refresh: move |_| {
                history_refresh_key.set(history_refresh_key() + 1);
            },
        }
    }
}
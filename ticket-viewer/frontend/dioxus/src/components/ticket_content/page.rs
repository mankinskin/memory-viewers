use dioxus::prelude::*;

use crate::{
    api::{
        HttpTicketBackend,
        TicketBackend,
    },
    types::HistoryEntry,
};

use super::{
    edit::render_edit_panel,
    helpers::{
        content_tab_label,
        fields_to_toml,
    },
    render::{
        render_description_panel,
        render_history_panel,
        render_tab_bar,
        render_toml_panel,
    },
    Tab,
};

#[component]
pub fn TicketContent(
    workspace: String,
    ticket_id: String,
    fields: serde_json::Value,
    #[props(default)] asset_path: Option<String>,
) -> Element {
    let mut active_tab: Signal<Tab> = use_signal(|| Tab::Description);
    let mut desc_loading: Signal<bool> = use_signal(|| true);
    let mut desc_text: Signal<Option<String>> = use_signal(|| None);
    let mut desc_error: Signal<Option<String>> = use_signal(|| None);
    let mut history_entries: Signal<Vec<HistoryEntry>> = use_signal(|| vec![]);
    let mut history_refresh_key: Signal<u32> = use_signal(|| 0);
    let mut edit_draft: Signal<String> = use_signal(String::new);
    let mut edit_pending: Signal<bool> = use_signal(|| false);
    let mut edit_success: Signal<bool> = use_signal(|| false);
    let mut edit_error: Signal<Option<String>> = use_signal(|| None);

    let is_description = asset_path
        .as_deref()
        .map(|path| path == "description.md")
        .unwrap_or(true);
    let content_tab_label =
        content_tab_label(asset_path.as_deref(), is_description);
    let show_edit_tab = is_description && HttpTicketBackend::has_auth_token();

    {
        let workspace = workspace.clone();
        let ticket_id = ticket_id.clone();
        let asset_path = asset_path.clone();
        let is_description = is_description;
        use_effect(move || {
            let workspace = workspace.clone();
            let ticket_id = ticket_id.clone();
            let asset_path = asset_path.clone();
            desc_loading.set(true);
            desc_text.set(None);
            desc_error.set(None);
            edit_draft.set(String::new());
            edit_success.set(false);
            edit_error.set(None);
            active_tab.set(Tab::Description);
            spawn(async move {
                let backend = HttpTicketBackend::new(None);
                if is_description {
                    match backend
                        .get_ticket_description(&workspace, &ticket_id)
                        .await
                    {
                        Ok(response) => {
                            let content =
                                response.description.unwrap_or_default();
                            edit_draft.set(content.clone());
                            desc_text.set(if content.is_empty() {
                                None
                            } else {
                                Some(content)
                            });
                            desc_loading.set(false);
                        },
                        Err(error) => {
                            desc_loading.set(false);
                            desc_error.set(Some(error));
                        },
                    }
                } else {
                    let path = asset_path.unwrap_or_default();
                    match backend
                        .get_ticket_asset(&workspace, &ticket_id, &path)
                        .await
                    {
                        Ok(response) => {
                            let content = response.content;
                            desc_text.set(if content.is_empty() {
                                None
                            } else {
                                Some(content)
                            });
                            desc_loading.set(false);
                        },
                        Err(error) => {
                            desc_loading.set(false);
                            desc_error.set(Some(error));
                        },
                    }
                }
            });
        });
    }

    {
        let workspace = workspace.clone();
        let ticket_id = ticket_id.clone();
        use_effect(move || {
            let _refresh = history_refresh_key();
            let workspace = workspace.clone();
            let ticket_id = ticket_id.clone();
            history_entries.set(vec![]);
            spawn(async move {
                let backend = HttpTicketBackend::new(None);
                if let Ok(response) =
                    backend.get_ticket_history(&workspace, &ticket_id).await
                {
                    history_entries.set(response.entries);
                }
            });
        });
    }

    let toml_text = fields_to_toml(&ticket_id, &fields);
    let body_style = if active_tab() == Tab::History {
        "flex: 1; overflow: hidden;"
    } else {
        "flex: 1; overflow-y: auto; padding: 20px 24px;"
    };

    rsx! {
        div {
            "data-testid": "ticket-content",
            style: "
                display: flex;
                flex-direction: column;
                height: 100%;
                font-family: sans-serif;
                color: #e0e0e8;
                overflow: hidden;
                background: transparent;
                backdrop-filter: blur(var(--panel-blur)) saturate(var(--panel-saturate));
                -webkit-backdrop-filter: blur(var(--panel-blur)) saturate(var(--panel-saturate));
            ",
            {render_tab_bar(
                active_tab,
                content_tab_label,
                show_edit_tab,
                history_entries().len(),
            )}
            div {
                "data-testid": "ticket-content-body",
                style: "{body_style}",
                if active_tab() == Tab::Description {
                    {render_description_panel(desc_loading(), desc_error(), desc_text(), asset_path.clone())}
                }
                if active_tab() == Tab::Toml {
                    {render_toml_panel(toml_text)}
                }
                if active_tab() == Tab::History {
                    {render_history_panel(
                        workspace.clone(),
                        ticket_id.clone(),
                        history_entries(),
                        history_refresh_key,
                    )}
                }
                if active_tab() == Tab::Edit {
                    {render_edit_panel(
                        workspace,
                        ticket_id,
                        desc_text,
                        edit_draft,
                        edit_pending,
                        edit_success,
                        edit_error,
                    )}
                }
            }
        }
    }
}

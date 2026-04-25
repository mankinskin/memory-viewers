//! `SpecDetail` — right-side panel showing spec body, sections, CodeRefs, and health.
//!
//! Tabs: `body` | `sections` | `code-refs` | `health`

use dioxus::prelude::*;
use wasm_bindgen_futures::spawn_local;
use viewer_api_dioxus::FileContentViewer;

use crate::api;
use crate::types::{HealthResponse, SpecFullResponse, SectionResponse};
use super::code_ref_list::CodeRefList;
use super::health_panel::HealthPanel;
use super::state_badge::StateBadge;

// ── Tabs ──────────────────────────────────────────────────────────────────────

const TABS: &[(&str, &str)] = &[
    ("body", "Body"),
    ("sections", "Sections"),
    ("code-refs", "CodeRefs"),
    ("health", "Health"),
];

// ── Props ─────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct SpecDetailProps {
    pub spec_id: String,
    pub active_tab: String,
    pub on_tab_change: EventHandler<String>,
}

// ── Component ─────────────────────────────────────────────────────────────────

#[component]
pub fn SpecDetail(props: SpecDetailProps) -> Element {
    // ── Full spec (body + sections list + code_refs) ──────────────────────
    let mut full: Signal<Option<SpecFullResponse>> = use_signal(|| None);
    let mut full_loading: Signal<bool> = use_signal(|| true);
    let mut full_error: Signal<Option<String>> = use_signal(|| None);

    let spec_id_fetch = props.spec_id.clone();
    use_effect(move || {
        full.set(None);
        full_loading.set(true);
        full_error.set(None);
        let id = spec_id_fetch.clone();
        spawn_local(async move {
            match api::get_spec_full(&id).await {
                Ok(resp) => {
                    full.set(Some(resp));
                    full_loading.set(false);
                }
                Err(e) => {
                    full_error.set(Some(e));
                    full_loading.set(false);
                }
            }
        });
    });

    // ── Expanded section body ─────────────────────────────────────────────
    let mut expanded_section: Signal<Option<String>> = use_signal(|| None);
    let mut section_body: Signal<Option<SectionResponse>> = use_signal(|| None);
    let mut section_loading: Signal<bool> = use_signal(|| false);
    let mut section_error: Signal<Option<String>> = use_signal(|| None);

    // ── Health ────────────────────────────────────────────────────────────
    let mut health: Signal<Option<HealthResponse>> = use_signal(|| None);
    let mut health_loading: Signal<bool> = use_signal(|| false);
    let mut health_error: Signal<Option<String>> = use_signal(|| None);

    let spec_id_health = props.spec_id.clone();
    let active_tab_clone = props.active_tab.clone();
    use_effect(move || {
        if active_tab_clone == "health" && health.read().is_none() && !*health_loading.read() {
            health_loading.set(true);
            health_error.set(None);
            let id = spec_id_health.clone();
            spawn_local(async move {
                match api::health_check(Some(&id)).await {
                    Ok(resp) => {
                        health.set(Some(resp));
                        health_loading.set(false);
                    }
                    Err(e) => {
                        health_error.set(Some(e));
                        health_loading.set(false);
                    }
                }
            });
        }
    });

    let full_data = full.read();
    let title = full_data
        .as_ref()
        .and_then(|f| f.spec.fields.get("title").and_then(|v| v.as_str()))
        .unwrap_or("Untitled")
        .to_string();
    let state = full_data
        .as_ref()
        .and_then(|f| f.spec.fields.get("state").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    rsx! {
        div {
            style: "
                display: flex;
                flex-direction: column;
                height: 100%;
                overflow: hidden;
                background: var(--bg-primary);
            ",

            // ── Header ────────────────────────────────────────────────────
            div {
                style: "
                    display: flex;
                    align-items: center;
                    gap: 10px;
                    padding: 12px 16px;
                    border-bottom: 1px solid var(--border-color);
                    flex-shrink: 0;
                ",
                span {
                    style: "font-size: 14px; font-weight: 600; color: var(--text-primary); flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
                    "{title}"
                }
                if !state.is_empty() {
                    StateBadge { state }
                }
            }

            // ── Tab bar ───────────────────────────────────────────────────
            div {
                style: "
                    display: flex;
                    gap: 0;
                    border-bottom: 1px solid var(--border-color);
                    flex-shrink: 0;
                    padding: 0 12px;
                ",
                for (key, label) in TABS.iter() {
                    {
                        let key_str = key.to_string();
                        let label_str = label.to_string();
                        let is_active = props.active_tab == *key;
                        let border = if is_active {
                            "border-bottom: 2px solid var(--accent-blue);"
                        } else {
                            "border-bottom: 2px solid transparent;"
                        };
                        let color = if is_active {
                            "var(--accent-blue)"
                        } else {
                            "var(--text-muted)"
                        };
                        rsx! {
                            button {
                                key: "{key_str}",
                                style: "
                                    background: none;
                                    border: none;
                                    padding: 8px 12px;
                                    font-size: 12px;
                                    font-weight: 500;
                                    cursor: pointer;
                                    color: {color};
                                    {border}
                                ",
                                onclick: move |_| props.on_tab_change.call(key_str.clone()),
                                "{label_str}"
                            }
                        }
                    }
                }
            }

            // ── Tab content ───────────────────────────────────────────────
            div {
                style: "flex: 1; overflow-y: auto; padding: 16px;",

                if *full_loading.read() {
                    p { style: "color: var(--text-muted); font-size: 13px;", "Loading…" }
                } else if let Some(err) = full_error.read().as_deref() {
                    p { style: "color: var(--accent-red); font-size: 13px;", "{err}" }
                } else if let Some(data) = full_data.as_ref() {
                    {
                        match props.active_tab.as_str() {
                            "body" => rsx! {
                                div {
                                    style: "color: var(--text-primary);",
                                    FileContentViewer {
                                        content: data.body.clone(),
                                        filename: "body.md".to_string(),
                                    }
                                }
                            },
                            "sections" => {
                                let sections = data.sections.clone();
                                let spec_id_sec = props.spec_id.clone();
                                rsx! {
                                    div {
                                        style: "display: flex; flex-direction: column; gap: 6px;",
                                        if sections.is_empty() {
                                                p { style: "color: var(--text-muted); font-size: 13px;", "No sections." }
                                        } else {
                                            for section_name in sections.iter() {
                                                {
                                                    let sn = section_name.clone();
                                                    let sn2 = sn.clone();
                                                    let id_clone = spec_id_sec.clone();
                                                    let is_expanded = expanded_section.read().as_deref() == Some(&sn);
                                                    rsx! {
                                                        div {
                                                            key: "{sn}",
                                                            style: "border: 1px solid var(--border-color); border-radius: 6px; overflow: hidden;",
                                                            button {
                                                                style: "
                                                                    width: 100%;
                                                                    text-align: left;
                                                                    padding: 8px 12px;
                                                                    background: var(--bg-secondary);
                                                                    border: none;
                                                                    color: var(--text-primary);
                                                                    font-size: 13px;
                                                                    font-weight: 500;
                                                                    cursor: pointer;
                                                                ",
                                                                onclick: move |_| {
                                                                    if is_expanded {
                                                                        expanded_section.set(None);
                                                                        section_body.set(None);
                                                                    } else {
                                                                        expanded_section.set(Some(sn.clone()));
                                                                        section_body.set(None);
                                                                        section_loading.set(true);
                                                                        section_error.set(None);
                                                                        let id = id_clone.clone();
                                                                        let name = sn.clone();
                                                                        spawn_local(async move {
                                                                            match api::get_section(&id, &name).await {
                                                                                Ok(resp) => {
                                                                                    section_body.set(Some(resp));
                                                                                    section_loading.set(false);
                                                                                }
                                                                                Err(e) => {
                                                                                    section_error.set(Some(e));
                                                                                    section_loading.set(false);
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                },
                                                                span {
                                                                    style: "margin-right: 6px; color: var(--text-muted);",
                                                                    if is_expanded { "▾" } else { "▸" }
                                                                }
                                                                "{sn2}"
                                                            }
                                                            if is_expanded {
                                                                div {
                                                                    style: "padding: 12px 16px; background: var(--bg-primary); border-top: 1px solid var(--border-color);",
                                                                    if *section_loading.read() {
                                                                        p { style: "color: var(--text-muted); font-size: 12px;", "Loading section…" }
                                                                    } else if let Some(err) = section_error.read().as_deref() {
                                                                        p { style: "color: var(--accent-red); font-size: 12px;", "{err}" }
                                                                    } else if let Some(sb) = section_body.read().as_ref() {
                                                                        FileContentViewer {
                                                                            content: sb.content.clone(),
                                                                            filename: "section.md".to_string(),
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            "code-refs" => rsx! {
                                CodeRefList { refs: data.spec.code_refs.clone() }
                            },
                            "health" => rsx! {
                                HealthPanel {
                                    health: health.read().clone(),
                                    loading: *health_loading.read(),
                                    error: health_error.read().clone(),
                                }
                            },
                            _ => rsx! { div {} },
                        }
                    }
                }
            }
        }
    }
}

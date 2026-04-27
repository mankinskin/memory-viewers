//! Dioxus Router route definitions for the spec viewer SPA.
//!
//! Route hierarchy:
//!
//!   /                → Redirect to /specs
//!   /specs           → SpecListPage (flat list + detail panel)
//!   /specs/tree      → SpecTreePage (hierarchical parent-child tree)
//!   /specs/:id       → SpecDetailPage (detail page navigated directly)

use dioxus::prelude::*;
use viewer_api_dioxus::{Header, Layout, Sidebar, ThemeSettings};
use wasm_bindgen_futures::spawn_local;

use crate::api;
use crate::components::spec_detail::SpecDetail;
use crate::components::spec_tree::SpecTree;
use crate::sse::use_sse;
use crate::store::SpecListStore;
use crate::types::SpecSummary;

// ── Route enum ────────────────────────────────────────────────────────────────

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[redirect("/", || Route::SpecListPage {})]
    #[route("/specs")]
    SpecListPage {},

    #[route("/specs/tree")]
    SpecTreePage {},

    #[route("/specs/:id")]
    SpecDetailPage { id: String },
}

// ── SpecListPage ──────────────────────────────────────────────────────────────

#[component]
pub fn SpecListPage() -> Element {
    let store = SpecListStore::use_store();

    let store_persist = store;
    use_effect(move || {
        store_persist.persist();
    });

    let mut sidebar_collapsed = use_signal(|| false);
    let mut show_theme_settings = use_signal(|| false);

    let mut specs: Signal<Vec<SpecSummary>> = use_signal(Vec::new);
    let mut loading: Signal<bool> = use_signal(|| true);
    let mut list_error: Signal<Option<String>> = use_signal(|| None);

    let mut selected_id = store.open_spec_id;
    let mut filter = store.filter;
    let mut state_filter = store.state_filter;
    let mut active_tab = store.active_tab;

    // Load spec list on mount and when filter / state_filter change.
    use_effect(move || {
        loading.set(true);
        list_error.set(None);
        let q_val = filter.read().clone();
        let s_val = state_filter.read().clone();
        let q: Option<String> = if q_val.trim().is_empty() { None } else { Some(q_val.trim().to_string()) };
        let s: Option<String> = if s_val.is_empty() { None } else { Some(s_val) };
        spawn_local(async move {
            match api::list_specs(s.as_deref(), q.as_deref(), None).await {
                Ok(resp) => {
                    specs.set(resp.items);
                    loading.set(false);
                }
                Err(e) => {
                    list_error.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    // SSE — keep list fresh from server push.
    use_sse(specs);

    rsx! {
        Layout {
            header: rsx! {
                Header {
                    left: rsx! {
                        span { class: "header-icon", "📐" }
                        span { class: "header-title", "Spec Viewer" }
                    },
                    right: rsx! {
                        button {
                            style: "
                                padding: 6px 10px; border-radius: 6px;
                                border: 1px solid var(--border-subtle);
                                background: var(--bg-secondary);
                                color: var(--text-primary);
                                cursor: pointer; font-size: 14px;
                                min-height: 32px;
                            ",
                            aria_label: "Theme settings",
                            onclick: move |_| {
                                let cur = *show_theme_settings.read();
                                show_theme_settings.set(!cur);
                            },
                            "🎨"
                        }
                    },
                }
            },
            Sidebar {
                title: "Specifications".to_string(),
                collapsed: *sidebar_collapsed.read(),
                on_toggle: move |_| {
                    let cur = *sidebar_collapsed.read();
                    sidebar_collapsed.set(!cur);
                },
                SpecTree {
                        specs: specs.read().clone(),
                        loading: *loading.read(),
                        error: list_error.read().clone(),
                        filter: filter.read().clone(),
                        on_filter_change: move |v: String| {
                            filter.set(v.clone());
                            // Re-run search if it looks like a search query
                            if v.len() >= 2 {
                                let q = v.clone();
                                spawn_local(async move {
                                    if let Ok(resp) = api::search_specs(&q, Some(50)).await {
                                        specs.set(resp.items);
                                    }
                                });
                            } else if v.is_empty() {
                                let s = state_filter.read().clone();
                                let s_opt = if s.is_empty() { None } else { Some(s) };
                                spawn_local(async move {
                                    if let Ok(resp) = api::list_specs(s_opt.as_deref(), None, None).await {
                                        specs.set(resp.items);
                                    }
                                });
                            }
                        },
                        state_filter: state_filter.read().clone(),
                        on_state_filter_change: move |v: String| {
                            state_filter.set(v.clone());
                            let s_opt = if v.is_empty() { None } else { Some(v) };
                            let q = filter.read().clone();
                            let q_opt = if q.is_empty() { None } else { Some(q) };
                            spawn_local(async move {
                                if let Ok(resp) = api::list_specs(s_opt.as_deref(), q_opt.as_deref(), None).await {
                                    specs.set(resp.items);
                                }
                            });
                        },
                        selected_id: selected_id.read().clone(),
                        on_select: move |id: String| {
                            selected_id.set(Some(id));
                            active_tab.set("body".to_string());
                        },
                    }
                }

            // ── Main content ──────────────────────────────────────────────
            div {
                class: "content",
                if let Some(id) = selected_id.read().clone() {
                    SpecDetail {
                        spec_id: id,
                        active_tab: active_tab.read().clone(),
                        on_tab_change: move |tab| active_tab.set(tab),
                    }
                } else {
                    div {
                        style: "
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            height: 100%;
                            color: #6b7280;
                            font-size: 14px;
                        ",
                        "Select a specification to view details."
                    }
                }
            }
        }

        // ── Theme settings panel — floats above everything ──────────────────
        if *show_theme_settings.read() {
            div {
                style: "
                    position: fixed; top: 48px; right: 16px; z-index: 200;
                    max-height: calc(100vh - 64px); overflow-y: auto;
                ",
                ThemeSettings {
                    on_close: move |_| show_theme_settings.set(false),
                }
            }
        }
    }
}

// ── SpecTreePage ──────────────────────────────────────────────────────────────

#[component]
pub fn SpecTreePage() -> Element {
    let mut specs: Signal<Vec<SpecSummary>> = use_signal(Vec::new);
    let mut loading: Signal<bool> = use_signal(|| true);
    let nav = use_navigator();

    // Load all specs on mount to build tree.
    use_effect(move || {
        spawn_local(async move {
            if let Ok(resp) = api::list_specs(None, None, None).await {
                specs.set(resp.items);
                loading.set(false);
            }
        });
    });

    rsx! {
        div {
            style: "display: flex; flex-direction: column; height: 100%;",
            div {
                style: "padding: 16px; border-bottom: 1px solid #1f2937;",
                h2 {
                    style: "color: #f9fafb; font-size: 16px; font-weight: 600; margin: 0;",
                    "Specification Tree"
                }
            }
            div {
                style: "flex: 1; overflow-y: auto; padding: 16px;",
                if *loading.read() {
                    p { style: "color: #6b7280;", "Loading…" }
                } else {
                    for spec in specs.read().iter() {
                        {
                            let id = spec.id.clone();
                            let nav2 = nav.clone();
                            rsx! {
                                button {
                                    key: "{id}",
                                    style: "
                                        display: block;
                                        width: 100%;
                                        text-align: left;
                                        padding: 8px 12px;
                                        background: none;
                                        border: none;
                                        border-bottom: 1px solid #1f2937;
                                        color: #e5e7eb;
                                        cursor: pointer;
                                        font-size: 13px;
                                    ",
                                    onclick: move |_| { nav2.push(Route::SpecDetailPage { id: id.clone() }); },
                                    {spec.title.as_deref().unwrap_or("Untitled")}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── SpecDetailPage ────────────────────────────────────────────────────────────

#[component]
pub fn SpecDetailPage(id: String) -> Element {
    let mut active_tab = use_signal(|| "body".to_string());

    rsx! {
        div {
            style: "height: 100%;",
            SpecDetail {
                spec_id: id,
                active_tab: active_tab.read().clone(),
                on_tab_change: move |tab| active_tab.set(tab),
            }
        }
    }
}

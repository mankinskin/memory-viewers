//! Dioxus Router route definitions for the spec viewer SPA.
//!
//! Route hierarchy:
//!
//!   /                → Redirect to /specs
//!   /specs           → SpecListPage (flat list + detail panel)
//!   /specs/tree      → SpecTreePage (hierarchical parent-child tree)
//!   /specs/:id       → SpecDetailPage (detail page navigated directly)

use dioxus::prelude::*;
use viewer_api_dioxus::{
    BreadcrumbItem, Breadcrumbs, Header, Layout, Sidebar, TabBar, TabItem, TabsStore, ThemeSettings,
};
use wasm_bindgen_futures::spawn_local;

use crate::api;
use crate::components::spec_detail::SpecDetail;
use crate::components::spec_graph::SpecGraphPage as SpecGraphView;
use crate::components::spec_tree::SpecTree;
use crate::sse::use_sse;
use crate::store::SpecListStore;
use crate::types::SpecSummary;

/// Resolve a tab label for `id` from the loaded spec list, falling back to
/// the bare id when the spec hasn't loaded yet (e.g. on URL deep-link).
fn label_for(specs: &[SpecSummary], id: &str) -> String {
    specs
        .iter()
        .find(|s| s.id == id)
        .and_then(|s| s.title.clone())
        .unwrap_or_else(|| id.to_string())
}

// ── Route enum ────────────────────────────────────────────────────────────────

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[redirect("/", || Route::SpecListPage {})]
    #[route("/specs")]
    SpecListPage {},

    #[route("/specs/tree")]
    SpecTreePage {},

    #[route("/specs/graph")]
    SpecGraphPage {},

    #[route("/specs/:id")]
    SpecDetailPage { id: String },
}

#[component]
pub fn SpecGraphPage() -> Element {
    rsx! { SpecGraphView {} }
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

    // Multi-spec tab store. Tab id == spec id; payload == cached label.
    let mut tabs: TabsStore<String> = use_hook(TabsStore::new);

    // Hydrate the tab set from the persisted `selected_id` on first mount.
    use_hook(|| {
        if let Some(id) = selected_id.peek().clone() {
            tabs.open(id.clone(), id);
        }
    });

    // Mirror the active tab id into `selected_id` so that the existing
    // hash/localStorage persistence continues to work.
    use_effect(move || {
        let next = tabs.active.read().clone();
        if *selected_id.peek() != next {
            selected_id.set(next);
        }
    });

    // External hash-id changes (deep link / back-forward) → open the tab.
    use_effect(move || {
        if let Some(id) = selected_id.read().clone() {
            if tabs.active.peek().as_deref() != Some(id.as_str()) {
                let label = label_for(&specs.peek(), &id);
                tabs.open(id, label);
            }
        }
    });

    // Refresh tab labels once the spec list loads.
    use_effect(move || {
        let list = specs.read().clone();
        if list.is_empty() {
            return;
        }
        let snapshot = tabs.snapshot();
        let mut updated = false;
        let new_tabs: Vec<viewer_api_dioxus::Tab<String>> = snapshot
            .tabs
            .into_iter()
            .map(|t| {
                let label = label_for(&list, &t.id);
                if label != t.payload {
                    updated = true;
                }
                viewer_api_dioxus::Tab { id: t.id, payload: label }
            })
            .collect();
        if updated {
            tabs.set_tabs(new_tabs, snapshot.active);
        }
    });

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
                        Link {
                            to: Route::SpecGraphPage {},
                            class: "btn-nav-link",
                            "🌐 Graph"
                        }
                        button {
                            class: "btn-nav-link",
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
                        selected_id: tabs.active.read().clone(),
                        on_select: move |id: String| {
                            let label = label_for(&specs.peek(), &id);
                            tabs.open(id, label);
                            active_tab.set("body".to_string());
                        },
                    }
                }

            // ── Main content ──────────────────────────────────────────────
            div {
                class: "content",
                {
                    let tab_items: Vec<TabItem> = tabs.tabs.read().iter().map(|t| {
                        let mut item = TabItem::new(t.id.clone(), t.payload.clone());
                        item.closeable = true;
                        item
                    }).collect();
                    let active_tab_id = tabs.active.read().clone().unwrap_or_default();
                    rsx! {
                        if !tab_items.is_empty() {
                            TabBar {
                                tabs: tab_items,
                                active_id: active_tab_id,
                                on_select: move |id: String| {
                                    tabs.activate(&id);
                                    active_tab.set("body".to_string());
                                },
                                on_close: move |id: String| {
                                    tabs.close(&id);
                                },
                            }
                        }
                    }
                }
                if let Some(id) = tabs.active.read().clone() {
                    {
                        let summary = specs.read().iter().find(|s| s.id == id).cloned();
                        let component = summary
                            .as_ref()
                            .and_then(|s| s.component.clone());
                        let title = summary
                            .as_ref()
                            .and_then(|s| s.title.clone())
                            .unwrap_or_else(|| id.clone());
                        let mut crumbs: Vec<BreadcrumbItem> = vec![BreadcrumbItem::link(
                            "All specs",
                            EventHandler::new(move |_| {
                                state_filter.set(String::new());
                                let q = filter.peek().clone();
                                let q_opt = if q.is_empty() { None } else { Some(q) };
                                spawn_local(async move {
                                    if let Ok(resp) = api::list_specs(None, q_opt.as_deref(), None).await {
                                        specs.set(resp.items);
                                    }
                                });
                            }),
                        )];
                        if let Some(comp) = component {
                            let comp_label = comp.clone();
                            crumbs.push(BreadcrumbItem::link(
                                comp_label,
                                EventHandler::new(move |_| {
                                    filter.set(comp.clone());
                                    let q = comp.clone();
                                    spawn_local(async move {
                                        if let Ok(resp) = api::search_specs(&q, Some(50)).await {
                                            specs.set(resp.items);
                                        }
                                    });
                                }),
                            ));
                        }
                        crumbs.push(BreadcrumbItem::current(title));
                        rsx! {
                            Breadcrumbs {
                                items: crumbs,
                                class: "spec-detail__breadcrumbs".to_string(),
                            }
                        }
                    }
                    SpecDetail {
                        key: "{id}",
                        spec_id: id,
                        active_tab: active_tab.read().clone(),
                        on_tab_change: move |tab| active_tab.set(tab),
                    }
                } else {
                    div {
                        class: "empty-state",
                        "Select a specification to view details."
                    }
                }
            }
        }

        // ── Theme settings panel — floats above everything ──────────────────
        if *show_theme_settings.read() {
            div {
                class: "theme-settings-floating",
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
            class: "spec-tree-page",
            div {
                class: "spec-tree-page__header",
                h2 {
                    class: "spec-tree-page__title",
                    "Specification Tree"
                }
            }
            div {
                class: "spec-tree-page__list",
                if *loading.read() {
                    p { class: "spec-detail__loading", "Loading…" }
                } else {
                    for spec in specs.read().iter() {
                        {
                            let id = spec.id.clone();
                            let nav2 = nav.clone();
                            rsx! {
                                button {
                                    key: "{id}",
                                    class: "spec-card",
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
            class: "spec-detail-page",
            SpecDetail {
                spec_id: id,
                active_tab: active_tab.read().clone(),
                on_tab_change: move |tab| active_tab.set(tab),
            }
        }
    }
}

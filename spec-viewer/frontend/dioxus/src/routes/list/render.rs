use dioxus::prelude::*;
use viewer_api_dioxus::{
    BreadcrumbItem,
    Breadcrumbs,
    HamburgerIcon,
    Header,
    HeaderActions,
    Sidebar,
    TabBar,
    TabItem,
    TabsStore,
};
use wasm_bindgen_futures::spawn_local;

use crate::{
    api,
    components::{
        spec_detail::SpecDetail,
        spec_tree::SpecTree,
    },
    types::SpecSummary,
};

use super::{
    super::Route,
    helpers::{
        close_or_toggle_sidebar,
        initial_expanded_for,
        label_for,
        toggle_sidebar,
    },
};

pub(super) fn render_spec_list_header(
    sidebar_button_active: bool,
    sidebar_button_label: &'static str,
    sidebar_collapsed: Signal<bool>,
    mobile_sidebar_open: Signal<bool>,
    mut filter_panel_open: Signal<bool>,
    mut show_theme_settings: Signal<bool>,
    filter: Signal<String>,
    state_filter: Signal<String>,
    on_home: EventHandler<()>,
) -> Element {
    rsx! {
        Header {
            left: rsx! {
                button {
                    class: if sidebar_button_active { "btn btn-icon btn-active" } else { "btn btn-icon" },
                    aria_label: sidebar_button_label,
                    title: sidebar_button_label,
                    onclick: move |_| toggle_sidebar(sidebar_collapsed, mobile_sidebar_open),
                    HamburgerIcon {}
                }
                span { class: "header-icon", "📐" }
                span { class: "header-title", "Spec Viewer" }
            },
            right: rsx! {
                Link {
                    to: Route::SpecGraphPage {},
                    class: "btn-nav-link",
                    "🌐 Graph"
                }
                HeaderActions {
                    on_home: Some(on_home),
                    on_filter_toggle: Some(EventHandler::new(move |_| {
                        let next = !*filter_panel_open.read();
                        filter_panel_open.set(next);
                    })),
                    on_theme_toggle: Some(EventHandler::new(move |_| {
                        let next = !*show_theme_settings.read();
                        show_theme_settings.set(next);
                    })),
                    filter_active: *filter_panel_open.read(),
                    has_active_filters: !filter.read().trim().is_empty()
                        || !state_filter.read().is_empty(),
                }
            },
        }
    }
}

pub(super) fn render_spec_list_sidebar(
    sidebar_collapsed: Signal<bool>,
    mut mobile_sidebar_open: Signal<bool>,
    specs: Signal<Vec<SpecSummary>>,
    loading: Signal<bool>,
    list_error: Signal<Option<String>>,
    mut filter: Signal<String>,
    mut state_filter: Signal<String>,
    mut active_tab: Signal<String>,
    mut tabs: TabsStore<String>,
) -> Element {
    rsx! {
        Sidebar {
            title: "Specifications".to_string(),
            collapsed: *sidebar_collapsed.read(),
            on_toggle: move |_| close_or_toggle_sidebar(sidebar_collapsed, mobile_sidebar_open),
            mobile_open: Some(*mobile_sidebar_open.read()),
            on_mobile_open_change: move |open| mobile_sidebar_open.set(open),
            SpecTree {
                specs: specs.read().clone(),
                loading: *loading.read(),
                error: list_error.read().clone(),
                filter: filter.read().clone(),
                on_filter_change: move |value: String| filter.set(value),
                state_filter: state_filter.read().clone(),
                on_state_filter_change: move |value: String| state_filter.set(value),
                selected_id: tabs.active.read().clone(),
                initially_expanded: initial_expanded_for(&specs.read(), tabs.active.read().as_deref()),
                on_select: move |id: String| {
                    let label = label_for(&specs.peek(), &id);
                    tabs.open(id, label);
                    active_tab.set("body".to_string());
                    mobile_sidebar_open.set(false);
                },
            }
        }
    }
}

pub(super) fn render_spec_list_content(
    specs: Signal<Vec<SpecSummary>>,
    filter: Signal<String>,
    state_filter: Signal<String>,
    mut active_tab: Signal<String>,
    mut tabs: TabsStore<String>,
) -> Element {
    let tab_items: Vec<TabItem> = tabs
        .tabs
        .read()
        .iter()
        .map(|tab| {
            let mut item = TabItem::new(tab.id.clone(), tab.payload.clone());
            item.closeable = true;
            item
        })
        .collect();
    let active_tab_id = tabs.active.read().clone().unwrap_or_default();

    rsx! {
        div {
            class: "content",
            if !tab_items.is_empty() {
                TabBar {
                    tabs: tab_items,
                    active_id: active_tab_id,
                    on_select: move |id: String| {
                        tabs.activate(&id);
                        active_tab.set("body".to_string());
                    },
                    on_close: move |id: String| tabs.close(&id),
                }
            }
            if let Some(id) = tabs.active.read().clone() {
                {render_selected_spec(id, specs, filter, state_filter, active_tab)}
            } else {
                div {
                    class: "empty-state",
                    "Select a specification to view details."
                }
            }
        }
    }
}

fn render_selected_spec(
    id: String,
    specs: Signal<Vec<SpecSummary>>,
    filter: Signal<String>,
    state_filter: Signal<String>,
    mut active_tab: Signal<String>,
) -> Element {
    let crumbs = build_detail_breadcrumbs(
        &specs.read(),
        &id,
        specs,
        filter,
        state_filter,
    );

    rsx! {
        Breadcrumbs {
            items: crumbs,
            class: "spec-detail__breadcrumbs".to_string(),
        }
        SpecDetail {
            spec_id: id,
            active_tab: active_tab.read().clone(),
            on_tab_change: move |tab| active_tab.set(tab),
        }
    }
}

fn build_detail_breadcrumbs(
    specs_list: &[SpecSummary],
    id: &str,
    mut specs: Signal<Vec<SpecSummary>>,
    mut filter: Signal<String>,
    mut state_filter: Signal<String>,
) -> Vec<BreadcrumbItem> {
    let summary = specs_list.iter().find(|spec| spec.id == id).cloned();
    let component = summary.as_ref().and_then(|spec| spec.component.clone());
    let title = summary
        .as_ref()
        .and_then(|spec| spec.title.clone())
        .unwrap_or_else(|| id.to_string());

    let mut crumbs = vec![BreadcrumbItem::link(
        "All specs",
        EventHandler::new(move |_| {
            state_filter.set(String::new());
            let query = filter.peek().clone();
            let query = if query.is_empty() { None } else { Some(query) };
            spawn_local(async move {
                if let Ok(response) =
                    api::list_specs(None, query.as_deref(), None).await
                {
                    specs.set(response.items);
                }
            });
        }),
    )];

    if let Some(component) = component {
        let label = component.clone();
        crumbs.push(BreadcrumbItem::link(
            label,
            EventHandler::new(move |_| {
                filter.set(component.clone());
                let query = component.clone();
                spawn_local(async move {
                    if let Ok(response) =
                        api::search_specs(&query, Some(50)).await
                    {
                        specs.set(response.items);
                    }
                });
            }),
        ));
    }

    crumbs.push(BreadcrumbItem::current(title));
    crumbs
}

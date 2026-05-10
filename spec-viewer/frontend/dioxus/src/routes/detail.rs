use dioxus::prelude::*;
use viewer_api_dioxus::{
    BreadcrumbItem,
    Breadcrumbs,
    Header,
    HeaderActions,
    Layout,
    Overlay,
    ThemeSettings,
};

use crate::components::spec_detail::SpecDetail;

use super::Route;

#[component]
pub fn SpecDetailPage(
    id: String,
    view: Option<String>,
) -> Element {
    let mut show_theme_settings = use_signal(|| false);
    let navigation_store = use_context::<crate::store::SpecNavigationStore>();
    let nav = use_navigator();
    let title = id.clone();
    let active_tab = super::canonical_spec_view(view.as_deref()).to_string();

    let nav_specs = nav.clone();
    let nav_graph = nav.clone();
    let crumbs: Vec<BreadcrumbItem> = vec![
        BreadcrumbItem::link(
            "Specs",
            EventHandler::new(move |_| {
                nav_specs.push(Route::SpecListPage {});
            }),
        )
        .with_href("/specs"),
        BreadcrumbItem::link(
            "Graph",
            EventHandler::new(move |_| {
                nav_graph.push(Route::SpecGraphPage {});
            }),
        )
        .with_href("/specs/graph"),
        BreadcrumbItem::current(title.clone()),
    ];

    let nav_back = nav.clone();
    let nav_normalize = nav.clone();
    let id_for_normalize = id.clone();
    let route_view = view.clone();
    use_effect(use_reactive!(|id_for_normalize, route_view| {
        if !super::is_canonical_spec_detail_view(route_view.as_deref()) {
            nav_normalize.replace(Route::spec_detail_path(
                &id_for_normalize,
                route_view.as_deref(),
            ));
        }
    }));

    let id_for_view_memory = id.clone();
    let active_tab_for_view_memory = active_tab.clone();
    use_effect(use_reactive!(|id_for_view_memory, active_tab_for_view_memory| {
        navigation_store.remember_spec_view(
            &id_for_view_memory,
            &active_tab_for_view_memory,
        );
    }));

    let nav_tabs = nav;
    let id_for_tabs = id.clone();
    let active_tab_for_tabs = active_tab.clone();
    let navigation_store_for_tabs = navigation_store;
    rsx! {
        Layout {
            header: rsx! {
                Header {
                    left: rsx! {
                        button {
                            class: "btn-back",
                            "data-testid": "spec-detail-back",
                            aria_label: "Back",
                            onclick: move |_| nav_back.go_back(),
                            "\u{2190} Back"
                        }
                        Breadcrumbs {
                            items: crumbs,
                            class: "spec-detail__breadcrumbs".to_string(),
                        }
                    },
                    right: rsx! {
                        Link {
                            to: Route::SpecListPage {},
                            class: "btn-nav-link",
                            "\u{1F4D0} Specs"
                        }
                        Link {
                            to: Route::SpecGraphPage {},
                            class: "btn-nav-link",
                            "\u{1F310} Graph"
                        }
                        HeaderActions {
                            on_theme_toggle: Some(EventHandler::new(move |_| {
                                let next = !*show_theme_settings.read();
                                show_theme_settings.set(next);
                            })),
                        }
                    },
                }
            },
            div {
                class: "spec-detail-page",
                SpecDetail {
                    spec_id: id,
                    active_tab: active_tab.clone(),
                    on_tab_change: move |tab: String| {
                        if tab == active_tab_for_tabs {
                            return;
                        }
                        navigation_store_for_tabs.remember_spec_view(
                            &id_for_tabs,
                            &tab,
                        );
                        nav_tabs.push(Route::spec_detail_path(
                            &id_for_tabs,
                            Some(tab.as_str()),
                        ));
                    },
                }
            }
        }
        Overlay {
            open: *show_theme_settings.read(),
            on_close: move |_| show_theme_settings.set(false),
            panel_class: "theme-settings-modal".to_string(),
            aria_label: "Theme settings".to_string(),
            ThemeSettings {
                on_close: move |_| show_theme_settings.set(false),
            }
        }
    }
}

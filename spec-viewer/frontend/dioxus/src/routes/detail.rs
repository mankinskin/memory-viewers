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
pub fn SpecDetailPage(id: String) -> Element {
    let mut active_tab = use_signal(|| "body".to_string());
    let mut show_theme_settings = use_signal(|| false);
    let nav = use_navigator();
    let title = id.clone();

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

    let nav_back = nav;
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
                    active_tab: active_tab.read().clone(),
                    on_tab_change: move |tab| active_tab.set(tab),
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

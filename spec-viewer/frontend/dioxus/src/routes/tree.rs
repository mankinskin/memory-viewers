use std::collections::BTreeMap;

use dioxus::prelude::*;
use dioxus_router::Navigator;
use viewer_api_dioxus::{
    Card,
    CardGrid,
    CardSection,
    Header,
    HeaderActions,
    Layout,
    Overlay,
    ThemeSettings,
};
use wasm_bindgen_futures::spawn_local;

use crate::{
    api,
    types::SpecSummary,
};

use super::Route;

#[component]
pub fn SpecTreePage() -> Element {
    let mut specs: Signal<Vec<SpecSummary>> = use_signal(Vec::new);
    let mut loading: Signal<bool> = use_signal(|| true);
    let mut show_theme_settings = use_signal(|| false);
    let nav = use_navigator();

    use_effect(move || {
        spawn_local(async move {
            if let Ok(response) = api::list_specs(None, None, None).await {
                specs.set(response.items);
                loading.set(false);
            }
        });
    });

    rsx! {
        Layout {
            header: rsx! {
                Header {
                    left: rsx! {
                        button {
                            class: "btn-back",
                            aria_label: "Back",
                            onclick: move |_| nav.go_back(),
                            "\u{2190} Back"
                        }
                        span { class: "header-icon", "\u{1F333}" }
                        span { class: "header-title", "Specification Tree" }
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
                        {render_tree_sections(specs.read().clone(), nav)}
                    }
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

fn render_tree_sections(
    specs: Vec<SpecSummary>,
    nav: Navigator,
) -> Element {
    let mut grouped: BTreeMap<String, Vec<SpecSummary>> = BTreeMap::new();
    for spec in specs {
        let category = spec
            .component
            .clone()
            .unwrap_or_else(|| "Uncategorised".to_string());
        grouped.entry(category).or_default().push(spec);
    }

    rsx! {
        for (category, items) in grouped.into_iter() {
            CardSection {
                key: "{category}",
                title: category.clone(),
                count: Some(items.len()),
                CardGrid {
                    for spec in items {
                        {
                            let id = spec.id.clone();
                            let title = spec
                                .title
                                .clone()
                                .unwrap_or_else(|| "Untitled".to_string());
                            let description = spec.state.clone();
                            let nav_to_detail = nav.clone();
                            rsx! {
                                Card {
                                    key: "{spec.id}",
                                    title,
                                    description,
                                    on_click: EventHandler::new(move |_| {
                                        nav_to_detail.push(Route::SpecDetailPage { id: id.clone() });
                                    }),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

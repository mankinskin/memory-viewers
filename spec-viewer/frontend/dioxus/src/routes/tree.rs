use std::collections::BTreeMap;

use dioxus::prelude::*;
use dioxus_router::Navigator;
use viewer_api_dioxus::{Card, CardGrid, CardSection};
use wasm_bindgen_futures::spawn_local;

use crate::api;
use crate::types::SpecSummary;

use super::Route;

#[component]
pub fn SpecTreePage() -> Element {
    let mut specs: Signal<Vec<SpecSummary>> = use_signal(Vec::new);
    let mut loading: Signal<bool> = use_signal(|| true);
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
}

fn render_tree_sections(specs: Vec<SpecSummary>, nav: Navigator) -> Element {
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
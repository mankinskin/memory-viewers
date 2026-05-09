use dioxus::prelude::*;
use viewer_api_dioxus::{
    Header,
    Layout,
};

use crate::components::spec_graph::SpecGraphPage as SpecGraphView;

use super::Route;

#[component]
pub fn SpecGraphPage() -> Element {
    let nav = use_navigator();

    rsx! {
        Layout {
            class: "spec-graph-shell".to_string(),
            header: rsx! {
                Header {
                    left: rsx! {
                        button {
                            class: "btn-back",
                            "data-testid": "spec-graph-back",
                            aria_label: "Back",
                            onclick: move |_| nav.go_back(),
                            "\u{2190} Back"
                        }
                        span { class: "header-icon", "\u{1F310}" }
                        span { class: "header-title", "Spec Graph" }
                    },
                    right: rsx! {
                        Link {
                            to: Route::SpecListPage {},
                            class: "btn-nav-link",
                            "\u{1F4D0} Specs"
                        }
                    },
                }
            },
            div {
                class: "spec-graph-page",
                SpecGraphView {}
            }
        }
    }
}

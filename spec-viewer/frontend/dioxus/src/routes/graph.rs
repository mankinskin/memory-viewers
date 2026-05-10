use dioxus::prelude::*;
use viewer_api_dioxus::{
    Header,
    HeaderActions,
    Layout,
    Overlay,
    ThemeSettings,
};

use crate::components::spec_graph::SpecGraphPage as SpecGraphView;

use super::Route;

#[component]
pub fn SpecGraphPage() -> Element {
    let nav = use_navigator();
    let mut show_theme_settings = use_signal(|| false);

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
                class: "spec-graph-page",
                SpecGraphView {}
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

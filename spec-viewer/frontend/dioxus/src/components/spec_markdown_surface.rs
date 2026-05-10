use dioxus::prelude::*;
use viewer_api_dioxus::MarkdownContent;

#[component]
pub fn SpecMarkdownSurface(
    content: String,
    #[props(default)] class: String,
) -> Element {
    let surface_class = if class.is_empty() {
        "spec-markdown-surface".to_string()
    } else {
        format!("spec-markdown-surface {class}")
    };

    rsx! {
        div {
            class: "{surface_class}",
            MarkdownContent {
                content,
                class: "spec-markdown-surface__body".to_string(),
            }
        }
    }
}
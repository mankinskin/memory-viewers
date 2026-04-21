//! `SearchBar` — text input that triggers spec search.

use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct SearchBarProps {
    pub value: String,
    pub on_change: EventHandler<String>,
    #[props(default = "Search specs…".to_string())]
    pub placeholder: String,
}

#[component]
pub fn SearchBar(props: SearchBarProps) -> Element {
    rsx! {
        div {
            style: "display: flex; align-items: center; gap: 6px;",
            input {
                r#type: "text",
                value: "{props.value}",
                placeholder: "{props.placeholder}",
                oninput: move |e| props.on_change.call(e.value()),
                style: "
                    flex: 1;
                    background: #1a1a2e;
                    border: 1px solid #374151;
                    border-radius: 6px;
                    color: #e5e7eb;
                    font-size: 13px;
                    padding: 7px 10px;
                    outline: none;
                ",
            }
        }
    }
}

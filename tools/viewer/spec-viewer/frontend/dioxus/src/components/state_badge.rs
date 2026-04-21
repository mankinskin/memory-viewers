//! `StateBadge` — coloured pill that displays a spec state label.

use dioxus::prelude::*;
use crate::types::state_colors;

#[component]
pub fn StateBadge(state: String) -> Element {
    let (bg, fg) = state_colors(&state);
    rsx! {
        span {
            style: "
                display: inline-block;
                padding: 3px 10px;
                border-radius: 12px;
                font-size: 11px;
                font-weight: 600;
                letter-spacing: 0.04em;
                background: {bg};
                color: {fg};
                white-space: nowrap;
            ",
            "{state}"
        }
    }
}

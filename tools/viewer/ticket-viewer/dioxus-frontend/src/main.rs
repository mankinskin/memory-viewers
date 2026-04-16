mod backend;
mod components;
mod graph;
#[cfg(target_arch = "wasm32")]
mod graph3d;
mod routes;
mod store;
mod sse;

use dioxus::prelude::*;
use viewer_api_dioxus::{ViewerShell, WgpuOverlay};

use routes::Route;

fn main() {
    dioxus::launch(App);
}

/// Root application component for the ticket viewer.
///
/// Mounts the shared `ViewerShell` (WebGPU canvas + UI overlay) from
/// `viewer-api-dioxus` and nests the ticket-viewer SPA router inside the
/// overlay so all route components render on top of the GPU canvas layer.
#[component]
fn App() -> Element {
    rsx! {
        style { "html, body, #main {{ overflow: hidden; margin: 0; padding: 0; width: 100%; height: 100%; }}" }
        ViewerShell {
            WgpuOverlay {}
            Router::<Route> {}
        }
    }
}

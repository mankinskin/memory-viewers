mod api;
mod components;
mod routes;
mod sse;
mod store;
mod types;

use dioxus::prelude::*;
use viewer_api_dioxus::{ThemeProvider, ViewerShell, WgpuOverlay};

use routes::Route;

fn main() {
    #[cfg(target_arch = "wasm32")]
    viewer_api_dioxus::tracing_setup::install();
    dioxus::launch(App);
}

/// Root application component for the spec viewer.
///
/// Mounts the shared `ViewerShell` (WebGPU canvas + UI overlay) from
/// `viewer-api-dioxus` and nests the spec-viewer SPA router inside the
/// overlay so all route components render on top of the GPU canvas layer.
#[component]
fn App() -> Element {
    rsx! {
        style { "html, body, #main {{ overflow: hidden; margin: 0; padding: 0; width: 100%; height: 100%; }}" }
        ThemeProvider {
            ViewerShell {
                WgpuOverlay {}
                Router::<Route> {}
            }
        }
    }
}

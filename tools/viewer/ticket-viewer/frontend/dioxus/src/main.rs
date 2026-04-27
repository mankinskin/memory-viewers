mod api;
mod components;
#[cfg(target_arch = "wasm32")]
mod graph3d;
mod layout;
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

/// Root application component for the ticket viewer.
///
/// Mounts the shared `ViewerShell` (WebGPU canvas + UI overlay) from
/// `viewer-api-dioxus` and nests the ticket-viewer SPA router inside the
/// overlay so all route components render on top of the GPU canvas layer.
///
/// [`WgpuOverlay`] provides background effects (smoke, CRT, particles) when
/// the 3-D dependency graph is not visible.  When [`Graph3D`] mounts it calls
/// [`viewer_api_dioxus::set_gpu_canvas_owner`]`(true)`, causing `WgpuOverlay`
/// to idle its render loop and cede the canvas.  When `Graph3D` unmounts the
/// flag is cleared and `WgpuOverlay` resumes automatically.
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

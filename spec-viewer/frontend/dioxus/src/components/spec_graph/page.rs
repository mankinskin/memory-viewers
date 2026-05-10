use dioxus::prelude::*;
use viewer_api_dioxus::{
    graph3d::camera::frame_distance,
    Camera,
    CameraCommand,
    Layout3D,
};
use wasm_bindgen_futures::spawn_local;

use crate::{
    api,
    store::SpecGraphStore,
    types::SpecGraphNode,
};

use super::{
    cards::render_graph_node_cards,
    layouts::build_layout,
    model::LayoutAlgorithm,
    preview::SpecPreviewSidebar,
    settings::{
        queue_camera_command,
        render_graph_settings_panel,
    },
};

enum GraphPageState {
    Error(String),
    Loading(&'static str),
    Ready {
        nodes_raw: Vec<SpecGraphNode>,
        layout: Layout3D,
    },
}

const SETTINGS_PANEL_VIEWPORT_INSET_LEFT: f32 = 284.0;
const PREVIEW_VIEWPORT_INSET_RIGHT: f32 = 368.0;

#[derive(Clone, PartialEq)]
struct CameraFocusRequest {
    selection_key: Option<String>,
    target: [f32; 3],
    distance: f32,
}

#[component]
pub fn SpecGraphPage() -> Element {
    let mut store = use_context::<SpecGraphStore>();
    let navigation_store = use_context::<crate::store::SpecNavigationStore>();
    let mut camera_cmd: Signal<Option<CameraCommand>> = use_signal(|| None);
    let mut camera_seq: Signal<u64> = use_signal(|| 0);
    let mut last_cam_algo: Signal<LayoutAlgorithm> =
        use_hook(|| Signal::new(LayoutAlgorithm::ForceDirected));
    let mut last_focus_request: Signal<Option<CameraFocusRequest>> =
        use_hook(|| Signal::new(None));
    let mut preview_id: Signal<Option<String>> = use_signal(|| None);
    let hovered_id: Signal<Option<String>> = use_signal(|| None);
    let nav = use_navigator();

    use_graph_fetch(store);
    use_layout_sync(store);
    sync_camera_for_algorithm(store, last_cam_algo, camera_cmd, camera_seq);

    let (nodes_raw, layout) = match graph_page_state(store) {
        GraphPageState::Error(message) =>
            return render_status(
                "empty-state",
                Some("color: var(--error);"),
                &format!("Failed to load graph: {message}"),
            ),
        GraphPageState::Loading(message) =>
            return render_status("empty-state", None, message),
        GraphPageState::Ready { nodes_raw, layout } => (nodes_raw, layout),
    };

    let nodes = layout.nodes.clone();
    let node_count = nodes.len();
    let edge_count = layout.edges.len();
    let camera_command = *camera_cmd.read();
    let camera_command_seq = *camera_seq.read();
    let selected_node_id = preview_id.read().clone();
    let preview_open = selected_node_id.is_some();
    let selection_auto_layout = *store.auto_layout_selected_node.read();
    let viewport_insets =
        graph_viewport_insets(*store.panel_open.read(), preview_open);
    sync_camera_for_selected_node(
        store,
        &layout,
        selected_node_id.as_deref(),
        last_focus_request,
        camera_cmd,
        camera_seq,
    );
    let hovered_node_id = hovered_id.read().clone();

    rsx! {
        div { class: "graph-overlay",
            viewer_api_dioxus::Graph3D {
                layout: layout.clone(),
                initial_camera: store.current_camera.read().clone(),
                selected_node_id,
                hovered_node_id,
                selection_auto_layout,
                viewport_insets,
                container_id: "spec-graph3d-container".to_string(),
                container_style: "position: absolute; inset: 0; overflow: hidden; user-select: none; cursor: grab;".to_string(),
                camera_command,
                camera_command_seq,
                on_layout_change: Some(EventHandler::new(move |layout: Layout3D| {
                    store.current_layout.set(Some(layout));
                })),
                on_camera_change: Some(EventHandler::new(move |camera: Camera| {
                    store.current_camera.set(Some(camera));
                })),
                {render_graph_node_cards(&nodes, &nodes_raw, preview_id, hovered_id)}
                div {
                    class: "graph-controls-hint",
                    "Left-drag: orbit \u{00b7} Right-drag: pan \u{00b7} Scroll: zoom \u{00b7} Click card: open"
                }
                if node_count > 0 {
                    div {
                        class: "graph-count-badge",
                        "{node_count} specs \u{00b7} {edge_count} edges"
                    }
                }
                button {
                    class: "graph-settings-toggle",
                    "data-testid": "graph-settings-toggle",
                    "data-graph-passthrough": "false",
                    aria_label: "Toggle graph settings",
                    onclick: move |event: Event<MouseData>| {
                        event.stop_propagation();
                        let visible = *store.panel_open.read();
                        store.panel_open.set(!visible);
                    },
                    if *store.panel_open.read() { "\u{2715} Settings" } else { "\u{2699} Settings" }
                }
                if *store.panel_open.read() {
                    {render_graph_settings_panel(store, camera_cmd, camera_seq)}
                }
            }
            if let Some(spec_id) = preview_id.read().clone() {
                SpecPreviewSidebar {
                    spec_id: spec_id.clone(),
                    on_close: move |_| preview_id.set(None),
                    on_view_details: move |id: String| {
                        preview_id.set(None);
                        nav.push(navigation_store.resolve_spec_detail_path(&id));
                    },
                }
            }
        }
    }
}

fn use_graph_fetch(store: SpecGraphStore) {
    let mut raw = store.raw;
    let mut error = store.error;

    use_effect(move || {
        if raw.read().is_some() || error.read().is_some() {
            return;
        }

        spawn_local(async move {
            match api::get_graph().await {
                Ok(response) => {
                    error.set(None);
                    raw.set(Some((response.nodes, response.edges)));
                },
                Err(message) => error.set(Some(message)),
            }
        });
    });
}

fn use_layout_sync(store: SpecGraphStore) {
    let mut current_layout = store.current_layout;
    let mut applied_layout_generation = store.applied_layout_generation;

    use_effect(move || {
        let Some((nodes_raw, edges_raw)) = store.raw.read().clone() else {
            return;
        };

        let generation = *store.layout_generation.read();
        if current_layout.read().is_some()
            && generation == *applied_layout_generation.read()
        {
            return;
        }

        let edges_for_layout = if *store.committed_show_edges.read() {
            edges_raw.clone()
        } else {
            Vec::new()
        };
        let layout = build_layout(
            *store.committed_algo.read(),
            *store.committed_params.read(),
            &nodes_raw,
            &edges_for_layout,
        );
        current_layout.set(Some(layout));
        applied_layout_generation.set(generation);
    });
}

fn sync_camera_for_algorithm(
    store: SpecGraphStore,
    mut last_cam_algo: Signal<LayoutAlgorithm>,
    mut camera_cmd: Signal<Option<CameraCommand>>,
    mut camera_seq: Signal<u64>,
) {
    let current_algo = *store.committed_algo.read();
    if current_algo == *last_cam_algo.peek() {
        return;
    }

    last_cam_algo.set(current_algo);
    queue_camera_command(
        camera_cmd,
        camera_seq,
        current_algo.preferred_camera(),
    );
}

fn sync_camera_for_selected_node(
    store: SpecGraphStore,
    layout: &Layout3D,
    selected_node_id: Option<&str>,
    mut last_focus_request: Signal<Option<CameraFocusRequest>>,
    mut camera_cmd: Signal<Option<CameraCommand>>,
    mut camera_seq: Signal<u64>,
) {
    let center_camera_on_selected_node =
        *store.center_camera_on_selected_node.read();
    let zoom_to_selected_node = *store.zoom_to_selected_node.read();
    let selected_node_zoom_factor = *store.selected_node_zoom_factor.read();
    let current_camera = store.current_camera.read().clone();

    if !(center_camera_on_selected_node || zoom_to_selected_node) {
        if last_focus_request.peek().is_some() {
            last_focus_request.set(None);
        }
        return;
    }

    let next_request = if selected_node_id.is_some() {
        selection_camera_request(
            layout,
            selected_node_id,
            current_camera.as_ref(),
            center_camera_on_selected_node,
            zoom_to_selected_node,
            selected_node_zoom_factor,
        )
    } else if last_focus_request.peek().is_some() {
        selection_camera_request(
            layout,
            None,
            current_camera.as_ref(),
            center_camera_on_selected_node,
            zoom_to_selected_node,
            selected_node_zoom_factor,
        )
    } else {
        None
    };

    let Some(request) = next_request else {
        return;
    };

    if last_focus_request.peek().as_ref() == Some(&request) {
        return;
    }

    last_focus_request.set(Some(request.clone()));
    queue_camera_command(
        camera_cmd,
        camera_seq,
        CameraCommand::FocusOn {
            target: request.target,
            distance: request.distance,
        },
    );
}

fn selection_camera_request(
    layout: &Layout3D,
    selected_node_id: Option<&str>,
    current_camera: Option<&Camera>,
    center_camera_on_selected_node: bool,
    zoom_to_selected_node: bool,
    selected_node_zoom_factor: f32,
) -> Option<CameraFocusRequest> {
    let (_centre, radius) = layout.bounds();
    let framed_distance = frame_distance(radius);

    if let Some(selected_node_id) = selected_node_id {
        if !(center_camera_on_selected_node || zoom_to_selected_node) {
            return None;
        }

        let node = layout
            .nodes
            .iter()
            .find(|node| node.id == selected_node_id)?;
        let distance = if zoom_to_selected_node {
            (framed_distance / selected_node_zoom_factor.max(1.0).powi(2))
                .clamp(6.0, 120.0)
        } else {
            current_camera
                .map(|camera| camera.distance)
                .unwrap_or(framed_distance)
                .clamp(6.0, 120.0)
        };
        return Some(CameraFocusRequest {
            selection_key: Some(selected_node_id.to_string()),
            target: [node.x, node.y, node.z],
            distance,
        });
    }

    if layout.nodes.is_empty() {
        return None;
    }

    let (centre, radius) = layout.bounds();
    Some(CameraFocusRequest {
        selection_key: None,
        target: centre,
        distance: frame_distance(radius),
    })
}

fn graph_viewport_insets(
    panel_open: bool,
    preview_open: bool,
) -> [f32; 4] {
    [
        if panel_open {
            SETTINGS_PANEL_VIEWPORT_INSET_LEFT
        } else {
            0.0
        },
        0.0,
        if preview_open {
            PREVIEW_VIEWPORT_INSET_RIGHT
        } else {
            0.0
        },
        0.0,
    ]
}

fn graph_page_state(store: SpecGraphStore) -> GraphPageState {
    if let Some(message) = store.error.read().clone() {
        return GraphPageState::Error(message);
    }

    let Some((nodes_raw, _)) = store.raw.read().clone() else {
        return GraphPageState::Loading("Loading graph\u{2026}");
    };
    let Some(layout) = store.current_layout.read().clone() else {
        return GraphPageState::Loading("Preparing graph layout\u{2026}");
    };

    GraphPageState::Ready { nodes_raw, layout }
}

fn render_status(
    class: &str,
    style: Option<&str>,
    message: &str,
) -> Element {
    rsx! {
        div {
            class: "{class}",
            style: style.unwrap_or_default(),
            "{message}"
        }
    }
}

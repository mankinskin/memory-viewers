use dioxus::prelude::*;
use viewer_api_dioxus::CameraCommand;

use crate::store::SpecGraphStore;

use super::model::{LayoutAlgorithm, LayoutParams};

pub(super) fn render_graph_settings_panel(
    mut store: SpecGraphStore,
    mut camera_cmd: Signal<Option<CameraCommand>>,
    mut camera_seq: Signal<u64>,
) -> Element {
    let draft_algo = *store.draft_algo.read();
    let draft_params = *store.draft_params.read();
    let draft_show_edges = *store.draft_show_edges.read();
    let auto_apply = *store.auto_apply.read();
    let apply_disabled = auto_apply || !has_draft_changes(store, draft_algo, draft_params, draft_show_edges);

    rsx! {
        div {
            class: "graph-settings-panel",
            "data-testid": "graph-settings-panel",
            "data-graph-passthrough": "false",
            onclick: move |event: Event<MouseData>| event.stop_propagation(),
            onmousedown: move |event: Event<MouseData>| event.stop_propagation(),
            onwheel: move |event: Event<WheelData>| event.stop_propagation(),

            h3 { class: "graph-settings-panel__title", "Graph settings" }

            div { class: "graph-settings-section",
                label { class: "graph-settings-label graph-settings-label--inline",
                    input {
                        r#type: "checkbox",
                        "data-testid": "graph-toggle-auto-apply",
                        checked: auto_apply,
                        onchange: move |event| set_auto_apply(store, event.checked()),
                    }
                    " Auto-apply"
                }
            }

            div { class: "graph-settings-section",
                label { class: "graph-settings-label", "Layout algorithm" }
                select {
                    class: "graph-settings-select",
                    "data-testid": "graph-algo-select",
                    value: draft_algo.as_str(),
                    onchange: move |event| {
                        if let Some(algo) = LayoutAlgorithm::from_str_opt(&event.value()) {
                            set_draft_algorithm(store, algo);
                        }
                    },
                    for algo in LayoutAlgorithm::ALL.iter() {
                        option { value: algo.as_str(), "{algo.label()}" }
                    }
                }
            }

            div { class: "graph-settings-section",
                label { class: "graph-settings-label",
                    "Spread "
                    span { class: "graph-settings-value", "{draft_params.spread:.2}" }
                }
                input {
                    r#type: "range",
                    min: "0.2", max: "4.0", step: "0.05",
                    value: "{draft_params.spread}",
                    oninput: move |event| {
                        if let Ok(value) = event.value().parse::<f32>() {
                            set_draft_params(store, |params| params.spread = value);
                        }
                    },
                }
            }

            if matches!(draft_algo, LayoutAlgorithm::RingsByDepth | LayoutAlgorithm::Grid) {
                div { class: "graph-settings-section",
                    label { class: "graph-settings-label",
                        "Y spacing "
                        span { class: "graph-settings-value", "{draft_params.y_spacing:.2}" }
                    }
                    input {
                        r#type: "range",
                        min: "0.0", max: "4.0", step: "0.05",
                        value: "{draft_params.y_spacing}",
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<f32>() {
                                set_draft_params(store, |params| params.y_spacing = value);
                            }
                        },
                    }
                }
            }

            if matches!(draft_algo, LayoutAlgorithm::ForceDirected) {
                div { class: "graph-settings-section",
                    label { class: "graph-settings-label",
                        "Iterations "
                        span { class: "graph-settings-value", "{draft_params.iterations}" }
                    }
                    input {
                        r#type: "range",
                        min: "10", max: "400", step: "10",
                        value: "{draft_params.iterations}",
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<u32>() {
                                set_draft_params(store, |params| params.iterations = value);
                            }
                        },
                    }
                }
                div { class: "graph-settings-section",
                    label { class: "graph-settings-label",
                        "Link distance "
                        span { class: "graph-settings-value", "{draft_params.link_dist:.2}" }
                    }
                    input {
                        r#type: "range",
                        min: "0.5", max: "6.0", step: "0.1",
                        value: "{draft_params.link_dist}",
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<f32>() {
                                set_draft_params(store, |params| params.link_dist = value);
                            }
                        },
                    }
                }
                div { class: "graph-settings-section",
                    label { class: "graph-settings-label",
                        "Repulsion "
                        span { class: "graph-settings-value", "{draft_params.repulsion:.2}" }
                    }
                    input {
                        r#type: "range",
                        min: "0.5", max: "8.0", step: "0.1",
                        value: "{draft_params.repulsion}",
                        oninput: move |event| {
                            if let Ok(value) = event.value().parse::<f32>() {
                                set_draft_params(store, |params| params.repulsion = value);
                            }
                        },
                    }
                }
            }

            div { class: "graph-settings-section",
                label { class: "graph-settings-label graph-settings-label--inline",
                    input {
                        r#type: "checkbox",
                        "data-testid": "graph-toggle-edges",
                        checked: draft_show_edges,
                        onchange: move |event| set_draft_show_edges(store, event.checked()),
                    }
                    " Show edges"
                }
            }

            div { class: "graph-settings-section graph-settings-actions",
                button {
                    class: "graph-settings-apply",
                    "data-testid": "graph-settings-apply",
                    disabled: apply_disabled,
                    onclick: move |event: Event<MouseData>| {
                        event.stop_propagation();
                        commit_draft(store);
                    },
                    "Apply"
                }
                button {
                    class: "graph-settings-reset",
                    "data-testid": "graph-settings-reset",
                    onclick: move |event: Event<MouseData>| {
                        event.stop_propagation();
                        store.draft_params.set(LayoutParams::default());
                        if *store.auto_apply.read() {
                            commit_draft(store);
                        }
                    },
                    "Reset"
                }
            }

            div { class: "graph-settings-section",
                button {
                    class: "graph-settings-reset",
                    "data-testid": "graph-reset-camera",
                    onclick: move |event: Event<MouseData>| {
                        event.stop_propagation();
                        queue_camera_command(camera_cmd, camera_seq, (*store.committed_algo.read()).preferred_camera());
                    },
                    "Reset camera"
                }
            }
        }
    }
}

pub(super) fn queue_camera_command(
    mut camera_cmd: Signal<Option<CameraCommand>>,
    mut camera_seq: Signal<u64>,
    command: CameraCommand,
) {
    camera_cmd.set(Some(command));
    let next = *camera_seq.peek() + 1;
    camera_seq.set(next);
}

fn has_draft_changes(
    store: SpecGraphStore,
    draft_algo: LayoutAlgorithm,
    draft_params: LayoutParams,
    draft_show_edges: bool,
) -> bool {
    draft_algo != *store.committed_algo.read()
        || draft_params != *store.committed_params.read()
        || draft_show_edges != *store.committed_show_edges.read()
}

fn set_auto_apply(mut store: SpecGraphStore, enabled: bool) {
    store.auto_apply.set(enabled);
    if enabled {
        commit_draft(store);
    }
}

fn set_draft_algorithm(mut store: SpecGraphStore, algo: LayoutAlgorithm) {
    store.draft_algo.set(algo);
    if *store.auto_apply.read() {
        commit_draft(store);
    }
}

fn set_draft_show_edges(mut store: SpecGraphStore, enabled: bool) {
    store.draft_show_edges.set(enabled);
    if *store.auto_apply.read() {
        commit_draft(store);
    }
}

fn set_draft_params(mut store: SpecGraphStore, update: impl FnOnce(&mut LayoutParams)) {
    let mut params = *store.draft_params.read();
    update(&mut params);
    store.draft_params.set(params);
    if *store.auto_apply.read() {
        commit_draft(store);
    }
}

fn commit_draft(mut store: SpecGraphStore) {
    store.committed_algo.set(*store.draft_algo.read());
    store.committed_params.set(*store.draft_params.read());
    store.committed_show_edges.set(*store.draft_show_edges.read());
    store.mark_layout_dirty();
}
use dioxus::prelude::*;
use dioxus_router::Navigator;

use crate::{
    api::{
        HttpTicketBackend,
        TicketBackend,
    },
    layout::GraphLayout,
    types::TicketRef,
};

use super::{
    edge_list::{
        EdgeListSidebar,
        RemoveEdgeDialog,
    },
    interactions::select_node_or_navigate,
    picker::EdgePickerOverlay,
    state::{
        subscribe_sse,
        DepSseHandle,
        DragState,
        RemoveEdge,
    },
    viewport::GraphViewport,
    DepGraphProps,
};

#[component]
pub fn DepGraph(props: DepGraphProps) -> Element {
    let workspace = props.workspace.clone();
    let root_id = props.root_id.clone();
    let on_select = props.on_select.clone();
    let selected_node_id = props.selected_node_id.clone();
    let nav = use_navigator();

    #[cfg(target_arch = "wasm32")]
    if crate::graph3d::can_use_webgpu() {
        return render_webgpu_graph(
            nav,
            props,
            workspace,
            root_id,
            on_select,
            selected_node_id,
        );
    }

    let mut layout: Signal<Option<GraphLayout>> = use_signal(|| None);
    let mut load_error: Signal<Option<String>> = use_signal(|| None);
    let mut fetch_trigger: Signal<u32> = use_signal(|| 0_u32);
    let pan_x: Signal<f64> = use_signal(|| 0.0_f64);
    let pan_y: Signal<f64> = use_signal(|| 0.0_f64);
    let zoom: Signal<f64> = use_signal(|| 1.0_f64);
    let drag: Signal<Option<DragState>> = use_signal(|| None);
    let mut sse_handle: Signal<Option<DepSseHandle>> = use_signal(|| None);
    let picker_open: Signal<bool> = use_signal(|| false);
    let remove_confirm: Signal<Option<RemoveEdge>> = use_signal(|| None);

    {
        let workspace_fetch = workspace.clone();
        let root_fetch = root_id.clone();
        use_effect(move || {
            let _trigger = fetch_trigger();
            let workspace = workspace_fetch.clone();
            let root_id = root_fetch.clone();
            let mut layout = layout;
            let mut load_error = load_error;
            spawn(async move {
                let backend = HttpTicketBackend::new(None);
                match backend.get_subgraph(&workspace, &root_id, 4).await {
                    Ok(response) => {
                        let active_workspace =
                            if response.active_workspace.is_empty() {
                                workspace.clone()
                            } else {
                                response.active_workspace.clone()
                            };
                        layout.set(Some(GraphLayout::build(
                            &active_workspace,
                            response.nodes,
                            response.edges,
                        )));
                        load_error.set(None);
                    },
                    Err(error) => load_error.set(Some(error)),
                }
            });
        });
    }

    {
        let workspace_sse = workspace.clone();
        use_effect(move || {
            sse_handle.set(subscribe_sse(&workspace_sse, fetch_trigger));
        });
    }

    rsx! {
        GraphViewport {
            workspace: workspace.clone(),
            on_select: on_select.clone(),
            layout,
            load_error,
            fetch_trigger,
            pan_x,
            pan_y,
            zoom,
            drag,
            picker_open,
            EdgeListSidebar {
                layout,
                remove_confirm,
            }
            EdgePickerOverlay {
                workspace: workspace.clone(),
                root_id: root_id.clone(),
                open: picker_open,
                fetch_trigger,
            }
            RemoveEdgeDialog {
                workspace,
                remove_confirm,
                fetch_trigger,
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn render_webgpu_graph(
    nav: Navigator,
    props: DepGraphProps,
    workspace: String,
    root_id: String,
    on_select: Option<EventHandler<TicketRef>>,
    selected_node_id: Option<String>,
) -> Element {
    rsx! {
        crate::graph3d::Graph3D {
            key: "{root_id}",
            workspace: workspace.clone(),
            root_id: root_id.clone(),
            selected_node_id,
            layout_mode: props.layout_mode,
            projection: props.projection,
            on_layout_mode_change: props.on_layout_mode_change.clone(),
            on_projection_change: props.on_projection_change.clone(),
            on_select: move |ticket_ref: TicketRef| {
                select_node_or_navigate(
                    on_select.clone(),
                    nav.clone(),
                    ticket_ref,
                )
            }
        }
    }
}

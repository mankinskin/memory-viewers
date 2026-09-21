# Render Graph + Pipeline: Custom Nodes, Canvas Setup, and Mipmap Generation

## Problem

Bevy's render graph must host 7 custom nodes executing in sequence, plus canvas/WebGPU initialization and mipmap generation for frosted glass. This ticket wires the render graph — it does NOT implement shader logic (handled by T6a–T6d and T3a–T3b).

## Scope

### Render Graph Structure

```
Camera Extract → Buffer Swap → Particle Compute → Voxel Splat Kernel
  → AABB projection Project → Radix Sort → Tile Bin → Tiled Raster → UI Overlay
```

### Bevy Plugin

```rust
pub struct ContextEditorRenderPlugin;

impl Plugin for ContextEditorRenderPlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .add_render_graph_node::<BufferSwapNode>("buffer_swap")
            .add_render_graph_node::<ParticleComputeNode>("particle_compute")
            .add_render_graph_node::<VoxelSplatKernelNode>("voxel_splat_kernel")
            .add_render_graph_node::<SortKeyBuildNode>("sort_key_build")
            .add_render_graph_node::<RadixSortNode>("radix_sort")
            .add_render_graph_node::<TileBinNode>("tile_bin")
            .add_render_graph_node::<TiledRasterNode>("tiled_raster")
            .add_render_graph_edges(&[
                "buffer_swap", "particle_compute", "voxel_splat_kernel",
                "sort_key_build", "radix_sort", "tile_bin", "tiled_raster",
            ]);
    }
}
```

Each node is a stub that dispatches its compute/render pass. The actual shader logic is implemented by the specific pipeline tickets (T6a-d, T3a-b, T4).

### Canvas + WebGPU Setup

```rust
app.add_plugins(DefaultPlugins.set(WindowPlugin {
    primary_window: Some(Window {
        canvas: Some("#bevy-canvas".to_string()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    }),
    ..default()
}));
```

### Mipmap Generation

WebGPU has no auto-mipmap. A compute/blit chain generates mipmaps for the background capture texture:

```rust
fn generate_mipmaps(encoder: &mut CommandEncoder, texture: &Texture, mip_levels: u32) {
    for level in 1..mip_levels {
        // Blit from level-1 to level with bilinear downsample
    }
}
```

### Compute + Render Pipeline Cache

Pipeline descriptors for all compute and render passes, cached by Bevy's pipeline cache system. Shader compilation happens once; subsequent frames reuse compiled pipelines.

## Dependencies
- T2a (GPU buffer infra): All buffers and bind groups must exist before nodes execute
- T1 (scaffold): Bevy App, module layout

## Acceptance Criteria
1. Bevy render loop runs in WASM, drawing to the HTML canvas
2. Render graph executes 7 nodes in correct sequence
3. Canvas resizes with browser window (responsive)
4. Mipmap generation pass produces 10 levels for background texture
5. Pipeline cache compiles shaders once without recompilation
6. Each node stub runs without errors (dispatch logic added by downstream tickets)

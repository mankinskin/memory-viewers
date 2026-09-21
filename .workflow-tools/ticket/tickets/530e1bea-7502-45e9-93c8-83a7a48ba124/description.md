# Rendering: Bevy Render Graph, Double-Buffered SVO, and Voxel Splatting Pipeline

> **Coordinator ticket** — this ticket has been decomposed into focused sub-tickets.
> Implementation work happens in the children; this ticket tracks overall completion.
>
> **Sub-tickets:**
> - **T2a** GPU Buffer Infrastructure — `c6c150d9-eba5-4370-8283-c759bac302ef`
> - **T2b** Render Graph & Pipeline Assembly — `c7659e24-3687-4581-bc4c-54bfc7e19267`
>
> This ticket is done when both sub-tickets are closed.

---

## Problem

Bevy's render loop must host the full GPU pipeline: SVO buffer management with double buffering, procedural splat generation, AABB screen projection, GPU radix sort, and tiled forward+ rasterization. This ticket wires the render graph and creates all buffer infrastructure.

## Architecture: Multi-Pass Render Graph

### Render Graph Structure

```
┌───────────────────────────────────────────────────────────┐
│                  Bevy Render Graph                        │
│                                                           │
│  ┌──────────────────┐                                     │
│  │ Camera Extract   │ (built-in)                          │
│  └────────┬─────────┘                                     │
│           │                                               │
│  ┌────────▼─────────┐                                     │
│  │ Buffer Swap      │ (custom) swap double-buffered       │
│  │                  │ bind groups (front ↔ back)          │
│  └────────┬─────────┘                                     │
│           │                                               │
│  ┌────────▼─────────┐                                     │
│  │ Particle Compute │ (custom) SVO collision              │
│  └────────┬─────────┘                                     │
│           │                                               │
│  ┌────────▼─────────────────┐                             │
│  │ Voxel Splat Kernel       │ (compute) SVO → splats   │
│  │  per occupied voxel:     │                             │
│  │  emit N splats w/ packed material  │                             │
│  └────────┬─────────────────┘                             │
│           │                                               │
│  ┌────────▼─────────────────┐                             │
│  │ Sort Key Build           │ (compute) AABB → screen     │
│  │  8-corner clip-space →   │                             │
│  │  screen-space min/max    │                             │
│  │  + tile_id | depth key   │                             │
│  └────────┬─────────────────┘                             │
│           │                                               │
│  ┌────────▼─────────────────┐                             │
│  │ Radix Sort (×8 passes)   │ (compute) parallel sort     │
│  │  key = tile_id|depth     │                             │
│  │  histogram → scan →      │                             │
│  │  scatter                 │                             │
│  └────────┬─────────────────┘                             │
│           │                                               │
│  ┌────────▼─────────────────┐                             │
│  │ Tile Binning             │ (compute) per-tile          │
│  │  offset + count          │                             │
│  └────────┬─────────────────┘                             │
│           │                                               │
│  ┌────────▼─────────────────┐                             │
│  │ Tiled Rasterizer         │ (fragment) per-pixel:       │
│  │  glass SDF refraction →  │                             │
│  │  loop tile splats →   │                             │
│  │  front-to-back blend     │                             │
│  └────────┬─────────────────┘                             │
│           │                                               │
│  ┌────────▼──────────┐                                    │
│  │ UI Overlay         │ (custom) Dioxus text/cursor       │
│  └────────────────────┘                                   │
└───────────────────────────────────────────────────────────┘
```

### Double-Buffered SVO Buffers

```rust
#[derive(Resource)]
pub struct SvoDoubleBuffer {
    pub front: wgpu::Buffer,
    pub back: wgpu::Buffer,
    pub current_is_front: bool,
    pub capacity_nodes: usize,
}

impl SvoDoubleBuffer {
    pub fn new(device: &RenderDevice, capacity: usize) -> Self {
        let size = (capacity * std::mem::size_of::<OctreeNode>()) as u64;
        let usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
        Self {
            front: device.create_buffer(&BufferDescriptor { label: Some("svo_front"), size, usage, mapped_at_creation: false }),
            back: device.create_buffer(&BufferDescriptor { label: Some("svo_back"), size, usage, mapped_at_creation: false }),
            current_is_front: true,
            capacity_nodes: capacity,
        }
    }

    pub fn write_target(&self) -> &wgpu::Buffer {
        if self.current_is_front { &self.back } else { &self.front }
    }

    pub fn read_source(&self) -> &wgpu::Buffer {
        if self.current_is_front { &self.front } else { &self.back }
    }

    pub fn swap(&mut self) { self.current_is_front = !self.current_is_front; }
}
```

### Voxel Splatting Buffers

```rust
#[derive(Resource)]
pub struct SplatBuffers {
    pub splats: wgpu::Buffer,        // VoxelSplat[] from generator
    pub projected: wgpu::Buffer,        // ProjectedSplat[] from AABB projection
    pub sort_keys: wgpu::Buffer,        // u32[] (tile_id | depth)
    pub sort_values: wgpu::Buffer,      // u32[] (splat indices)
    pub sort_scratch: wgpu::Buffer,     // radix sort workspace
    pub histograms: wgpu::Buffer,       // per-workgroup histograms
    pub tile_data: wgpu::Buffer,        // TileData[] (offset, count per tile)
    pub splat_count: wgpu::Buffer,   // atomic counter
    pub max_splats: u32,
}
```

### Bind Group Layouts

```wgsl
// Group 0: SVO (reads FRONT buffer)
@group(0) @binding(0) var<storage, read> octree: array<OctreeNode>;
@group(0) @binding(1) var<uniform> camera: CameraUniforms;
@group(0) @binding(2) var<uniform> globals: GlobalUniforms;

// Group 1: splats (generator output → AABB projection input → sort input)
@group(1) @binding(0) var<storage, read_write> splats: array<VoxelSplat>;
@group(1) @binding(1) var<storage, read_write> projected: array<ProjectedSplat>;
@group(1) @binding(2) var<storage, read_write> sort_keys: array<u32>;

// Group 2: Tiles + glass
@group(2) @binding(0) var<storage, read> tile_data: array<TileData>;
@group(2) @binding(1) var<storage, read> sorted_instances: array<SortedInstance>;
@group(2) @binding(2) var<storage, read> glass_panels: array<GlassPanel>;

// Group 3: Mipmap background texture (for frosted glass)
@group(3) @binding(0) var bg_tex: texture_2d<f32>;
@group(3) @binding(1) var bg_sampler: sampler;
```

### Double-Buffered Bind Groups

Both front and back bind groups are pre-built. On swap, the render graph simply picks the other set — no per-frame bind group creation.

```rust
pub struct DoubleBindGroups {
    pub front_group: wgpu::BindGroup, // SVO front buffer
    pub back_group: wgpu::BindGroup,  // SVO back buffer
}

impl DoubleBindGroups {
    pub fn active(&self, current_is_front: bool) -> &wgpu::BindGroup {
        if current_is_front { &self.front_group } else { &self.back_group }
    }
}
```

### Canvas and WebGPU Setup

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

WebGPU has no auto-mipmap. A small compute pass (or blit chain) generates mipmaps for the background capture texture used by frosted glass:

```rust
fn generate_mipmaps(encoder: &mut CommandEncoder, texture: &Texture, mip_levels: u32) {
    for level in 1..mip_levels {
        // Blit from level-1 to level with bilinear downsample
    }
}
```

### Bevy Plugin

```rust
pub struct ContextEditorRenderPlugin;

impl Plugin for ContextEditorRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SvoDoubleBuffer>()
           .init_resource::<SplatBuffers>()
           .init_resource::<GlassPanelBuffer>()
           .init_resource::<ParticleBuffer>();

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

## Dependencies
- T1 (scaffold): Bevy App skeleton, Cargo.toml, module layout

## Acceptance Criteria
1. Bevy render loop runs in WASM, drawing to the HTML canvas
2. SVO double buffer created with front/back pair; swap works without stalls
3. Render graph executes full pipeline: swap → particles → gen → AABB projection → sort → bin → raster
4. splat storage buffer created with configurable max capacity
5. Radix sort buffers (keys, values, scratch, histograms) allocated
6. Tile data buffer sized for screen resolution / 16×16
7. Mipmap generation compute pass produces 10 levels for background texture
8. Canvas resizes with browser window (responsive)
9. Pre-built double bind groups swap without per-frame allocation
10. No WebGPU validation errors in browser console

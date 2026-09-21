# Sort Key Construction & Tiled Depth Ordering for Voxel Splats

## Problem

The second rendering stage: project each `VoxelSplat`'s bounding box to screen-space, compute tile membership, and construct composite sort keys `(tile_id << 12) | depth` for the GPU radix sort (T6c). This replaces the old EWA covariance projection and SH evaluation — voxel splats are axis-aligned boxes, so projection is a simple AABB-to-screen transform.

## Scope

### ProjectedSplat Struct

```wgsl
struct ProjectedSplat {
    screen_min: vec2f,      // screen-space AABB min (pixels)
    screen_max: vec2f,      // screen-space AABB max (pixels)
    center_ws: vec3f,       // world-space center (for ray-box in T6d)
    half_extent: f32,       // world-space half-size
    depth: f32,             // view-space Z for sorting
    material_packed: u32,   // passthrough from VoxelSplat
    _pad: vec2u,
}
```

### Compute Shader: sort_key_build.wgsl

```wgsl
@group(0) @binding(0) var<storage, read> splats: array<VoxelSplat>;
@group(0) @binding(1) var<storage, read_write> projected: array<ProjectedSplat>;
@group(0) @binding(2) var<storage, read_write> sort_keys: array<u32>;
@group(0) @binding(3) var<storage, read_write> sort_values: array<u32>;
@group(0) @binding(4) var<uniform> camera: CameraUniforms;

struct CameraUniforms {
    view_proj: mat4x4f,
    view_mat: mat4x4f,
    camera_pos: vec3f,
    resolution: vec2f,
    max_depth: f32,
}

const TILE_SIZE: u32 = 16u;

@compute @workgroup_size(256)
fn build_sort_keys(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    if idx >= splat_count_val { return; }
    let s = splats[idx];

    // Transform voxel AABB corners to clip-space, find screen-space bounding rect
    let half = vec3f(s.half_extent);
    let corners = array<vec3f, 8>(
        s.center_ws + vec3f(-1,-1,-1) * half,
        s.center_ws + vec3f( 1,-1,-1) * half,
        s.center_ws + vec3f(-1, 1,-1) * half,
        s.center_ws + vec3f( 1, 1,-1) * half,
        s.center_ws + vec3f(-1,-1, 1) * half,
        s.center_ws + vec3f( 1,-1, 1) * half,
        s.center_ws + vec3f(-1, 1, 1) * half,
        s.center_ws + vec3f( 1, 1, 1) * half,
    );

    var screen_min = vec2f(1e9);
    var screen_max = vec2f(-1e9);
    var min_depth = 1e9f;
    var all_behind = true;

    for (var c = 0u; c < 8u; c++) {
        let clip = camera.view_proj * vec4f(corners[c], 1.0);
        if clip.w <= 0.0 { continue; }
        all_behind = false;
        let ndc = clip.xyz / clip.w;
        let screen = (ndc.xy * vec2f(0.5, -0.5) + 0.5) * camera.resolution;
        screen_min = min(screen_min, screen);
        screen_max = max(screen_max, screen);
        min_depth = min(min_depth, clip.w);
    }

    if all_behind { return; }  // entirely behind camera

    // Frustum cull: if AABB is entirely off-screen
    if screen_max.x < 0.0 || screen_min.x > camera.resolution.x ||
       screen_max.y < 0.0 || screen_min.y > camera.resolution.y {
        return;
    }

    // Clamp to screen
    screen_min = clamp(screen_min, vec2f(0.0), camera.resolution);
    screen_max = clamp(screen_max, vec2f(0.0), camera.resolution);

    // View-space depth for center
    let pos_view = (camera.view_mat * vec4f(s.center_ws, 1.0)).xyz;

    // Store projected splat
    projected[idx] = ProjectedSplat(
        screen_min, screen_max,
        s.center_ws, s.half_extent,
        pos_view.z, s.material_packed,
        vec2u(0u),
    );

    // Sort key: tile_id of center (20 bits) | depth (12 bits)
    let center_screen = (screen_min + screen_max) * 0.5;
    let tile_x = u32(center_screen.x) / TILE_SIZE;
    let tile_y = u32(center_screen.y) / TILE_SIZE;
    let grid_width = (u32(camera.resolution.x) + TILE_SIZE - 1u) / TILE_SIZE;
    let tile_id = tile_y * grid_width + tile_x;
    let depth_quantized = u32(clamp(pos_view.z / camera.max_depth * 4095.0, 0.0, 4095.0));

    sort_keys[idx] = (tile_id << 12u) | depth_quantized;
    sort_values[idx] = idx;
}
```

### Key Differences from Old EWA Approach

| Old (Gaussian) | New (Voxel Splat) |
|----------------|-------------------|
| 3D covariance → 2D via Jacobian matrix | 8-corner AABB → screen rect |
| SH evaluation (48 floats per Gaussian) | Material packed in 1× u32 — PBR in fragment |
| Isotropic blur radius | Exact axis-aligned bounding box |
| `exp(-0.5 * d²/σ²)` falloff | `sd_box` + `smoothstep` (in T6d) |

This is significantly cheaper per-splat: no matrix multiplications, no SH basis function evaluation, no covariance inversion. The PBR evaluation moves to the fragment shader (T6d+T6e) where it only runs for visible pixels.

### Bevy Render Node

```rust
pub struct SortKeyBuildNode;
impl Node for SortKeyBuildNode {
    fn run(&self, graph: &mut RenderGraphContext, world: &World) -> Result<(), NodeRunError> {
        // Dispatch compute: ceil(splat_count / 256) workgroups
        // Input: splats[] from VoxelSplatKernelNode
        // Output: projected[], sort_keys[], sort_values[]
        Ok(())
    }
}
```

### Multi-Tile Splats

Large voxels near the camera can span multiple tiles. For v1, each splat is assigned to one tile (its center tile). This means large splats may be clipped at tile edges. Future optimization: emit one sort entry per overlapped tile (fan-out), but this requires an additional atomic append. Deferring to a follow-up ticket.

## Implementation Plan

1. Define `ProjectedSplat` struct in `kernel/src/render/splat_types.rs`
2. Create `sort_key_build.wgsl` compute shader
3. Implement `SortKeyBuildNode` replacing the current `EwaProject` stub
4. Create bind group: `[splats, projected, sort_keys, sort_values, camera_uniform]`
5. Wire `CameraUniforms` extraction from Bevy camera
6. Unit test: verify sort keys encode correct tile_id for known screen positions

## Dependencies
- T6a (voxel splat kernel): `VoxelSplat[]` and `splat_count` input
- T2a (GPU buffer infra): `projected[]`, `sort_keys[]`, `sort_values[]` buffers

## Acceptance Criteria
1. AABB projection produces tight screen-space bounding boxes for each voxel splat
2. Frustum culling discards fully off-screen and behind-camera splats
3. Sort keys encode `tile_id` (20 bits) and `depth` (12 bits) correctly
4. Projection completes in < 0.3ms for 1M splats (faster than old EWA — no matrix math)
5. `ProjectedSplat` carries world-space center + half_extent for ray-box SDF in T6d
6. Material `u32` is passed through unchanged for PBR evaluation in T6d+T6e

# Voxel Splat Kernel: Ray-Box SDF Splatting with Screen-Space EWA Filtering and LOD Blend

## Problem

The first rendering stage: each occupied SVO leaf is projected to screen as a **voxel splat**. Instead of emitting 3D Gaussians with SH, each splat is evaluated analytically in screen-space using a **ray-box SDF** distance function with `smoothstep` soft edges, then filtered with screen-space **EWA** derivatives (`fwidth`) to prevent aliasing. PBR material is packed in a compact `u32`.

This replaces the old Gaussian generation approach entirely — no `GaussianData` struct, no SH coefficients, no isotropic covariance. The SDF kernel produces crisp voxel edges at close range that soften naturally at distance.

## Scope

### VoxelSplat Struct

```wgsl
struct VoxelSplat {
    center_ws: vec3f,       // world-space voxel center
    half_extent: f32,       // half-size of axis-aligned box
    material_packed: u32,   // R8 G8 B5 roughness3 metallic1 (see T6e)
    _pad: u32,
}
```

Each leaf voxel produces exactly one `VoxelSplat`. The material is unpacked per-pixel in the rasterizer (T6d) for Cook-Torrance/GGX evaluation (T6e).

### Compute Shader: voxel_splat_kernel.wgsl

```wgsl
@group(0) @binding(0) var<storage, read> octree: array<OctreeNode>;
@group(0) @binding(1) var<storage, read_write> splats: array<VoxelSplat>;
@group(0) @binding(2) var<storage, read_write> splat_count: atomic<u32>;
@group(0) @binding(3) var<uniform> params: SplatParams;

struct SplatParams {
    camera_pos: vec3f,
    total_nodes: u32,
    lod_scale: f32,
    max_depth: u32,
    world_size: f32,
    _pad: f32,
}

@compute @workgroup_size(256)
fn generate_splats(@builtin(global_invocation_id) id: vec3u) {
    let node_idx = id.x;
    if node_idx >= params.total_nodes { return; }
    let node = octree[node_idx];
    let child_mask = node.child_pointer & 0xFFu;
    if child_mask != 0u { return; }  // skip internal nodes

    let pos = node_position(node_idx, params.max_depth, params.world_size);
    let half = voxel_half_extent(node_idx, params.max_depth, params.world_size);

    // LOD culling: skip tiny splats that project below 1 pixel
    let cam_dist = length(pos - params.camera_pos);
    let screen_size = half / max(cam_dist, 0.001);
    if screen_size < params.lod_scale { return; }

    let si = atomicAdd(&splat_count, 1u);
    splats[si] = VoxelSplat(pos, half, node.color_data, 0u);
}

fn node_position(idx: u32, max_depth: u32, world_size: f32) -> vec3f {
    // Reconstruct 3D position from Morton-coded index path through octree
    // Each depth level encodes 3 bits (xyz child octant)
    // ...implementation detail...
    return vec3f(0.0);  // placeholder
}

fn voxel_half_extent(idx: u32, max_depth: u32, world_size: f32) -> f32 {
    let depth = node_depth(idx, max_depth);
    return world_size / f32(1u << (depth + 1u));
}
```

### Ray-Box SDF (used in T6d rasterizer)

The kernel's output is consumed by the tiled rasterizer. Per-pixel SDF evaluation:

```wgsl
fn sd_box(p: vec3f, half_ext: vec3f) -> f32 {
    let q = abs(p) - half_ext;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}
```

### EWA Screen-Space Filtering (applied per-pixel)

```wgsl
// In fragment shader (T6d), anti-alias the SDF edge:
let d = sd_box(local_pos, vec3f(half_extent));
let fw = fwidth(d);  // screen-space derivative
let alpha = 1.0 - smoothstep(-fw, fw, d);
```

This produces:
- **Near camera**: crisp, hard voxel edges (`fw` is small relative to `d` gradient)
- **Far away**: soft, blended edges (`fw` approaches voxel size → natural LOD)

### LOD Blend Strategy

| Distance | Behavior | Visual |
|----------|----------|--------|
| Near (< 5 voxels) | Leaf splats, sharp `smoothstep` | Crisp cubes |
| Mid (5–50 voxels) | Leaf splats, `fwidth` softens edges | Soft cubes |
| Far (> 50 voxels) | Sub-pixel splats culled by LOD | Clean fade-out |

Unlike Gaussian LOD which required multi-level traversal to generate coarser Gaussians, the SDF approach gets automatic LOD from `fwidth` — no extra passes needed.

### Bevy Render Node

```rust
pub struct VoxelSplatKernelNode;
impl Node for VoxelSplatKernelNode {
    fn run(&self, graph: &mut RenderGraphContext, world: &World) -> Result<(), NodeRunError> {
        // 1. Reset splat_count atomic to 0
        // 2. Dispatch compute: ceil(total_nodes / 256) workgroups
        // Output: splats[] + splat_count ready for sort key build (T6b)
        Ok(())
    }
}
```

## Implementation Plan

1. Define `VoxelSplat` struct in `kernel/src/render/splat_types.rs` (shared between Rust and WGSL via `include_wgsl!`)
2. Create `voxel_splat_kernel.wgsl` compute shader
3. Implement `VoxelSplatKernelNode` replacing the current `GaussianGen` stub
4. Create bind group layout: `[octree_buf, splat_buf, splat_count_buf, params_uniform]`
5. Wire `SplatParams` uniform extraction from camera + SVO state
6. Unit test: verify splat count equals leaf-node count for a known SVO

## Dependencies
- T2a (GPU buffer infra): `SplatBuffers` with `splats: Buffer`, `splat_count: Buffer` (atomic)
- T2b (render graph): `VoxelSplatKernel` node slot (already stubbed as `ContextEditorLabel::VoxelSplatKernel`)

## Acceptance Criteria
1. Compute shader emits one `VoxelSplat` per occupied leaf voxel
2. LOD culling: splats projecting below ~1 pixel are discarded
3. `color_data` from `OctreeNode` is passed through as `material_packed` (u32)
4. Atomic counter tracks splat count accurately
5. Non-leaf nodes (with children) are skipped
6. Generation completes in < 0.5ms for a 256³ voxel world (cheaper than old Gaussian gen — no SH computation)
7. `sd_box` + `fwidth` + `smoothstep` produce visually correct anti-aliased voxel edges (verified in T6d integration)

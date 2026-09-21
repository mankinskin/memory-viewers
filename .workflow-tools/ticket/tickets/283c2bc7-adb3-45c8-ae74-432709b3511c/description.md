# Phase 5: Z-Prepass Depth Buffer for Early-Z Voxel Culling

## Problem

The tiled rasterizer currently has `depth_stencil: None` in its `RenderPipelineDescriptor` and
performs no depth testing whatsoever. Every pixel composites all splats in its tile front-to-back
using `remaining_alpha` accumulation with a `smoothstep(-fw, fw, d)` coverage weight.

This causes two visible artifacts:

### 1. Z-Fighting
Adjacent SVO surface voxels at nearly identical depths all contribute fractional alpha to the same
pixel. Because `fw = hit_dist / resolution.y ≈ 0.001`, even a correct ray hit on one voxel surface
has neighboring voxels contribute small but non-zero alpha. The result looks like shimmering or
translucent overlap between terrain tiles — classic z-fighting.

### 2. Semi-Transparent Terrain
For the same reason, solid terrain voxels (grass, stone, sand) appear partially transparent. The
`smoothstep` fringe is only guaranteed to give `alpha ≈ 1.0` when the SDF evaluates to a strongly
negative value at the ray-surface hit point. In practice, at LOD depth 4 (~1 unit voxels), a ray
hitting the face of a box at a slight angle evaluates to `d ≈ 0`, giving `alpha ≈ 0.5 * remaining_alpha`
instead of `alpha ≈ 1.0`. Stacked adjacent voxels compound this, making terrain look ghostly.

## Current Architecture

```
VoxelSplatKernel → SortKeyBuild → RadixSort → TileBin → TiledRaster
                                                              ↑
                                                    depth_stencil: None
                                                    alpha-blend only
```

The `TiledRaster` render node runs a single fullscreen-triangle pass. Splats are pre-sorted
front-to-back by the radix sort, but there is no GPU depth buffer to short-circuit evaluation.

## Solution

### Part A — Opacity Fix (immediate, low risk)

For box-type splats (`sdf_type == 0`, which is all SVO voxels), replace the soft smoothstep with a
hard step once the SDF value is clearly inside the surface:

```wgsl
// Current (causes semi-transparency):
let alpha = (1.0 - smoothstep(-fw, fw, d)) * remaining_alpha;

// Proposed: hard opaque for box voxels, smooth AA only at the fringe
var alpha: f32;
if sdf_tp == 0u && d < -fw {
    // Clearly inside a box voxel — fully opaque, consume all remaining alpha
    alpha = remaining_alpha;
} else {
    alpha = (1.0 - smoothstep(-fw, fw, d)) * remaining_alpha;
}
```

This keeps analytical anti-aliasing at voxel edges but makes interior hits fully opaque,
eliminating the translucent terrain appearance.

### Part B — Z-Prepass (depth testing, eliminates z-fighting)

Add a two-pass structure inside the `TiledRasterNode`:

**Pass 1 — Depth prepass (new compute shader `z_prepass.wgsl`):**
- For each pixel, scan the first `N ≤ 8` front-to-back splats in the tile's active list.
- For each splat, evaluate a cheap box SDF (regardless of actual `sdf_type`).
- If the hit gives `alpha > 0.95`, write the view-space depth to a `depth_prepass` storage buffer.
- Stop at the first opaque hit — no PBR, no accumulation.

Output: `depth_prepass: array<f32>` — one f32 per pixel, initialized to `f32::MAX`.

**Pass 2 — PBR pass (existing `tiled_raster.wgsl`, modified):**
- At the start of the splat loop, read `depth_prepass[px_idx]`.
- Skip any splat whose `hit_dist > depth_prepass[px_idx] + epsilon`.
- This discards all splats behind the already-found opaque surface — eliminating z-fighting.

> **Why not a real depth buffer?** WebGPU does not support writing `@builtin(frag_depth)` from a
> fragment shader and then using that depth for early-Z testing in the same or a later pass without
> significant pipeline restructuring. Our fullscreen-triangle approach reads splat data from storage
> buffers, not from rasterized geometry, so there is no natural depth attachment. A storage-buffer
> depth prepass is the correct solution for this architecture.

## Implementation Plan

### Step 1: Opacity fix in `tiled_raster.wgsl`
- Replace the `smoothstep` line with the conditional hard-step for `sdf_tp == 0u`.
- No Rust changes needed.
- Immediately fixes semi-transparent terrain.
- **Test:** reload browser, confirm grass/stone/sand render fully opaque.

### Step 2: Add `depth_prepass` buffer

In `kernel/src/render/tiled_raster.rs`:
- Add a `depth_prepass: Buffer` field to `SplatBuffers` (or create a new resource).
- Size: `max_width * max_height * 4` bytes (f32 per pixel). At 1920×1080 that is ~8 MB.
- Usage: `STORAGE | COPY_DST` (the CPU or a clear pass resets it each frame).
- Add binding slot 6 to the raster bind group layout (`read_write` storage, visibility `COMPUTE | FRAGMENT`).

### Step 3: `z_prepass.wgsl` compute shader

New file `kernel/src/render/z_prepass.wgsl`:

```wgsl
// One thread per pixel.
// For each pixel, iterate the first min(tile_count, MAX_PREPASS_SPLATS)
// sorted splats. Evaluate cheap box SDF. Write view-space depth at first
// opaque hit (alpha > 0.95), then stop.

const MAX_PREPASS_SPLATS: u32 = 8u;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3u) {
    let px = vec2f(f32(gid.x) + 0.5, f32(gid.y) + 0.5);
    let px_idx = gid.y * u32(uniforms.resolution.x) + gid.x;

    if gid.x >= u32(uniforms.resolution.x) || gid.y >= u32(uniforms.resolution.y) {
        return;
    }

    // Reconstruct ray (same as tiled_raster.wgsl)
    let ndc = px / uniforms.resolution * 2.0 - 1.0;
    let clip_0 = vec4f(ndc.x, -ndc.y, 1.0, 1.0);
    let clip_1 = vec4f(ndc.x, -ndc.y, 0.5, 1.0);
    let w0 = uniforms.inv_view_proj * clip_0;
    let w1 = uniforms.inv_view_proj * clip_1;
    let ray_origin = uniforms.camera_pos;
    let ray_dir = normalize(w1.xyz / w1.w - w0.xyz / w0.w);

    // Tile lookup
    let tile_x   = gid.x / TILE_SIZE;
    let tile_y   = gid.y / TILE_SIZE;
    let tile_idx = tile_y * uniforms.grid_width + tile_x;
    let tile_val = tile_data[tile_idx];
    let tile_offset = tile_val >> 12u;
    let tile_count  = tile_val & 0xFFFu;

    let limit = min(tile_count, MAX_PREPASS_SPLATS);
    for (var i = 0u; i < limit; i++) {
        let splat_idx  = active_list[tile_offset + i];
        let s          = projected[splat_idx];
        let center_ws  = s.center_and_extent.xyz;
        let half_ext   = s.center_and_extent.w;

        // Cheap box hit — ignore sdf_type
        let local_pos = ray_box_closest_point(ray_origin, ray_dir, center_ws, half_ext);
        let p_local   = local_pos - center_ws;
        let d         = sd_box(p_local, vec3f(half_ext));
        let hit_dist  = length(local_pos - ray_origin);
        let fw        = hit_dist / uniforms.resolution.y;
        let alpha     = 1.0 - smoothstep(-fw, fw, d);

        if alpha > 0.95 {
            depth_prepass[px_idx] = hit_dist;
            return;
        }
    }
    // No opaque hit in top-N splats — leave depth_prepass[px_idx] unchanged (MAX)
}
```

### Step 4: Rust — `ZPrepassNode` render graph node

New struct `ZPrepassNode` in `tiled_raster.rs` (or a new file `z_prepass.rs`):
- Dispatch dimensions: `ceil(width / 8) × ceil(height / 8)`.
- Clear `depth_prepass` buffer to `f32::MAX` bits (`0x7F7FFFFF`) each frame via `clear_buffer` or a
  small fill compute pass.
- Must run **after** `TileBinNode` and **before** `TiledRasterNode` in the render graph.

Render graph order:
```
VoxelSplatKernel → SortKeyBuild → RadixSort → TileBin → ZPrepass → TiledRaster
```

### Step 5: Modify `tiled_raster.wgsl` — depth cull

At the start of the splat loop in `fs_main`:

```wgsl
let px_idx = u32(px.y) * u32(uniforms.resolution.x) + u32(px.x);
let prepass_depth = depth_prepass[px_idx];

// ... inside the splat loop, after hit_dist is computed:
if hit_dist > prepass_depth + 0.05 {
    continue; // behind the opaque surface found in prepass
}
```

### Step 6: Clear pass

Each frame, before `ZPrepassNode` runs, reset `depth_prepass` to `f32::MAX`.
Options:
- `render_context.command_encoder().clear_buffer(&depth_prepass_buffer, 0, None)` — fills with
  zeros (not MAX). Requires a tiny fill-with-MAX compute shader, or use `0x7F800000` (infinity).
- Alternatively, initialize to `0x7F800000` (f32 positive infinity) in the prepass shader itself
  using an init pass at `@workgroup_size(256)`.

Use a one-dispatch clear compute shader for simplicity: write `bitcast<f32>(0x7F800000u)` to every
slot. Run this as the first step in `ZPrepassNode::run()`.

## Files Changed

| File | Change |
|------|--------|
| `kernel/src/render/tiled_raster.wgsl` | Opacity fix; depth cull in `fs_main`; add `depth_prepass` binding |
| `kernel/src/render/tiled_raster.rs` | Add `depth_prepass` buffer; add binding 6 to layout; add `ZPrepassNode`; wire render graph |
| `kernel/src/render/z_prepass.wgsl` | New compute shader |

## Acceptance Criteria

1. Grass, stone, and sand terrain voxels render fully opaque (no see-through).
2. Z-fighting between adjacent terrain voxels is eliminated — no shimmering at voxel boundaries.
3. Particle splats (sphere SDF type) continue to render with soft, anti-aliased edges.
4. No visual regression on open sky / empty tiles.
5. Frame time does not increase by more than 10% at 1080p (the prepass is cheap — box SDF only).

## Dependencies

- `e3340271` — SVO-Accelerated Ray Marching (base rasterizer and tile binning)
- `194ade77` — Tiled Forward+ Rasterizer (pipeline + bind group structure being modified)

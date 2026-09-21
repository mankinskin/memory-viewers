# SVO-Accelerated Ray Marching: Per-Voxel SDF Evaluation, Hi-Z Occlusion & Fragment Optimizations

> **Interview:** [interview.md](interview.md) — all 11 design questions answered and finalized.

## Problem

The current Tiled Forward+ Rasterizer (ticket `194ade77`) evaluates a simple `ray_box_closest_point` → `sd_box` per splat in the fragment shader. This produces crisp cube edges but cannot represent sub-voxel geometry detail. Additionally, the fragment loop has significant overdraw cost — every splat in a tile runs the full AABB test + PBR evaluation even when occluded by nearer opaque voxels.

This ticket upgrades the pipeline in five areas:

1. **Atomics-based active-list tile binning** with two-pass prefix-sum (replaces linear boundary scan)
2. **Bandwidth-optimized TileData packing** (2×u32 → 1×u32)
3. **SVO-guided SDF ray marching** inside surface voxels for sub-voxel detail (4 SDF types via 2-bit encoding)
4. **CPU-side surface/interior classification** during SVO mutation (skip interior voxels entirely)
5. **Z-prepass for top-N splats per tile** enabling hardware early-Z for the PBR pass

TAA is **deferred** to a future ticket — SDF gradient normals already provide inherent smoothing.

## Key Decisions (from Interview)

| # | Decision |
|---|----------|
| SDF encoding | 2 reserved bits in `color_data` (bits 30-31): box, sphere/round, SVO-sampled, custom/procedural |
| SVO descent | Hybrid binary occupancy, max 2-3 levels below current LOD, stack-less bitmask traversal |
| Surface detect | CPU-side in `set_voxel`/`remove_voxel` — interior bit prevents kernel emit |
| Hi-Z | Z-prepass for top ~4 sorted voxels/tile → hardware early-Z for PBR pass (no compute Hi-Z pyramid) |
| Active lists | Two-pass prefix-sum: count pass → prefix-sum → scatter pass. No fixed-capacity clipping |
| TileData | Pack now: `(offset << 12) \| count` in single u32 (20-bit offset, 12-bit count) |
| AA | Defer. `smoothstep + fwidth` + SDF gradients sufficient for now |
| Tile binning | Replace `build_tiles` scan immediately with atomics-based binning |
| SVO in fragment | Direct binding — bounded 2-3 level descent, surface voxels only |

## Context

### Current Architecture

```
VoxelSplatKernel → SortKeyBuild → RadixSort → TileBin → TiledRaster
```

- **OctreeNode** = 8 bytes (`child_pointer: u32`, `color_data: u32`)
- **VoxelSplat** = 24 bytes (`center_ws: vec3f`, `half_extent: f32`, `material_packed: u32`)
- **ProjectedSplat** = 48 bytes (`screen_min/max: vec2f`, `center_ws: vec3f`, `half_extent: f32`, `depth: f32`, `material_packed: u32`)
- **TileData** = 8 bytes (`offset: u32`, `count: u32`) per 16×16 tile
- SVO: max_depth=8 (256³ grid), nodes flat-indexed, child_pointer upper 24 bits = first-child offset
- Fragment shader: box SDF → `smoothstep(-fwidth, +fwidth, d)` → Cook-Torrance PBR
- Glass refraction pre-pass before voxel splat loop

### What This Ticket Changes

| Current | Proposed |
|---------|----------|
| `ray_box_closest_point` → single-point SDF | Short ray march (2-8 steps) inside SDF voxels along view ray |
| `box_normal` face approximation | Gradient-based normal: `normalize(SDF(p+ε) - SDF(p-ε))` per axis |
| No depth write | Z-prepass for top ~4 splats/tile → hardware early-Z for remaining PBR pass |
| All tile splats evaluated per pixel | Two-pass prefix-sum active-list (AABB-tile intersection, not just center-tile) |
| `TileData` = 2×u32 | Packed: offset(20 bits) + count(12 bits) = 1×u32 |
| `center_ws` + `half_extent` = vec3f + f32 | Packed into single `vec4f` (xyz=center, w=extent) |
| Interior voxels emitted by kernel | CPU-side interior bit → kernel skips interior voxels |
| SVO buffer only in compute | SVO buffer also bound in tiled raster fragment shader |
| Linear `build_tiles` boundary scan | Atomics-based tile binning with prefix-sum offsets |

## Implementation Plan (Priority Order)

### Phase 1: TileData Packing + vec4f Splat Packing

Immediate bandwidth win, minimal risk.

- Pack `TileData` into single `u32`: `(offset << 12) | count`
  - 20-bit offset → max 1,048,576 splats (matches `MAX_GAUSSIANS`)
  - 12-bit count → max 4,096 splats/tile (with early-out, <100 evaluated in practice)
  - **Overflow guard:** The count pass must clamp at 4095 — `if count >= 4095u { return; }`. Losing a few distant voxels is acceptable; an integer overflow would corrupt the offset of the next tile and cascade into rendering artifacts or out-of-bounds reads.
- Pack `center_ws + half_extent` into `vec4f` in `ProjectedSplat`
- Update `tile_binning.wgsl`, `tiled_raster.wgsl`, and Rust buffer stride constants
- Update `sort_key_build.wgsl` output layout

### Phase 2: Atomics-Based Active-List Tile Binning

Replace `build_tiles` linear boundary scan entirely.

**Pass 1 — Count:** Each thread reads one projected splat, computes which tiles its screen AABB overlaps, atomically increments `tile_counts[tile_idx]` for each overlapping tile.

**Prefix-Sum:** Exclusive scan over `tile_counts[]` → `tile_offsets[]`. Allocate `active_list` buffer sized to total count.

**Pass 2 — Scatter:** Each thread re-reads its splat, re-computes overlapping tiles, atomically grabs a slot from `tile_offsets[tile_idx]`, writes `splat_idx` into `active_list[slot]`.

> **Atomic-contention mitigation:** When a large voxel covers many tiles (e.g. near the camera), many threads write to the same `tile_counts` entries simultaneously, creating memory-controller hotspots. Use `var<workgroup>` local atomics: each thread increments a shared-memory counter first, then a single `workgroupBarrier()` + one global `atomicAdd` per tile flushes the accumulated count. This reduces global memory traffic by up to ~16× per workgroup.

```wgsl
// Pass 1: Count (with workgroup-local accumulation)
var<workgroup> local_tile_counts: array<atomic<u32>, 256>; // 16×16 tiles per workgroup region

@compute @workgroup_size(256)
fn count_tile_overlaps(
    @builtin(global_invocation_id) gid: vec3u,
    @builtin(local_invocation_index) local_idx: u32,
) {
    // Initialize local counters
    if local_idx < 256u { atomicStore(&local_tile_counts[local_idx], 0u); }
    workgroupBarrier();

    let splat_idx = gid.x;
    if splat_idx < splat_count {
        let s = projected[splat_idx];
        let tx0 = u32(s.screen_min.x) / TILE_SIZE;
        let tx1 = u32(s.screen_max.x) / TILE_SIZE;
        let ty0 = u32(s.screen_min.y) / TILE_SIZE;
        let ty1 = u32(s.screen_max.y) / TILE_SIZE;
        for (var ty = ty0; ty <= ty1; ty++) {
            for (var tx = tx0; tx <= tx1; tx++) {
                // Map global tile to workgroup-local slot
                let local_slot = (ty % 16u) * 16u + (tx % 16u);
                atomicAdd(&local_tile_counts[local_slot], 1u);
            }
        }
    }

    workgroupBarrier();

    // Flush local counts to global buffer (one write per tile)
    if local_idx < 256u {
        let count = atomicLoad(&local_tile_counts[local_idx]);
        if count > 0u {
            // Derive global tile index from workgroup + local position
            let wg_tile_y = gid.y / 16u * 16u + local_idx / 16u;
            let wg_tile_x = gid.x / 16u * 16u + local_idx % 16u;
            let global_tile = wg_tile_y * grid_width + wg_tile_x;
            atomicAdd(&tile_counts[global_tile], count);
        }
    }
}

// Pass 2: Scatter (after prefix-sum on CPU/compute)
@compute @workgroup_size(256)
fn scatter_to_tiles(@builtin(global_invocation_id) id: vec3u) {
    let splat_idx = id.x;
    if splat_idx >= splat_count { return; }
    let s = projected[splat_idx];
    let tx0 = u32(s.screen_min.x) / TILE_SIZE;
    let tx1 = u32(s.screen_max.x) / TILE_SIZE;
    let ty0 = u32(s.screen_min.y) / TILE_SIZE;
    let ty1 = u32(s.screen_max.y) / TILE_SIZE;
    for (var ty = ty0; ty <= ty1; ty++) {
        for (var tx = tx0; tx <= tx1; tx++) {
            let tile_idx = ty * grid_width + tx;
            let slot = atomicAdd(&tile_write_heads[tile_idx], 1u);
            active_list[tile_offsets[tile_idx] + slot] = splat_idx;
        }
    }
}
```

This eliminates per-fragment AABB rejection — every entry in a tile's active list genuinely overlaps that tile.

### Phase 3: SVO SDF Evaluation in Fragment Shader

Add 4 SDF types encoded in `material_packed` bits 30-31:

| Bits | Type | Evaluation |
|------|------|------------|
| `0b00` | Box | Current `sd_box` — no march, fastest path |
| `0b01` | Sphere/RoundBox | `sd_rounded_box` with corner radius derived from roughness |
| `0b10` | SVO-Sampled | Binary occupancy descent (2-3 levels), stack-less bitmask traversal |
| `0b11` | Custom/Procedural | Torus, cylinder — for tool previews |

**SVO-Sampled evaluation:**

> **Empty-pocket refinement:** The naive `return extent * 0.5` for empty octants is a poor SDF approximation — a ray marching through multiple consecutive empty octants can overshoot and miss small features. Instead, when an octant is empty, use the `child_mask` to compute the distance to the nearest _occupied_ sibling octant center. Use `firstLeadingBit` / `countTrailingZeros` for fast bitmask queries.

```wgsl
fn sdf_svo_sampled(p: vec3f, splat: ProjectedSplat) -> f32 {
    // Convert world-pos to octree coordinates
    let local = (p - splat.center_ws) / splat.half_extent; // [-1, 1]
    
    // Descend 2-3 levels from current node into children
    var node_idx = splat.node_index;  // New field: index into SVO
    var extent = splat.half_extent;
    
    for (var level = 0u; level < MAX_SUB_LEVELS; level++) {
        let node = octree[node_idx];
        let child_mask = node.child_pointer & 0xFFu;
        let first_child = node.child_pointer >> 8u;
        
        // Determine which octant p falls in
        let octant = octant_from_local(local, extent);
        
        if (child_mask & (1u << octant)) == 0u {
            // Empty octant → compute distance to nearest occupied sibling
            return sdf_empty_octant(local, child_mask, extent);
        }
        
        // Count preceding set bits for child index
        let child_offset = countOneBits(child_mask & ((1u << octant) - 1u));
        node_idx = first_child + child_offset;
        extent *= 0.5;
    }
    
    return -extent * 0.5;  // Inside occupied leaf → negative distance
}

// Distance estimator for empty octants using child_mask occupancy
fn sdf_empty_octant(p_local: vec3f, child_mask: u32, extent: f32) -> f32 {
    // Quick bail: no children at all → fully empty node
    if child_mask == 0u { return extent; }

    // Find nearest occupied sibling by computing distance to each
    // occupied octant's center (2×2×2 grid at half-extent offsets)
    var min_d = extent;
    for (var i = 0u; i < 8u; i++) {
        if (child_mask & (1u << i)) != 0u {
            let child_center = vec3f(
                select(-0.5, 0.5, (i & 1u) != 0u),
                select(-0.5, 0.5, (i & 2u) != 0u),
                select(-0.5, 0.5, (i & 4u) != 0u),
            ) * extent;
            min_d = min(min_d, length(p_local - child_center));
        }
    }
    return min_d;
}
```

**Gradient-based normals** (replaces `box_normal` for non-box types):
```wgsl
fn sdf_gradient(p: vec3f, splat: ProjectedSplat) -> vec3f {
    let e = vec3f(0.001 * splat.half_extent);
    return normalize(vec3f(
        sdf_eval(p + vec3f(e.x, 0.0, 0.0), splat) - sdf_eval(p - vec3f(e.x, 0.0, 0.0), splat),
        sdf_eval(p + vec3f(0.0, e.y, 0.0), splat) - sdf_eval(p - vec3f(0.0, e.y, 0.0), splat),
        sdf_eval(p + vec3f(0.0, 0.0, e.z), splat) - sdf_eval(p - vec3f(0.0, 0.0, e.z), splat),
    ));
}
```

**SVO buffer binding:** Add `octree: array<OctreeNode>` to the tiled raster bind group (already in VRAM, ~8 MB, just needs binding).

### Phase 4: CPU-Side Surface/Interior Classification

In `VoxelWorld::set_voxel` and `remove_voxel`, check 6 face-neighbors:
- If all 6 occupied at same depth → mark **interior** (new bit in `child_pointer` or separate bitfield)
- If any neighbor empty or different LOD → mark **surface**
- Dirty-neighbor tracking: when a voxel changes, re-classify its 6 neighbors too

`VoxelSplatKernel` skips interior-flagged nodes entirely — frustum culling + interior culling in one pass. Only surface voxels reach the projected buffer.

### Phase 5: Z-Prepass for Top-N Splats Per Tile

Two-pass rendering within the tiled raster node:

**Pass 1 — Depth prepass:** For each pixel, evaluate only the first ~4 sorted (front-to-back) voxels in the tile's active list. Write depth for voxels with `alpha ≈ 1.0`. Use cheap box SDF even for non-box types (the prepass only needs approximate depth). No PBR evaluation.

**Pass 2 — Full PBR pass:** Render with `@early_fragment_tests` enabled. Hardware early-Z rejects fragments behind the prepass depth. Full SDF evaluation + PBR for surviving fragments.

This avoids the `frag_depth` output problem in WebGPU (which disables hardware early-Z) by separating the depth write into its own pass.

> **Early-Z pipeline constraints (critical for wgpu):** The PBR pass `RenderPipeline` must be configured with:
> - `depth_write_enabled: false` — the PBR fragment shader must **not** write `@builtin(frag_depth)`, otherwise the GPU disables hardware early-Z and all fragments run the expensive PBR code.
> - `depth_compare: CompareFunction::Equal` (or `LessEqual`) — ensures only fragments matching the Z-prepass depth survive. `Equal` is optimal when the prepass already computes the exact SDF depth; `LessEqual` is the safer fallback if the prepass uses approximate box SDF.
> - The Z-prepass itself must include the SDF evaluation so the written depth is precise. If the prepass uses cheap box SDF, use `LessEqual` in the PBR pass to account for the approximation gap.

### Phase 6: TAA (Deferred)

Not in scope for this ticket. SDF gradient normals + `smoothstep + fwidth` provide adequate edge quality. TAA to be revisited after SDF pipeline is stable. A2C rejected due to MSAA bandwidth cost at 4K in the browser.

## Dependencies
- `194ade77` — Tiled Forward+ Rasterizer (base implementation must be working)
- T6a — VoxelSplatKernel (interior culling integration)
- T2a — GPU buffer infrastructure (active-list buffer, prefix-sum scratch)

## Implementation Notes

Critical details that must be respected during implementation:

| Area | Constraint |
|------|------------|
| Tile count overflow | Clamp at 4095 in count pass — overflow corrupts adjacent tile offsets |
| Atomic contention | Use `var<workgroup>` local atomics in count pass, flush once per tile per workgroup |
| SDF empty octant | Never return `extent * 0.5` — use `child_mask` distance to nearest occupied sibling center |
| SDF bitmask ops | Use `firstLeadingBit` / `countTrailingZeros` for fast child_mask queries |
| Early-Z pipeline | PBR pass must set `depth_write_enabled: false` and never output `@builtin(frag_depth)` |
| Depth compare | `CompareFunction::Equal` if prepass uses exact SDF depth, `LessEqual` if box-approximated |
| Workgroup sizing | `@workgroup_size(256)` for both count and scatter passes — matches WebGPU min limit |

## Acceptance Criteria
1. Four SDF types render correctly: box (unchanged), sphere/round, SVO-sampled, custom/procedural
2. SVO-sampled voxels show sub-voxel detail from child occupancy (2-3 level descent)
3. SDF gradient normals produce correct specular reflections (no face-stepping artifacts)
4. Active-list binning: zero per-pixel AABB rejection tests; all list entries overlap their tile
5. Prefix-sum allocation: no splat loss, no fixed-capacity clipping
6. Packed `TileData` u32 reduces tile buffer size by 50%
7. Interior voxels never reach the projected buffer (CPU-side classification)
8. Z-prepass + hardware early-Z measurably reduces fragment invocations (>30% in dense scenes)
9. No performance regression on simple box-only scenes (SDF dispatch is conditional on type bits)
10. Frame time ≤ 5ms at 1080p, ≤ 8ms at 4K for 1M voxel splats

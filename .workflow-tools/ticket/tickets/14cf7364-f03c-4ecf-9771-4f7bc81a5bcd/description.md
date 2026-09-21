# Rendering: Voxel Splatting from SVO — Ray-Box SDF Kernel, EWA Filtering, PBR & Tiled Forward+ Rasterization

> **Coordinator ticket** — this ticket has been decomposed into focused sub-tickets.
> Implementation work happens in the children; this ticket tracks overall completion.
>
> **Sub-tickets:**
> - **T6a** Voxel Splat Kernel: Ray-Box SDF Splatting — `f0ac6e8b-4e12-4765-9a9a-6b3e107f6779`
> - **T6b** Sort Key Construction & Tiled Depth Ordering — `5070c6b3-a37a-47fa-8dcf-69f805c1a2d2`
> - **T6c** GPU Radix Sort — `cf71418d-038b-4fc1-879d-0a302b681f84`
> - **T6d** Tiled Forward+ Rasterizer with Ray-Box SDF — `194ade77-6922-4be8-8c5b-4423173abcf6`
> - **T6e** PBR Material System: Cook-Torrance/GGX — `6851d03f-692e-4fc0-ada3-08480ecced6e`
>
> This ticket is done when all five sub-tickets are closed.

---

## Problem

The context-editor world must be rendered with photorealistic quality at 120 FPS. The Sparse Voxel Octree (SVO) provides structure and physics. **Voxel Splatting** renders each occupied voxel as an axis-aligned box evaluated per-pixel via a **ray-box SDF** distance function, with `smoothstep` soft edges filtered by screen-space `fwidth` derivatives. PBR materials (Cook-Torrance/GGX) are evaluated per-pixel from a compact `u32` material encoding. No Gaussian covariance matrices, no Spherical Harmonics — the SDF approach produces crisp voxel geometry that softens naturally at distance.

## Architecture: Dual-Layer Rendering

### Layer Separation

| Layer | Data | Purpose |
|-------|------|---------|
| **SVO (structure)** | `OctreeNode[]` in storage buffer | Physics collision, voxel editing, octree queries |
| **Voxel Splats (visual)** | Generated per-frame from SVO leaves | Photorealistic rendering with PBR, SDF edges, anti-aliasing |

The SVO is the single source of truth. Splats are ephemeral — regenerated every frame. This means:
- No splat storage on disk
- Voxel edits instantly produce new visuals
- LOD is automatic: `fwidth` of SDF distance provides free anti-aliasing

### Phase 1: Voxel Splat Generation (Compute — T6a)

For each occupied leaf voxel, emit one `VoxelSplat`:

```wgsl
struct VoxelSplat {
    center_ws: vec3f,       // world-space center
    half_extent: f32,       // half-size of axis-aligned box
    material_packed: u32,   // R8G8B8 + roughness5 + metallic1
    _pad: u32,
}
```

A compute shader traverses all `OctreeNode`s. Leaf nodes (child_mask == 0) emit a splat. Sub-pixel splats are culled by LOD threshold. No SH computation — material stays packed as a `u32`.

### Phase 2: Sort Key Construction (Compute — T6b)

Project each splat's AABB to screen-space, compute center tile, and build sort key:

```
sort_key = (tile_id << 12) | depth_quantized
```

This is cheaper than the old EWA projection: 8-corner AABB transform replaces Jacobian × covariance × transpose matrix chain.

### Phase 3: GPU Radix Sort (Compute — T6c)

8-pass, 4-bit radix sort for `tile_id | depth` ordering. Algorithm-agnostic — identical to the original design. Data stays in VRAM.

### Phase 4: Tile Binning (Compute — T6d, Phase 1)

Scan sorted keys to find per-tile `(offset, count)` boundaries.

### Phase 5: Ray-Box SDF Rasterization (Fragment — T6d, Phase 2)

Per-pixel evaluation of signed distance to each voxel's axis-aligned box:

```wgsl
fn sd_box(p: vec3f, half_ext: vec3f) -> f32 {
    let q = abs(p) - half_ext;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Anti-aliased edge:
let d = sd_box(local_pos, vec3f(half_extent));
let fw = fwidth(d);
let alpha = 1.0 - smoothstep(-fw, fw, d);
```

Per-pixel PBR material evaluation (Cook-Torrance/GGX — T6e):

```wgsl
let mat = unpack_material(splat.material_packed);
let color = evaluate_pbr(mat, normal, view_dir, light_dir, light_color);
```

Front-to-back alpha blending with early-out at `remaining_alpha < 0.01`.

### Key Differences from Old Gaussian Architecture

| Aspect | Old (Gaussian Splatting) | New (Voxel Splatting) |
|--------|-------------------------|----------------------|
| Per-voxel data | 232 bytes (pos + cov6 + SH48 + opacity) | 24 bytes (center + half + material_u32) |
| Shape | Soft elliptical blob (`exp(-power)`) | Crisp axis-aligned box (`sd_box`) |
| Color model | SH bands 0–3 (48 floats, view-dependent) | PBR Cook-Torrance/GGX (per-pixel, from u32) |
| Anti-aliasing | +0.3px² covariance blur | `fwidth(d)` + `smoothstep` |
| LOD | Multi-level octree traversal, covariance scaling | Sub-pixel culling + `fwidth` auto-softening |
| Projection | EWA: Jacobian × covariance × transpose | AABB 8-corner clip-space projection |

**Benefits of the pivot:**
- ~10× less per-splat memory → more splats fit in cache
- No SH evaluation per-pixel → bandwidth savings
- Crisp voxel edges that match the SVO structure exactly
- PBR gives physically-correct metallic/dielectric distinction
- `fwidth` anti-aliasing is free (GPU provides screen-space derivatives)

### Performance Targets

| Metric | Target | Method |
|--------|--------|--------|
| Voxel splat generation | < 0.5ms | 1 compute dispatch, no SH math |
| Sort key construction | < 0.3ms | AABB projection (faster than EWA) |
| Radix sort (1M splats) | < 1ms | 8 × 4-bit passes, data stays in VRAM |
| Tile binning | < 0.2ms | Single pass over sorted array |
| Tiled rasterization (1080p) | < 5ms | SDF + PBR per-pixel, early-out |
| Tiled rasterization (4K) | < 8ms | Same with 4× more tiles |
| Total frame (1M splats) | < 8ms | Leaves room for glass + particles |

## Scope

### WGSL Shaders
| Shader | Purpose |
|--------|---------|
| `voxel_splat_kernel.wgsl` | SVO → VoxelSplat[] generation + LOD cull |
| `sort_key_build.wgsl` | AABB → screen projection + tile sort key |
| `radix_sort.wgsl` | Histogram, prefix-sum (Blelloch scan), scatter |
| `tile_binning.wgsl` | Sorted keys → per-tile offset/count |
| `tiled_raster.wgsl` | Ray-box SDF + PBR + front-to-back compositing |
| `pbr_material.wgsl` | Shared: Cook-Torrance/GGX, unpack_material, BRDF LUT |

### Rust: Bevy Render Nodes
| Node | Type | Role |
|------|------|------|
| `VoxelSplatKernelNode` | Compute | Dispatch SVO → VoxelSplat generation |
| `SortKeyBuildNode` | Compute | Dispatch AABB projection + sort key build |
| `RadixSortNode` | Compute | 8-pass radix sort dispatch |
| `TileBinNode` | Compute | Build per-tile offset/count |
| `TiledRasterNode` | Fragment | Full-screen tiled SDF + PBR compositing |

### Rust: ECS Systems
| System | Schedule | Role |
|--------|----------|------|
| `camera_uniform_system` | `Update` | Extract camera matrices → `CameraUniforms` |
| `light_uniform_system` | `Update` | Extract lights → `GlobalUniforms` |
| `svo_upload_system` | `PostUpdate` | Dirty regions → BACK buffer `write_buffer` |
| `double_buffer_swap` | `PostUpdate` | Swap front/back after upload |

## Dependencies
- T1 (crate scaffold): Project structure, svo/ and render/ modules
- T2 (render init): Render graph, double-buffered SVO buffers, splat buffers

## Acceptance Criteria
1. Voxel splats generated from SVO — no pre-baked data stored
2. Ray-box SDF produces crisp voxel edges at close range
3. `fwidth` + `smoothstep` anti-aliasing prevents jaggies without over-blurring
4. PBR materials show correct metallic/dielectric distinction (Cook-Torrance/GGX)
5. Radix sort correctly orders splats by tile + depth
6. Tiled rasterizer composites front-to-back with early-out at saturated pixels
7. LOD visible: sub-pixel splats culled, distant splats softened by `fwidth`
8. Frame time < 10ms at 1080p for a 256³ voxel world (~1M splats)
9. Early-out optimization measurably reduces fragment work
10. No Bevy PbrBundle or Camera3dBundle — all rendering is custom voxel splatting pipeline

---
tags: `#interview` `#context-editor` `#rendering` `#svo` `#sdf` `#ray-marching` `#gpu`
summary: Design interview for SVO-accelerated ray marching, per-voxel SDF evaluation, Hi-Z occlusion, and fragment shader optimizations in the context-editor rendering pipeline.
status: ✅ complete
date: 2026-03-30
ticket: e3340271-557d-4d1c-bef7-db73712f468e
parent-ticket: 194ade77-6922-4be8-8c5b-4423173abcf6
---

# Interview: SVO-Accelerated Ray Marching & Fragment Optimizations

**Date:** 2026-03-30
**Scope:** context-editor rendering pipeline — SVO SDF evaluation, Hi-Z occlusion, tile active lists, bandwidth packing, TAA
**Ticket:** `e3340271` (depends on `194ade77`)

---

## Background

The current tiled rasterizer evaluates a single `sd_box` per voxel splat — crisp cubes, but no sub-voxel detail. Your feedback identifies five improvement axes: SDF ray marching inside surface voxels, Hi-Z occlusion culling, active-list tile filtering, bandwidth packing, and TAA. Before implementing, several architectural decisions need your input.

---

## Q1: SDF Type Encoding — Where to Store the Per-Voxel SDF Shape?

The current `OctreeNode.color_data` packs RGB(24 bits) + roughness(5 bits) + metallic(1 bit) = 30 bits used, **2 bits reserved** (30-31).

**Option A:** Use the 2 reserved bits for SDF type (`box`, `sphere`, `svo-sampled`, `reserved`) — fits perfectly, zero extra memory.

**Option B:** Add a separate `sdf_data: u32` field to `OctreeNode` (increases from 8 to 12 bytes, breaking the tight cache-line packing) but allows richer SDF parameterization (e.g., corner radius, blend factor).

**Option C:** Store SDF type in a separate per-splat buffer that only surface voxels populate — no AoS bloat, but requires an indirection.

Which approach? If A, are 4 SDF types sufficient for the near-term? If B, what additional SDF parameters matter?

> **Answer: Option A (2 reserved bits).** WebGPU memory alignment is critical — breaking the 8-byte struct alignment (Option B) causes massive padding. 4 types suffice:
> - `0b00` = Box (standard/fast)
> - `0b01` = Sphere/RoundBox (organic/soft forms)
> - `0b10` = SVO-Sampled (sub-voxel detail)
> - `0b11` = Custom/Procedural (torus, cylinder for tools)
>
> Corner radius for soft edges can be derived from the roughness value or hard-coded.

---

## Q2: SVO-Sampled SDF — What Should `sdf_type = 0b10` Actually Evaluate?

For `svo-sampled` voxels, we could evaluate the SDF by sampling the SVO's child occupancy as a distance field. Two possible approaches:

**Option A — Binary occupancy distance:** At each march step, descend into the SVO from the current world-space position, check whether the leaf is occupied, and use the distance to the nearest occupied/empty boundary as the SDF value. This gives organic surfaces where child voxels create sub-voxel detail.

**Option B — Pre-computed SDF volume:** During SVO construction, pre-compute a 3D distance field texture (e.g., 8³ per voxel) from child occupancy. Store a texture atlas index in the splat. The fragment shader samples this texture instead of descending the octree. Much faster per-sample but requires significant VRAM.

**Option C — Hybrid:** Use binary occupancy descent for close-up (LOD 0-2) and box SDF for distant voxels (LOD 3+).

How deep should the SVO descent go in the fragment shader? The SVO is max_depth=8 (256³). If a surface voxel is at depth 6, should the fragment shader descend 2 more levels into its children to evaluate sub-voxel shape?

> **Answer: Option C (Hybrid) with binary occupancy descent.** Fragment shader descends max 2-3 levels deeper than the current voxel LOD. No texture atlas (Option B) — 3D SDF textures would blow VRAM limits under WebGPU quotas. Since we're already at a leaf node, the local search space is small. A stack-less traversal via bitmasks is very performant in WGSL.

---

## Q3: Surface vs. Interior Classification — Where to Compute?

Interior voxels (all 6 face-neighbors occupied at same depth) can skip expensive SDF evaluation and use flat shading. Two options for when to classify:

**Option A — Splat kernel (GPU compute):** Tag each `VoxelSplat` with a `is_surface: bool` bit during `VoxelSplatKernel` dispatch. Requires neighbor queries during the kernel — 6 SVO descents per voxel.

**Option B — CPU-side during SVO mutation:** When `set_voxel` / `remove_voxel` is called, mark the node and its neighbors as surface/interior in a bitfield. Upload the bitfield alongside the SVO. No GPU cost but requires tracking dirty neighbors.

**Option C — Skip classification entirely:** Always evaluate SDF for visible voxels. Interior voxels are occluded by surface voxels anyway (after Hi-Z), so the classification is redundant if Hi-Z works well.

Which approach? Does the CPU-side tracking in VoxelWorld feel acceptable, or do you prefer keeping the SVO buffer-only (no separate metadata)?

> **Answer: Option B (CPU-side during SVO mutation).** Voxels change rarely per frame relative to the number rendered. CPU can efficiently check 6 neighbors during `set_voxel`. Saves 6 expensive SVO lookups per splat on the GPU. Mark interior voxels with a bit so the `VoxelSplatKernel` can skip them entirely — frustum culling + interior culling in one pass.

---

## Q4: Hi-Z Occlusion Strategy — Depth Pre-Pass or Inline Depth Write?

**Option A — Separate depth pre-pass:** Run the tile loop twice: first pass writes depth only (cheap — no PBR, no SDF march), second pass reads Hi-Z and skips occluded fragments. Doubles the tile iteration but the first pass is very cheap.

**Option B — Inline depth write with early-Z:** In the existing single-pass loop, when the first opaque voxel is hit (`alpha ≈ 1.0`), write `frag_depth` output. The GPU's early-Z hardware rejects subsequent fragments. No extra pass but requires careful fragment output declaration and may disable early-Z on some drivers when `frag_depth` is written conditionally.

**Option C — Deferred depth from compute:** After tile binning, run a compute pass that writes a depth buffer from the front-most splat per pixel (fast — just read `sorted_values[tile.offset]`). The fragment pass reads this as a Hi-Z texture.

Which strategy fits the WebGPU/WASM target best? Do you have experience with `frag_depth` + early-Z behavior on wgpu?

> **Answer: Hybrid A+B — Z-prepass for top-N splats per tile.** In WebGPU/wgpu, `early_fragment_tests` usually only works if you don't manually write `frag_depth`. Better strategy: run a Z-prepass (Option A) but only for the first ~4 sorted voxels per tile. This fills the depth buffer to ~90% coverage and enables real hardware early-Z for the expensive PBR pass, without the complexity of a full compute Hi-Z mip pyramid.

---

## Q5: Active-List Atomics — Memory Budget Concern

The atomics-based active-list approach (each splat writes into every tile it overlaps) produces a much tighter per-tile list but requires a worst-case allocation. For 1M splats where each might overlap up to ~4 tiles on average:

- **Worst-case buffer**: 4M entries × 4 bytes = 16 MB
- **Two-pass approach**: First pass counts per-tile totals, prefix-sum for offsets, second pass writes indices. Requires 2 dispatches but gives exact allocation.
- **Fixed-capacity per tile**: Allocate N slots per tile (e.g., 1024). Drop excess splats. Simpler but lossy.

Given the WASM/WebGPU memory constraints, which approach do you prefer? Is 16 MB acceptable, or should we use the two-pass prefix-sum method?

> **Answer: Two-pass prefix-sum.** WebGPU buffer limits are tricky — 16 MB sounds small but at 4K with complex overlaps you hit OOM or buffer limits quickly. The prefix-sum method is stable to implement in Rust/wgpu and guarantees no splat loss (no fixed-capacity clipping).

---

## Q6: TileData Packing — How Aggressive?

You suggested packing `TileData` from 2×u32 → 1×u32 with `offset(20 bits) | count(12 bits)`.

- 20-bit offset → max 1,048,576 splats before overflow (matches `MAX_GAUSSIANS`)
- 12-bit count → max 4,096 splats per tile

Is 4,096 splats per tile sufficient? In a dense scene viewed from afar, many small voxels could land in a single tile. If they're sorted front-to-back and we early-out at `remaining_alpha < 0.01`, do we expect to process more than ~50-100 splats per pixel in practice?

If active-list filtering (Q5) is implemented, the count represents actual overlapping splats rather than all splats whose center falls in the tile — likely much lower. Should we pack regardless or keep the simple 2×u32 until we have profiling data?

> **Answer: Pack now (20-bit offset / 12-bit count).** 4,096 splats per 16×16 tile is massive (16 splats/pixel). With early-out at `remaining_alpha < 0.01`, fewer than 100 splats per pixel will be evaluated in 99% of cases. Bandwidth savings from reading TileData in the fragment shader outweigh the theoretical headroom.

---

## Q7: TAA vs. Alpha-to-Coverage — Which Anti-Aliasing Path?

**Alpha-to-Coverage (A2C):**
- Hardware MSAA resolves sub-pixel coverage from alpha values
- Requires MSAA render target (2x or 4x) — doubles/quadruples bandwidth
- Simple to implement: just enable `alpha_to_coverage_enabled: true` in pipeline state
- Works in a single frame

**Temporal Anti-Aliasing (TAA):**
- Jitter projection per frame (Halton sequence), reproject + blend with previous frame
- Requires motion vector pass (or derive from depth + camera delta)
- Works with standard 1x render targets — no MSAA bandwidth cost
- Better quality for sub-pixel features but adds complexity and temporal ghosting risk

**Neither (ship `smoothstep + fwidth` as-is):**
- Current approach already looks decent for cube voxels
- SDF surfaces with gradient normals inherently smooth edges better than hard box normals
- Defer AA to a future polish pass

For WebGPU/WASM, MSAA can be expensive. TAA requires persistent frame history. Which do you prefer, or should we defer AA entirely and revisit after the SDF pipeline is stable?

> **Answer: Defer AA.** SDF gradient normals already provide inherent smoothing. TAA in the browser is complex due to floating-point precision issues and WASM overhead for history management. A2C doubles bandwidth — fatal for 4K framerate in the browser. Revisit TAA once the SDF pipeline is stable.

---

## Q8: Atomics-Based Tile Binning vs. Current Sorted-Key Boundary Scan

You mentioned replacing the linear `build_tiles` boundary scan with atomics-based binning. The current approach (scan sorted keys for tile transitions) is `O(n)` and correct, but assumes each splat belongs to exactly one tile (its center tile).

If we switch to active-list binning (Q5), the current `build_tiles` shader becomes redundant — the active-list compute replaces it entirely. But the active-list approach is heavier (atomics, multi-tile writes).

**Should we:**
- (A) Switch to active-list binning immediately and remove `build_tiles`?
- (B) Keep `build_tiles` as a fast path for simple scenes and use active-list only when `max_splats_per_tile > threshold`?
- (C) Ship the current `build_tiles` first, measure the per-pixel AABB rejection rate, and only add active-lists if the rejection rate is > X%?

> **Answer: Option A — replace the scan immediately.** Atomics-based binning is far more robust against edge cases (e.g., extreme close-up on a wall where the linear scan creates uneven workgroups). It also pairs naturally with the prefix-sum approach from Q5.

---

## Q9: Implementation Phasing — What's the Priority Order?

Given the 6 phases in the ticket:
1. SVO SDF evaluation in fragment shader
2. SVO neighbor query for surface detection
3. Hi-Z occlusion
4. Compute-side active-list filtering
5. Bandwidth optimizations (packing)
6. TAA

What's your preferred implementation order? My suggestion:

```
Phase 1 (SVO SDF) → Phase 3 (Hi-Z) → Phase 4 (Active Lists) → Phase 5 (Packing) → Phase 2 (Surface Detect) → Phase 6 (TAA)
```

Rationale: SDF evaluation is the visual centerpiece and tests the core idea. Hi-Z gives the biggest perf win. Active lists + packing are optimizations. Surface detection is a refinement of Phase 1. TAA is polish.

Do you agree, or would you reorder? Are any phases out of scope for now?

> **Answer:** Not explicitly reordered — Q8 answer implies active-list binning (Phase 4) should ship early alongside the tile rework. Implied priority from answers:
>
> 1. **Phase 5 (Packing)** — pack TileData now (Q6)
> 2. **Phase 4 (Active Lists)** — replace `build_tiles` immediately with atomics + prefix-sum (Q5, Q8)
> 3. **Phase 1 (SVO SDF)** — SDF type bits + hybrid descent (Q1, Q2)
> 4. **Phase 2 (Surface Detect)** — CPU-side interior culling (Q3)
> 5. **Phase 3 (Hi-Z)** — Z-prepass for top-N splats per tile (Q4)
> 6. **Phase 6 (TAA)** — deferred (Q7)

---

## Q10: SDF Function Library — Scope of Supported Primitives

Beyond box and sphere, what SDF primitives should the library support at launch?

Candidates:
- `sd_rounded_box(p, half_ext, radius)` — rounded cube edges (already in glass shader)
- `sd_cylinder(p, height, radius)` — pillars, tree trunks
- `sd_torus(p, major_r, minor_r)` — rings, donuts
- `sd_blend(d1, d2, k)` — smooth union between neighboring voxels
- Custom SDF atlas (texture3D lookup per voxel type)

The 2-bit SDF type field (Q1, Option A) only supports 4 types. If we want more, we need either a lookup table or the separate buffer approach.

What's the minimum set you need for a visually compelling demo?

> **Answer:** Implicitly answered via Q1: 4 SDF types cover the initial set:
> 1. Box (standard)
> 2. Sphere/RoundBox (organic — corner radius derived from roughness or hard-coded)
> 3. SVO-Sampled (sub-voxel detail from child occupancy)
> 4. Custom/Procedural (torus, cylinder for tool previews)
>
> Smooth blend (`sd_blend`) between neighbors is a desirable future addition but not launch-blocking.

---

## Q11: SVO Buffer Access in Fragment Shader — Feasibility?

Phase 1 (SVO-sampled SDF) and Phase 2 (surface detection) both require the fragment shader to read the SVO buffer (`octree: array<OctreeNode>`). Currently, the SVO buffer is only bound in Group 0 for the `VoxelSplatKernel` compute shader.

To make it accessible in the fragment shader:
- Add the SVO storage buffer to the tiled raster bind group
- The SVO is ~8 MB (1M nodes × 8 bytes) — already in VRAM, just needs binding
- SVO traversal in fragment shader is divergent (each pixel may traverse different paths) — could cause SIMD occupancy issues on some GPUs

Are you comfortable with SVO traversal in the fragment shader, or should we pre-compute SDF values in a compute pass and store them in a per-splat buffer instead? The compute pre-pass is more GPU-friendly (uniform workgroup traversal) but adds latency and memory.

> **Answer:** Implicitly answered via Q2 + Q3: Yes, SVO traversal in the fragment shader is acceptable. The hybrid approach (Q2) limits descent to 2-3 levels, keeping divergence bounded. CPU-side interior culling (Q3) ensures only surface voxels reach the fragment shader, and these are the ones that benefit from SVO descent. Stack-less bitmask traversal in WGSL keeps it fast. Bind the SVO buffer in the tiled raster bind group.

---

## Summary Table

| # | Topic | Decision |
|---|-------|----------|
| Q1 | SDF type storage | **A — 2 reserved bits** (box, sphere/round, SVO-sampled, custom/procedural) |
| Q2 | SVO-sampled SDF | **C — Hybrid** binary descent, max 2-3 levels deep, stack-less bitmask traversal |
| Q3 | Surface classification | **B — CPU-side** during `set_voxel`/`remove_voxel`, interior bit skips kernel emit |
| Q4 | Hi-Z strategy | **A+B hybrid** — Z-prepass for top ~4 splats/tile, then hardware early-Z for PBR pass |
| Q5 | Active-list memory | **Two-pass prefix-sum** — exact allocation, no splat loss, WebGPU-safe |
| Q6 | TileData packing | **Pack now** — 20-bit offset / 12-bit count in single u32 |
| Q7 | Anti-aliasing | **Defer** — SDF gradients smooth inherently; TAA later when stable |
| Q8 | Tile binning migration | **A — Replace immediately** with atomics-based binning |
| Q9 | Phase priority | Pack → Active Lists → SVO SDF → Surface Detect → Hi-Z → TAA (deferred) |
| Q10 | SDF primitives | 4 types: box, sphere/round, SVO-sampled, custom/procedural |
| Q11 | SVO in fragment shader | **Direct binding** — bounded descent (2-3 levels), stack-less, surface-only |

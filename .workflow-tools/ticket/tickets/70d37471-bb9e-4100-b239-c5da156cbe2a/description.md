# Phase 4b: LOD Cutoff

## Problem

With paging in place (Phase 4a), the ray march shader can handle arbitrarily large worlds. However, distant terrain is still traversed to full depth, wasting GPU cycles on sub-pixel detail. This ticket adds LOD (Level of Detail) cutoff: when a node's screen-space projection is smaller than a threshold, stop descending and render it as a solid voxel at its current level, using a pre-propagated average color.

## Design

### LOD Cutoff During Traversal

During SVO traversal, before descending into a child node, compute a continuous blend factor:

```wgsl
fn lod_blend_factor(node_half_extent: f32, ray_origin: vec3f,
                    node_center: vec3f, resolution_y: f32,
                    cot_half_fov: f32) -> f32 {
    let dist = length(node_center - ray_origin);
    // cot_half_fov = 1.0 / tan(fov_y * 0.5): makes threshold FOV-independent
    let screen_size = node_half_extent * cot_half_fov * resolution_y / dist;
    // Smooth transition band of width 2×LOD_SOFTNESS centered on LOD_THRESHOLD.
    // 0.0 → use coarse color (don’t descend); 1.0 → fully descend.
    return smoothstep(LOD_THRESHOLD - LOD_SOFTNESS, LOD_THRESHOLD + LOD_SOFTNESS, screen_size);
}
```

Based on the blend factor:
- `1.0`: fully descend — continue normal traversal to finer detail
- `0.0`: stop descending — treat this internal node as a “virtual leaf”; use its `color_data` (propagated average of descendants); evaluate a box SDF at this coarser level
- `(0, 1)` soft transition: compare against a per-pixel blue-noise value from a screen-space hash (e.g. `pcg_hash(frame_index * width * height + pixel_y * width + pixel_x)`). Pixels stochastically descend or stop; averaged over frames by TAA the result is a seamless edge with no pop-in, without needing to render the node at two LOD levels simultaneously. (Note: simple `frame_index ^ pixel_pos` XOR produces correlated banding — use a proper integer hash.)

Result: distant terrain renders as larger, cheaper voxels with pop-free transitions.

### Color Propagation for Internal Nodes

Internal nodes carry propagated average colors computed by `propagate_colors_up()` (introduced in Phase 4a). This function performs a bottom-up DFS, averaging each internal node’s `color_data` from its non-empty children. It runs once per SVO modification (not per-frame). The propagated colors are included in the page upload automatically.

This phase *consumes* the propagated colors for LOD rendering; Phase 4a introduces them for non-resident page fallback.

### Graceful Non-Resident Fallback

When the shader encounters a non-resident page (sentinel `0xFFFFFFFF` from Phase 4a's page table), it already stops descending. With LOD colors propagated, the parent node now has a meaningful color to render, so the fallback looks correct instead of showing black/empty.

## Implementation Plan

1. Add `lod_blend_factor()` and `pcg_hash()`-seeded stochastic descent helper to `svo_ray_march.wgsl`
2. Add `LOD_THRESHOLD` and `LOD_SOFTNESS` to `RayMarchUniforms` (note: `cot_half_fov` already exists from Phase 1b)
3. Update traversal loop: evaluate `lod_blend_factor()` before pushing children; compare against per-pixel temporal noise for stochastic descent decision
4. Render LOD-stopped nodes as box SDFs with their propagated color
5. Add LOD threshold and LOD softness sliders to debug overlay
6. Verify non-resident page fallback uses propagated colors correctly

## Acceptance Criteria

1. Distant terrain renders at coarser LOD — voxels appear larger far away.
2. LOD uses a smooth soft band (`LOD_SOFTNESS`, default 1.0 pixel) centered on `LOD_THRESHOLD`. Stochastic per-pixel descent decisions averaged by TAA eliminate visual pop-in. Both `LOD_THRESHOLD` and `LOD_SOFTNESS` are tunable via debug overlay sliders.
3. Internal nodes carry propagated average colors that look reasonable from distance.
4. LOD threshold and LOD softness are adjustable via debug overlay sliders.
5. Performance improvement measurable: increasing LOD_THRESHOLD reduces ray march depth, increasing FPS.
6. Non-resident pages (from Phase 4a) render with propagated parent color instead of black.
7. Close-up rendering at full LOD matches pre-LOD output (no regression when screen_size > threshold).

## Dependencies

- Phase 4a (paged upload + VAT — provides the page residency system and `resolve_node()` infrastructure)

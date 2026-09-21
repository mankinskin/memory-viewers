# Epic: Direct SVO Ray Marching — Replace Tiled Forward+ Pipeline

## Problem

The current rendering pipeline uses a multi-stage GPU-driven splatting approach:

```
VoxelSplatKernel → SortKeyBuild → RadixSort → TileBin (4 passes) → ZPrepass → TiledRaster
```

This architecture has fundamental problems:

1. **Wrong depth ordering**: Splat-based projection loses 3D structure. The sort key uses
   voxel *center* depth, but neighboring voxels at similar depths can appear in wrong order
   depending on per-pixel ray-surface intersections. Per-tile insertion sort cannot
   fix this because the ordering varies per pixel within a tile.

2. **Per-frame re-sort**: Every frame, ALL visible voxels are re-extracted from the SVO,
   re-projected, radix-sorted, tile-binned, and per-tile insertion-sorted. None of this work
   is cached across frames despite the SVO being static between edits.

3. **Tiles destroy SVO coherence**: The SVO is already a spatial acceleration structure, but
   the tiled pipeline flattens it into a screen-space grid, losing all spatial hierarchy.

4. **Performance**: 6 FPS at 1080p in dev profile. The pipeline is compute-bound on
   sorting and memory-bound on scatter/gather tile binning.

## Solution: Direct SVO Ray Marching

Replace the entire splat-sort-tile pipeline with a single compute shader that ray-marches
directly through the world-space SVO:

```
SVO Upload (paged + frustum-culled) → Per-Pixel Ray March Compute → Output Framebuffer
```
*(Phases 1–3 use the existing full-SVO upload; paged upload and frustum culling are introduced in Phase 4a.)*

### Key benefits:
- **Correct depth by construction**: Each ray traverses the octree front-to-back.
- **SVO is the acceleration structure**: Hierarchical traversal naturally skips empty space and provides LOD.
- **Frame coherence**: SVO on GPU only re-uploads when voxels change.
- **Secondary rays for free**: Reflections, refractions, shadows are additional ray marches through the same SVO.

## Architecture

### GPU Data
- World-space SVO: existing 2×u32 `OctreeNode` layout (child_pointer + color_data/atom_ref)
- Paged upload: CPU frustum-culls SVO subtrees, uploads only visible pages
- Virtual address table: maps SVO node indices to GPU buffer offsets for paged upload

### Compute Shader: `svo_ray_march.wgsl`
- One thread per pixel (workgroup_size 8×8)
- Ray generation from inverse view-projection
- Small-stack DDA traversal through SVO hierarchy
- At leaves: SDF evaluation for sub-voxel AA
- Front-to-back alpha compositing with early ray termination (opacity ≥ 0.99)
- PBR lighting from SDF gradient normals
- Writes RGBA + depth output

### Removed stages
- VoxelSplatKernel (splat extraction)
- SortKeyBuild (projection + sort key)
- RadixSort (8-pass radix sort)
- TileBin (4-pass tile binning + per-tile sort)
- ZPrepass (depth prepass)
- TiledRaster (fullscreen-triangle fragment shader)
- Glass panel special-case code

## Phases

1. **Core Shader**: World-to-SVO transform, core ray march compute shader (using existing full-SVO upload)
2. **Extensions**: SDF blending, secondary rays (shadows, reflections, refractions)
3. **Integration**: Full PBR lighting + depth buffer, then remove old tiled pipeline
4. **Optimization**: Frustum culling, paged upload + VAT, LOD cutoff

The key insight is: build and validate the core shader first on the existing full-SVO upload (no paging complexity), add all rendering features, then optimize with paging as the final step.

## Sub-Tickets

| Phase | Ticket | Description |
|-------|--------|-------------|
| 1a | `febe05b2` | World-to-SVO Transform and Layout Validation |
| 1b | `9ef831d0` | Core SVO Ray March Compute Shader |
| 2a | `22801e4f` | SDF Blending and Front-to-Back Alpha Compositing |
| 2b | `8c2f1575` | Secondary Rays — Reflections, Refractions, Shadows |
| 3a | `5e87d2e3` | Full PBR Lighting and Depth Buffer Integration |
| 3b | `5eea3447` | Remove Tiled Forward+ Pipeline |
| 4a | `86de425a` | Frustum Culling, Paged SVO Upload and Virtual Address Table |
| 4b | `70d37471` | LOD Cutoff |

## Dependencies

- Existing SVO data structure (`kernel/src/svo/mod.rs`)
- Existing double-buffer upload system (`kernel/src/svo/upload.rs`)
- Existing PBR lighting math (reused from `tiled_raster.wgsl`)
- SDF-DAG atom type system (`52ed521c`) — complementary, not blocking

## Files Affected

Major changes:
- `kernel/src/render/mod.rs` — new render graph (remove 6 old nodes, add 1 new)
- `kernel/src/render/svo_ray_march.wgsl` — new core shader
- `kernel/src/render/svo_ray_march.rs` — new render node + pipeline
- `kernel/src/svo/upload.rs` — paged upload + frustum culling
- `kernel/src/svo/mod.rs` — world-to-SVO transform, page management

Removed files:
- `kernel/src/render/voxel_splat_kernel.wgsl` + `.rs`
- `kernel/src/render/sort_key_build.wgsl` + `.rs`
- `kernel/src/render/radix_sort.wgsl` + `.rs`
- `kernel/src/render/tile_binning.wgsl` + `.rs`
- `kernel/src/render/z_prepass.wgsl` + `.rs`
- `kernel/src/render/tiled_raster.wgsl` + `.rs`

## Acceptance Criteria

1. Opaque terrain renders with correct depth ordering — no z-fighting or face-behind artifacts. *(Phase 1b)*
2. Frame rate ≥ 30 FPS at 1080p in release profile for a 256³ voxel world. *(Phase 4a+4b)*
3. Camera movement only re-uploads newly-visible frustum pages — the full SVO is never re-uploaded from a camera move alone. *(Phase 4a)*
4. SDF anti-aliasing at voxel edges (smoothstep fringe on exterior only). *(Phase 1b, refined in 2a)*
5. PBR lighting identical quality to current pipeline (Cook-Torrance + ambient). *(Phase 3a)*
6. Secondary rays (reflections/refractions) work uniformly for all voxel types. *(Phase 2b)*
7. LOD: distant voxels rendered at coarser octree levels without visual pop-in. *(Phase 4b)*
8. Old tiled pipeline fully removed — no dead shader/Rust code remaining. *(Phase 3b)*

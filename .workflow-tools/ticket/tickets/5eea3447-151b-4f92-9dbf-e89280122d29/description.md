# Phase 3b: Remove Tiled Forward+ Pipeline

## Problem

Once the SVO ray march pipeline is fully functional — with PBR lighting, depth buffer output, and secondary rays all working (Phases 1b through 3a) — the old Tiled Forward+ pipeline becomes dead code. It should be cleanly removed to reduce maintenance burden, compile times, and confusion.

**Prerequisites satisfied by Phase 3a**: The new ray march pipeline already provides full PBR lighting, writes an r32float depth texture, and runs `DepthBridgeNode` to convert that texture to a real Bevy `Depth32Float` hardware attachment. Wireframe overlay, particles, and UI compositing have been verified against the hardware depth attachment. It is now safe to remove the old pipeline without breaking any downstream render features.

## Scope

### Files to DELETE entirely:
- `kernel/src/render/voxel_splat_kernel.wgsl` — splat extraction from SVO
- `kernel/src/render/voxel_splat_kernel.rs` — splat kernel render node
- `kernel/src/render/sort_key_build.wgsl` — projection + sort key construction
- `kernel/src/render/sort_key_build.rs` — sort key render node
- `kernel/src/render/radix_sort.wgsl` — 8-pass GPU radix sort
- `kernel/src/render/radix_sort.rs` — radix sort render node
- `kernel/src/render/tile_binning.wgsl` — 4-pass tile binning + per-tile sort
- `kernel/src/render/tile_binning.rs` — tile bin render node
- `kernel/src/render/z_prepass.wgsl` — depth prepass compute shader
- `kernel/src/render/z_prepass.rs` — z-prepass render node
- `kernel/src/render/tiled_raster.wgsl` — fullscreen-triangle fragment shader
- `kernel/src/render/tiled_raster.rs` — tiled raster render node + pipeline

### Files to MODIFY:
- `kernel/src/render/mod.rs` — Remove old `ContextEditorLabel` variants (VoxelSplatKernel, SortKeyBuild, RadixSort, TileBin, ZPrepass, TiledRaster), remove old node registrations, remove old system registrations
- `kernel/src/gpu/mod.rs` — Remove `SplatBuffers` resource (splats, projected, sort_keys, sort_values, active_list, tile_data, tile_counts, tile_write_heads, depth_prepass, splat_count). Remove associated buffer allocation/initialization.
- `kernel/src/debug_overlay.rs` — Remove tiled-vs-ray-march toggle (ray march is now the only path), remove z-prepass toggle
- `kernel/src/svo/upload.rs` — Remove `NodePositionBuffer` if no longer needed (positions are computed analytically during traversal, not from a buffer)

### Buffers to REMOVE from GPU:
- `splats: array<VoxelSplat>`
- `projected: array<ProjectedSplat>`
- `sort_keys: array<u32>` + `sort_values: array<u32>`
- `active_list: array<u32>` + `tile_data: array<u32>`
- `tile_counts: array<atomic<u32>>` + `tile_write_heads: array<atomic<u32>>`
- `depth_prepass: array<f32>`
- `splat_count: atomic<u32>`
- Various radix sort histogram/temp buffers

### Render graph simplification:
Before: `BufferSwap → ParticleCompute → VoxelSplatKernel → SortKeyBuild → RadixSort → TileBin → ZPrepass → TiledRaster → UiComposite → WireframeOverlay`
After: `BufferSwap → ParticleCompute → SvoRayMarch → DepthBridge → UiComposite → WireframeOverlay`

## Implementation Plan

1. Switch the render graph to use only the ray march path (remove the A/B toggle from Phase 1b)
2. Delete the 12 WGSL + RS files listed above
3. Remove `SplatBuffers` and associated init/update systems from `gpu/mod.rs`
4. Remove old uniform update systems (sort_key_camera, tile_bin_uniforms, raster_uniforms)
5. Confirm `pbr.wgsl` (extracted in Phase 3a) is correctly imported wherever PBR functions are needed — after step 2 deletes `tiled_raster.wgsl`, verify no remaining references to PBR functions defined there (no duplicate extraction needed)
6. Clean up imports and dead code
7. Verify `cargo check --target wasm32-unknown-unknown -p kernel` passes
8. Verify `trunk build` succeeds
9. Run wireframe/particle/UI checks against the new depth buffer (should already work from 3a)

## Acceptance Criteria

1. All 12 files listed above are deleted.
2. `cargo check --target wasm32-unknown-unknown -p kernel` passes with no errors.
3. `trunk build` produces a working WASM binary.
4. The app renders correctly using only the SVO ray march pipeline.
5. No dangling references to removed types, buffers, or shaders.
6. GPU memory usage is reduced (no more splat/sort/tile buffers).
7. Compile time for `kernel` crate is noticeably reduced.

## Dependencies

- Phase 3a (PBR + depth buffer integration — depth output and PBR must be working in the new pipeline before the old pipeline is removed, otherwise wireframes/particles/UI compositing will break)

Note: Phase 3a already ensures PBR functions are extracted from `tiled_raster.wgsl` into a shared `pbr.wgsl`, and depth buffer output is integrated. This phase is purely deletion.

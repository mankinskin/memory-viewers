# Phase 3a: Full PBR Lighting and Depth Buffer Integration

## Problem

Phase 1b uses simplified Lambertian + ambient shading. This ticket upgrades to full Cook-Torrance PBR (matching the quality of the old tiled rasterizer) and integrates the ray march output with Bevy's standard depth buffer for compatibility with other render content (particles, wireframe overlay, UI).

**Critical sequencing note**: Depth buffer output MUST be implemented in this phase (before Phase 3b removes the old pipeline) to prevent a regression where wireframes, particles, and UI compositing break due to missing depth data. The old pipeline currently provides depth via z-prepass — the new pipeline must provide its own depth output before the old one is deleted.

## Design

### Full PBR from SDF Gradients
The surface normal is computed from the SDF gradient (central differences):
```wgsl
fn sdf_normal(p_local: vec3f, half: f32, sdf_type: u32) -> vec3f {
    let eps = 0.001;
    let dx = eval_voxel_sdf(p_local + vec3f(eps,0,0), half, sdf_type)
           - eval_voxel_sdf(p_local - vec3f(eps,0,0), half, sdf_type);
    let dy = eval_voxel_sdf(p_local + vec3f(0,eps,0), half, sdf_type)
           - eval_voxel_sdf(p_local - vec3f(0,eps,0), half, sdf_type);
    let dz = eval_voxel_sdf(p_local + vec3f(0,0,eps), half, sdf_type)
           - eval_voxel_sdf(p_local - vec3f(0,0,eps), half, sdf_type);
    return normalize(vec3f(dx, dy, dz));
}
```

Then apply the existing PBR functions from the old tiled_raster.wgsl:
- `ggx_distribution()`
- `fresnel_schlick()`
- `geometry_smith()`
- `evaluate_pbr()` — Cook-Torrance specular + Lambertian diffuse

These functions are extracted into a shared `pbr.wgsl` include file.

### Depth Buffer Integration
The ray march shader writes view-space depth of the first opaque hit. This must be converted to Bevy's depth format (infinite reverse-Z) and written to a depth texture:

```wgsl
// Convert world-space hit distance to clip-space depth
let hit_world = ray_origin + ray_dir * hit_t;
let clip_pos = view_proj * vec4f(hit_world, 1.0);
let ndc_depth = clip_pos.z / clip_pos.w;  // reverse-Z: near=1.0, far=0.0
textureStore(output_depth, pixel_xy, vec4f(ndc_depth, 0.0, 0.0, 0.0));
```

The r32float storage texture cannot be used directly for hardware depth testing by downstream Bevy render passes (they expect a `Depth32Float` hardware attachment). A thin `DepthBridgeNode` fullscreen fragment pass bridges the gap: it samples the r32float texture and writes `@builtin(frag_depth)` to a real `Depth32Float` hardware attachment:

```wgsl
// depth_bridge.wgsl — fullscreen fragment pass
@fragment
fn fs_main(@builtin(position) frag_coord: vec4f) -> @builtin(frag_depth) f32 {
    return textureLoad(ray_depth_tex, vec2i(frag_coord.xy), 0).r;
}
```

After the bridge pass, the hardware depth attachment is available for:
- Wireframe overlay (hardware depth testing against voxel surfaces)
- Particle system (depth-aware soft particles)
- UI composite (z-ordering)

### Output format change
Instead of a single RGBA storage texture, this phase introduces three GPU resources:
1. `output_color: texture_storage_2d<rgba8unorm, write>` — final lit color (written by ray march compute)
2. `output_depth: texture_storage_2d<r32float, write>` — NDC depth (written by ray march compute; sampled by `DepthBridgeNode`)
3. `depth_attachment: texture<depth32float>` — hardware depth attachment (written by `DepthBridgeNode` via `@builtin(frag_depth)`)

## Implementation Plan

1. Extract PBR functions from `tiled_raster.wgsl` into `pbr.wgsl` shared file **while the old pipeline still exists** (Phase 3b hasn't run yet)
2. Add PBR evaluation to the ray march hit shading (replace Lambertian)
3. Add `view_proj` matrix to `RayMarchUniforms`
4. Compute and write NDC depth for each primary hit
5. Create the `output_depth` r32float sampled texture and a real `Depth32Float` hardware depth texture; implement `DepthBridgeNode` as a fullscreen fragment pass (`depth_bridge.wgsl`) that reads r32float and writes `@builtin(frag_depth)` to the hardware depth attachment
6. Wire `DepthBridgeNode` into the render graph immediately after `SvoRayMarch`
7. Verify wireframe overlay works against the NEW hardware depth attachment while old pipeline still runs in parallel

## Acceptance Criteria

1. PBR lighting quality matches old tiled rasterizer — specular highlights visible on smooth surfaces.
2. Roughness and metallic material properties affect shading correctly.
3. Wireframe overlay renders with correct depth testing against voxel surfaces.
4. Particle system composites correctly with voxel depth.
5. Depth output uses Bevy's infinite reverse-Z convention.
6. No visual regression compared to Phase 1b+2b output (lighting upgrade only).
7. Both pipelines (old tiled + new ray march) can still run side by side at this point.

## Dependencies

- Phase 2a (SDF type dispatch and normals — `eval_voxel_sdf()` with `sdf_type` parameter used by `sdf_normal()`)
- Phase 2b (secondary rays — shadow/reflection working, shading framework complete)

Note: The old `tiled_raster.wgsl` is still present at this point (Phase 3b hasn't deleted it yet), so PBR functions can be directly copied/extracted from it.

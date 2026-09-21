# Tiled Forward+ Rasterizer: Tile Binning + Ray-Box SDF Fragment Compositing

## Problem

The final rendering stage: bin sorted voxel splats into 16×16 pixel tiles, then composite them per-pixel with front-to-back alpha blending in a fragment shader. Each pixel evaluates a **ray-box SDF** distance function against the voxel's axis-aligned bounding box, producing crisp edges via `smoothstep(−fw, fw, d)` where `fw = fwidth(d)` provides automatic screen-space anti-aliasing. PBR material is evaluated per-pixel using the unpacked `material_packed` u32 (via T6e's Cook-Torrance/GGX functions).

## Scope

### Phase 1: Tile Binning (Compute)

Scan sorted keys to find per-tile boundaries:

```wgsl
struct TileData {
    offset: u32,
    count: u32,
}

@compute @workgroup_size(256)
fn build_tiles(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    if idx >= sorted_count { return; }
    let tile_id = sorted_keys[idx] >> 12u;
    let prev_tile = select(0xFFFFFFFFu, sorted_keys[idx - 1u] >> 12u, idx > 0u);
    if tile_id != prev_tile {
        tile_data[tile_id].offset = idx;
    }
    let next_tile = select(0xFFFFFFFFu, sorted_keys[idx + 1u] >> 12u, idx < sorted_count - 1u);
    if tile_id != next_tile {
        tile_data[tile_id].count = idx - tile_data[tile_id].offset + 1u;
    }
}
```

### Phase 2: Fragment Rasterizer with Ray-Box SDF

```wgsl
// Signed distance to axis-aligned box
fn sd_box(p: vec3f, half_ext: vec3f) -> f32 {
    let q = abs(p) - half_ext;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    let tile_x = u32(in.coords.x) / TILE_SIZE;
    let tile_y = u32(in.coords.y) / TILE_SIZE;
    let tile_idx = tile_y * grid_width + tile_x;
    let tile = tile_data[tile_idx];

    var final_color = vec4f(0.0);
    var remaining_alpha = 1.0;

    // Reconstruct world-space ray from pixel coordinates
    let ray_origin = camera.position.xyz;
    let ray_dir = normalize(unproject(in.coords.xy, camera.inv_view_proj));

    for (var i = 0u; i < tile.count; i++) {
        let splat_idx = sorted_values[tile.offset + i];
        let s = projected[splat_idx];

        // Skip if pixel is outside splat's screen-space AABB
        if in.coords.x < s.screen_min.x || in.coords.x > s.screen_max.x ||
           in.coords.y < s.screen_min.y || in.coords.y > s.screen_max.y {
            continue;
        }

        // Local-space position for SDF evaluation
        let local_pos = ray_box_closest_point(ray_origin, ray_dir, s.center_ws, s.half_extent);
        let d = sd_box(local_pos - s.center_ws, vec3f(s.half_extent));

        // EWA-style anti-aliasing via screen-space derivative
        let fw = fwidth(d);
        let alpha = (1.0 - smoothstep(-fw, fw, d)) * remaining_alpha;
        if alpha < 1.0 / 255.0 { continue; }

        // PBR material evaluation (T6e)
        let mat = unpack_material(s.material_packed);
        let normal = box_normal(local_pos - s.center_ws, vec3f(s.half_extent));
        let view_dir = normalize(camera.position.xyz - local_pos);
        let color = evaluate_pbr(mat, normal, view_dir, light_dir, light_color);

        let weight = alpha;
        final_color += vec4f(color * weight, weight);
        remaining_alpha -= weight;

        if remaining_alpha < 0.01 { break; }  // EARLY-OUT: pixel saturated
    }

    return final_color;
}

fn ray_box_closest_point(ro: vec3f, rd: vec3f, center: vec3f, half_ext: f32) -> vec3f {
    // Ray-AABB intersection: find closest point on box surface along ray
    let inv_rd = 1.0 / rd;
    let t1 = (center - vec3f(half_ext) - ro) * inv_rd;
    let t2 = (center + vec3f(half_ext) - ro) * inv_rd;
    let tmin = max(max(min(t1.x, t2.x), min(t1.y, t2.y)), min(t1.z, t2.z));
    let tmax = min(min(max(t1.x, t2.x), max(t1.y, t2.y)), max(t1.z, t2.z));
    let t = select(tmin, 0.0, tmin < 0.0);  // inside box → t=0
    return ro + rd * t;
}

fn box_normal(p: vec3f, half_ext: vec3f) -> vec3f {
    // Approximate face normal from closest-face test
    let d = abs(p) - half_ext;
    let eps = vec3f(0.001);
    return normalize(sign(p) * step(d.yzx, d.xyz) * step(d.zxy, d.xyz));
}
```

### Key Differences from Old Gaussian Rasterizer

| Old (Gaussian) | New (Voxel Splat) |
|----------------|-------------------|
| `exp(-0.5 * power)` Gaussian falloff | `sd_box` + `smoothstep` → crisp box edges |
| Inverse 2D covariance matrix | Ray-box intersection → exact shape |
| Pre-computed SH color | Per-pixel PBR Cook-Torrance/GGX (T6e) |
| Soft blobs only | Hard voxel edges that soften with `fwidth` at distance |

### Early-Out Optimization

When `remaining_alpha < 0.01`, the pixel is saturated — no further splats can contribute visible color. This saves significant fragment work in dense scenes with many overlapping voxels.

### Integration Points

- Glass SDF pre-loop (T3a) inserts before the voxel splat loop, refraction shifts splat lookups
- Fullscreen triangle with no vertex geometry — all work in fragment shader
- PBR functions imported from T6e's shared `pbr_material.wgsl`

## Implementation Plan

1. Create `tile_binning.wgsl` compute shader (tile boundary detection)
2. Create `tiled_raster.wgsl` with `sd_box`, `ray_box_closest_point`, `box_normal`, PBR hookup
3. Implement `TileBinNode` (compute) + `TiledRasterNode` (render) replacing current stubs
4. Create fullscreen-triangle vertex shader
5. Bind groups: `[tile_data, sorted_keys, sorted_values, projected, camera, lights]`
6. Unit test: render a single voxel and verify SDF-correct silhouette

## Dependencies
- T6c (GPU radix sort): `sorted_keys[]`, `sorted_values[]` input
- T6e (PBR material): `evaluate_pbr()`, `unpack_material()` functions
- T2a (GPU buffer infra): `tile_data[]`, `projected[]` buffers
- T2b (render graph): `TileBin` + `TiledRaster` node slots

## Acceptance Criteria
1. Tile binning produces correct per-tile offset/count from sorted data
2. Fragment shader evaluates `sd_box` for each splat — voxel edges are crisp up close
3. `fwidth` + `smoothstep` anti-aliasing prevents jaggies without blur
4. PBR materials show correct diffuse/specular response (metallic/dielectric distinction)
5. Early-out measurably reduces fragment work (compare with/without)
6. Visual output: sharp cubes near camera, softening at distance, correct depth ordering
7. Frame time < 5ms at 1080p, < 8ms at 4K for 1M voxel splats
8. Empty tiles (no splats) render background color

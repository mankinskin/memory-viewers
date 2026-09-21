# VFX: Liquid Glass — SDF Refraction of Voxel Splats with Chromatic Aberration, Caustics, and Mipmap Blur

> **Coordinator ticket** — this ticket has been decomposed into focused sub-tickets.
> Implementation work happens in the children; this ticket tracks overall completion.
>
> **Sub-tickets:**
> - **T3a** Glass SDF Core — Analytical SDF & Snell's Refraction — `a87f450a-f703-4773-8467-44718d5ba70f`
> - **T3b** Glass VFX — Chromatic Aberration, Caustics & Mipmap Blur — `5008909b-e6f2-40a4-897c-8cd359efc292`
>
> This ticket is done when both sub-tickets are closed.

---

## Problem

UI panels must appear as physically realistic glass floating in 3D space. In the Voxel Splatting pipeline, glass is an **analytical SDF evaluated per-pixel in the tiled rasterizer**. When a pixel is inside a glass region, the lookup coordinates are refracted via Snell's law before sampling tiled splats, producing chromatic aberration, pseudo-caustics, and mipmap-based frosted blur.

## Architecture: Glass in the Tiled Forward+ Renderer

### Why This Is Better Than Glass + Ray Marching

In the previous SVO-only approach, glass was evaluated during ray marching. With voxel splatting the visual layer is tile-sorted splats, not ray-marched voxels. Glass now operates in 2D screen-space on the tiled output:

1. Per-pixel: evaluate glass SDF → get refraction offset
2. The offset shifts which tile's splats are sampled (cross-tile lookup)
3. Since all tiles are parallel in VRAM, this costs almost nothing
4. Chromatic aberration, caustics, and frosted blur are all simple post-refraction effects

### Glass SDF Evaluation (unchanged from SVO era)

```wgsl
struct GlassPanel {
    center: vec3f,
    half_size: vec3f,
    corner_radius: f32,
    ior: f32,
    tint: vec4f,
    blur_roughness: f32,
}

fn sdf_rounded_box(p: vec3f, panel: GlassPanel) -> f32 {
    let q = abs(p - panel.center) - panel.half_size + panel.corner_radius;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - panel.corner_radius;
}
```

### Snell's Law Refraction

```wgsl
fn refract_ray(incident: vec3f, normal: vec3f, eta: f32) -> vec3f {
    let cos_i = dot(-incident, normal);
    let sin2_t = eta * eta * (1.0 - cos_i * cos_i);
    if sin2_t > 1.0 { return reflect(incident, normal); } // total internal reflection
    let cos_t = sqrt(1.0 - sin2_t);
    return eta * incident + (eta * cos_i - cos_t) * normal;
}
```

### Chromatic Aberration (Spectral RGB Split)

Real glass bends blue light more than red. Instead of three full ray traces, we apply a single-pass UV offset per channel:

```wgsl
fn get_chromatic_refraction(uv: vec2f, distortion: vec2f) -> vec3f {
    let r = sample_tiled_splats(uv + distortion * 1.0).r;
    let g = sample_tiled_splats(uv + distortion * 1.1).g;
    let b = sample_tiled_splats(uv + distortion * 1.2).b;
    return vec3f(r, g, b);
}
```

### Pseudo-Caustics (Refraction Divergence → Brightness)

Where refraction vectors converge, light concentrates. We approximate this using `fwidth`:

```wgsl
let distortion = calculate_refraction(in.uv);
let caustics = fwidth(distortion.x + distortion.y) * caustic_strength;
final_color += vec3f(caustics * light_color.rgb);
```

This is orders of magnitude cheaper than path-traced caustics and visually convincing for UI glass.

### Frosted Glass via Mipmap Blur

Instead of per-pixel blur (expensive), we use `textureSampleLevel` on a mipmapped background capture:

```wgsl
fn get_frosted_glass(uv: vec2f, roughness: f32) -> vec4f {
    // roughness 0.0 = clear glass, 1.0 = fully frosted
    let lod_level = roughness * 9.0; // 10 mip levels for 1024px
    return textureSampleLevel(bg_tex, bg_sampler, uv, lod_level);
}
```

Adaptive roughness from SDF curvature:
```wgsl
let normal = get_sdf_normal(in.uv, element);
let curvature = length(fwidth(normal));
let blur_amount = clamp(base_roughness + curvature * 2.0, 0.0, 1.0);
```

**Why this is fast**: GPU hardware interpolates between mip levels on-chip. Smaller mips fit in L1 cache, making frosted glass actually *faster* than clear glass.

### Integration into Tiled Rasterizer

The glass evaluation happens **before** the splat tile loop:

```wgsl
@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    var tile_x = u32(in.coords.x) / TILE_SIZE;
    var tile_y = u32(in.coords.y) / TILE_SIZE;
    var uv = in.coords.xy / resolution;
    var glass_tint = vec4f(1.0);

    // Glass SDF check
    for (var g = 0u; g < glass_count; g++) {
        let d = sdf_rounded_box(pixel_to_world(in.coords), glass_panels[g]);
        if d < 0.0 {
            let normal = glass_sdf_normal(pixel_to_world(in.coords), glass_panels[g]);
            let refracted = refract_ray(view_dir, normal, 1.0 / glass_panels[g].ior);
            let distortion = refracted.xy - view_dir.xy;

            // Chromatic aberration
            if glass_panels[g].blur_roughness < 0.01 {
                // Clear glass: chromatic refraction of splats
                let refracted_color = get_chromatic_refraction(uv, distortion);
                glass_tint *= vec4f(refracted_color * glass_panels[g].tint.rgb, 1.0);
            } else {
                // Frosted glass: mipmap blur
                let frosted = get_frosted_glass(uv + distortion, glass_panels[g].blur_roughness);
                glass_tint *= vec4f(frosted.rgb * glass_panels[g].tint.rgb, 1.0);
            }

            // Caustics
            let caustics = fwidth(distortion.x + distortion.y) * 5.0;
            glass_tint.rgb += caustics * light_color.rgb;

            // Shifted tile for refracted lookup
            uv += distortion;
            tile_x = u32(uv.x * resolution.x) / TILE_SIZE;
            tile_y = u32(uv.y * resolution.y) / TILE_SIZE;
        }
    }

    // Standard tiled splat loop (using potentially shifted tile)
    let tile_idx = tile_y * grid_width + tile_x;
    let tile = tile_data[tile_idx];
    var final_color = vec4f(0.0);
    var remaining_alpha = 1.0;

    for (var i = 0u; i < tile.count; i++) {
        let inst = sorted_instances[tile.offset + i];
        let g = projected[inst.splat_id];
        let d = uv * resolution - g.center_screen;
        let power = -0.5 * (d.x * d.x * g.cov2d_inv[0] + d.y * d.y * g.cov2d_inv[1] + 2.0 * d.x * d.y * g.cov2d_inv[2]);
        if power > 0.0 { continue; }
        let alpha = min(0.99, g.opacity * exp(power));
        if alpha < 1.0 / 255.0 { continue; }
        let weight = alpha * remaining_alpha;
        final_color += vec4f(g.color * weight, weight);
        remaining_alpha *= (1.0 - alpha);
        if remaining_alpha < 0.01 { break; } // early-out: tile saturated
    }

    return final_color * glass_tint;
}
```

### Bevy ECS Integration

```rust
#[derive(Component)]
pub struct GlassPanel {
    pub ior: f32,
    pub tint: Color,
    pub blur_roughness: f32,
    pub corner_radius: f32,
    pub caustic_strength: f32,
    pub chromatic_spread: f32, // RGB offset multiplier
}

fn glass_panel_uniform_system(
    query: Query<(&Transform, &GlassPanel, &LayoutRect)>,
    mut glass_buffer: ResMut<GlassPanelBuffer>,
) { /* pack into GPU uniform */ }
```

### Performance

| Metric | Target |
|--------|--------|
| Glass SDF eval per pixel | < 0.01ms (analytical) |
| Chromatic aberration | 3 tile lookups instead of 1 (~3× per-pixel, but only in glass region) |
| Pseudo-caustics | 1 `fwidth` per pixel (essentially free) |
| Frosted blur | 1 `textureSampleLevel` (hardware mip interpolation, 0 extra cycles) |
| Cross-tile refraction | Near zero (all tiles in VRAM, linear access) |
| Max panels per frame | 16 (expandable via storage buffer) |

## Scope

### WGSL (in `tiled_render.wgsl`)
- `GlassPanel` struct, `sdf_rounded_box()`, `refract_ray()`
- `get_chromatic_refraction()`, pseudo-caustics via `fwidth`
- `get_frosted_glass()` using `textureSampleLevel`
- Adaptive curvature-based roughness
- Integration pre-loop in tiled rasterizer

### Rust: ECS
- `GlassPanel` component (ior, tint, roughness, caustic_strength, chromatic_spread)
- `GlassPanelBuffer` resource
- `glass_panel_uniform_system`

### Rust: Render
- Mipmap generation pass for background capture texture
- Glass uniform bind group layout

## Dependencies
- T2 (render init): Tiled rasterizer pass, mipmap generation infrastructure
- T6 (3D scene): splat generation + sorted tile data that glass refracts
- T9 (bridge): Panel positions from Taffy layout → 3D transform

## Acceptance Criteria
1. Glass panel SDF visible as translucent region in the tiled-rendered scene
2. Objects (splats) behind glass appear refracted via Snell's law
3. Chromatic aberration produces visible RGB fringing at glass edges
4. Pseudo-caustics brighten converging refraction regions
5. Frosted glass (roughness > 0) uses mipmap blur — NOT per-pixel blur
6. Curvature-adaptive roughness: glass edges appear more diffuse than flat centers
7. Two overlapping glass panels produce cumulative refraction and tinting
8. Total internal reflection occurs at extreme viewing angles
9. Cross-tile splat sampling through refracted coordinates works correctly

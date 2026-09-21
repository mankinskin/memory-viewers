# Glass SDF Core: Analytical SDF Evaluation + Snell's Refraction in Tiled Rasterizer

## Problem

UI panels must appear as physically realistic glass floating in 3D space. This ticket implements the core glass system: analytical SDF evaluation per-pixel, Snell's law refraction to bend splat lookup coordinates, and the glass pre-loop integration in the tiled rasterizer fragment shader.

## Scope

### Glass SDF (WGSL)

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

### Snell's Law Refraction (WGSL)

```wgsl
fn refract_ray(incident: vec3f, normal: vec3f, eta: f32) -> vec3f {
    let cos_i = dot(-incident, normal);
    let sin2_t = eta * eta * (1.0 - cos_i * cos_i);
    if sin2_t > 1.0 { return reflect(incident, normal); }
    let cos_t = sqrt(1.0 - sin2_t);
    return eta * incident + (eta * cos_i - cos_t) * normal;
}
```

### Glass Pre-Loop in Tiled Rasterizer

The glass SDF is evaluated BEFORE the splat tile loop. If inside glass, the refraction offset shifts which tile's splats are sampled (cross-tile lookup):

```wgsl
// In fs_main() — before splat loop
for (var g = 0u; g < glass_count; g++) {
    let d = sdf_rounded_box(pixel_to_world(in.coords), glass_panels[g]);
    if d < 0.0 {
        let normal = glass_sdf_normal(pixel_to_world(in.coords), glass_panels[g]);
        let refracted = refract_ray(view_dir, normal, 1.0 / glass_panels[g].ior);
        let distortion = refracted.xy - view_dir.xy;
        uv += distortion;
        tile_x = u32(uv.x * resolution.x) / TILE_SIZE;
        tile_y = u32(uv.y * resolution.y) / TILE_SIZE;
        glass_tint *= glass_panels[g].tint;
    }
}
```

### ECS Integration

```rust
#[derive(Component)]
pub struct GlassPanel {
    pub ior: f32,
    pub tint: Color,
    pub blur_roughness: f32,
    pub corner_radius: f32,
}

#[derive(Resource)]
pub struct GlassPanelBuffer { /* GPU storage */ }

fn glass_panel_uniform_system(
    query: Query<(&Transform, &GlassPanel, &LayoutRect)>,
    mut glass_buffer: ResMut<GlassPanelBuffer>,
) { /* pack into GPU uniform */ }
```

## Dependencies
- T2b (render graph): Tiled rasterizer node where glass pre-loop runs
- T6d (tiled rasterizer): Fragment shader that this pre-loop integrates into

## Acceptance Criteria
1. Glass panel SDF visible as translucent region in the scene
2. splats behind glass appear refracted via Snell's law
3. Cross-tile splat sampling through refracted coordinates works
4. Two overlapping glass panels produce cumulative refraction + tinting
5. Total internal reflection at extreme viewing angles
6. Max 16 glass panels per frame (expandable)

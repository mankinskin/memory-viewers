# 3D UI Panels: Glass SDF Elements in Voxel-Splatted World

> **Coordinator ticket** — this ticket has been decomposed into focused sub-tickets.
> Implementation work happens in the children; this ticket tracks overall completion.
>
> **Sub-tickets:**
> - **T10a** WorldPanel Rendering — Glass SDF Panels & Content Textures — `d02afc1f-7e0c-483f-a70d-86c7b1e088ad`
> - **T10b** Panel Interaction — 3D Hit Testing & Input Dispatch — `8d289d6c-941b-41e2-aaa3-58465bcba3d3`
>
> This ticket is done when both sub-tickets are closed.

---

## Problem

In-world UI panels (floating labels, health bars, menus anchored to positions) exist as glass SDF shapes inside the 3D scene. They interact with the voxel splatting pipeline: the tiled rasterizer renders splats behind/around them, and the glass shader refracts those splats through the panel surface.

## Architecture

### Panel as Glass SDF

Each world panel is defined as a flat rectangular SDF (rounded-rect) positioned in 3D space:

```rust
#[derive(Component)]
pub struct WorldPanel {
    pub half_extents: Vec2,       // panel size in world units
    pub corner_radius: f32,
    pub content_texture: Handle<Texture>,  // UI rendered to texture
    pub roughness: f32,           // 0.0 = clear glass, 1.0 = fully frosted
    pub tint: Vec3,               // color tint for glass
    pub anchor: PanelAnchor,      // world position + orientation
}

pub enum PanelAnchor {
    WorldFixed(Vec3, Quat),                // absolute position
    EntityAttached(Entity, Vec3),           // offset from entity
    Billboard { target: Entity, offset: Vec3 }, // always faces camera
}
```

### Glass Interaction with splats

The panel's SDF is evaluated in the tiled rasterizer's glass pre-loop (same path as T3).  
When a pixel hits a panel SDF:
1. **Clear panel** (roughness < 0.1): Chromatic aberration refraction of splats behind it, plus alpha-blended content texture on top
2. **Frosted panel** (roughness > 0.1): Mipmap blur of splats behind, curvature-adaptive at edges, content on top
3. **Opaque panel** (roughness = 1.0): No refraction, just content texture

```wgsl
// In glass pre-loop (tiled_rasterizer.wgsl)
let panel_sdf = evaluate_panel_sdf(world_pos, panel_center, panel_normal, panel_extents, panel_radius);
if panel_sdf < 0.0 {
    let panel_normal = panel_normal_at(world_pos, panel_center, panel_normal);
    if panel_roughness < 0.1 {
        // Clear glass: chromatic refraction of splats
        refraction_offset = chromatic_refract(view_dir, panel_normal, 1.0 / 1.5);
    } else {
        // Frosted: mipmap blur
        frost_level = panel_roughness * 9.0 + fwidth(panel_normal) * 4.0;
    }
}
```

### Content Rendering

Panel content (text, icons, layouts) is rendered to a separate texture per panel using the same Dioxus→Taffy pipeline (T9). This texture is sampled in the tiled rasterizer when the pixel is inside the panel SDF:

```wgsl
let content_color = textureSample(panel_content_tex, sampler, panel_uv);
// Alpha-blend content over refracted/frosted voxel-splatted background
final_color = mix(refracted_bg, content_color.rgb, content_color.a);
```

### Billboard System

```rust
fn billboard_system(
    camera: Query<&Transform, With<Camera3d>>,
    mut panels: Query<(&WorldPanel, &mut Transform), Without<Camera3d>>,
) {
    let cam_pos = camera.single().translation;
    for (panel, mut tf) in panels.iter_mut() {
        if let PanelAnchor::Billboard { .. } = panel.anchor {
            tf.look_at(cam_pos, Vec3::Y);
        }
    }
}
```

### Hit Testing in 3D

Ray-cast from mouse through camera, intersect with panel planes (not SVO colliders). Panels have higher priority than world geometry for UI interaction.

## Dependencies
- T3 (liquid glass): Glass SDF evaluation, chromatic refraction, mipmap frosted blur
- T6 (3D scene): Panel SDFs registered with tiled rasterizer
- T9 (bridge): Content textures from Dioxus→Taffy rendering
- T2 (render init): Bind groups for panel content textures

## Acceptance Criteria
1. World panels render as glass SDFs in the voxel-splatted scene
2. Clear glass panels show chromatic refraction of splats behind them
3. Frosted glass panels show mipmap-blurred splats with curvature blur at edges
4. Panel content (text, UI) alpha-blended over glass background
5. Billboard panels face camera
6. Entity-attached panels follow their host entity
7. Mouse ray-cast hits panels for click/hover interaction
8. Panel SDF edge smoothing (no aliased edges)

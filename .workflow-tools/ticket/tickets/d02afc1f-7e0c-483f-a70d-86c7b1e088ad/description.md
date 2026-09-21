# WorldPanel Rendering: Glass SDF Panels with Content Textures in 3D Scene

## Problem

In-world UI panels (floating labels, menus, information displays) must render as glass SDF shapes integrated into the tiled voxel splat rasterizer. Each panel has a content texture (rendered by Dioxus→Taffy) that is alpha-blended over the glass background. This ticket covers the rendering side — interaction is in T10b.

## Scope

### WorldPanel Component

```rust
#[derive(Component)]
pub struct WorldPanel {
    pub half_extents: Vec2,
    pub corner_radius: f32,
    pub content_texture: Handle<Texture>,
    pub roughness: f32,           // 0.0 = clear glass, 1.0 = fully frosted
    pub tint: Vec3,
    pub anchor: PanelAnchor,
}

pub enum PanelAnchor {
    WorldFixed(Vec3, Quat),
    EntityAttached(Entity, Vec3),
    Billboard { target: Entity, offset: Vec3 },
}
```

### Glass Integration

Panel SDFs are registered in the glass panel buffer (T3a) and evaluated in the tiled rasterizer's glass pre-loop. Glass interaction (clear vs frosted) uses the same path as regular glass.

### Content Texture Sampling

```wgsl
let content_color = textureSample(panel_content_tex, sampler, panel_uv);
final_color = mix(refracted_bg, content_color.rgb, content_color.a);
```

Content is rendered to a texture per panel using the Dioxus→Taffy pipeline (T9), then sampled when the fragment is inside the panel SDF.

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

### Entity Attachment

Panels with `EntityAttached` anchor follow their host entity's transform (offset applied in local space).

## Dependencies
- T3a (Glass SDF core): Glass pre-loop, SDF evaluation
- T9 (bridge): Content textures from Dioxus→Taffy rendering
- T2a (GPU buffer infra): Bind groups for panel content textures

## Acceptance Criteria
1. World panels render as glass SDFs in the voxel-splatted scene
2. Clear glass panels show refracted splats behind them
3. Frosted glass panels show mipmap-blurred splats
4. Panel content alpha-blended over glass background
5. Billboard panels face camera
6. Entity-attached panels follow host entity
7. Panel SDF edge smoothing (no aliased edges)

# Style: Theme Palette Driving SVO Materials, PBR Parameters, and Glass Tints

## Problem

All visual elements — SVO voxel colors, PBR roughness/metallic parameters, glass panel tints, particle colors, lighting — must be driven by a single `ThemePalette` Bevy resource for runtime re-theming.

## Architecture: Palette → Packed Material → Voxel Color

### ThemePalette Resource

```rust
#[derive(Resource, Clone)]
pub struct ThemePalette {
    // Voxel materials (stored in SVO color_data, drive PBR rendering)
    pub voxel_primary: MaterialDef,
    pub voxel_secondary: MaterialDef,
    pub voxel_terrain: MaterialDef,
    pub voxel_highlight: MaterialDef,

    // Glass
    pub glass_tint: Color,
    pub glass_frosted_tint: Color,
    pub glass_accent: Color,

    // Particles
    pub particle_primary: Color,
    pub particle_secondary: Color,

    // Lighting
    pub ambient_color: Color,
    pub key_light_color: Color,
    pub fill_light_color: Color,

    // UI text
    pub text_primary: Color,
    pub text_secondary: Color,
}

#[derive(Clone)]
pub struct MaterialDef {
    pub base_color: Color,
    pub roughness: f32,
    pub metallic: bool,
}
```

### Palette → Packed u32 Material

When a voxel is placed or a theme changes, `MaterialDef` is packed into the `OctreeNode.color_data` u32 field using the encoding from T6e:

```
Bits 0–7:   R (8 bits)
Bits 8–15:  G (8 bits)
Bits 16–23: B (8 bits)
Bits 24–28: Roughness (5 bits, 0–31 → 0.0–1.0)
Bits 29:    Metallic (1 bit)
Bits 30–31: Reserved
```

The voxel splat kernel (T6a) passes this `u32` through to the tiled rasterizer (T6d), which unpacks it per-pixel for Cook-Torrance/GGX evaluation (T6e). No SH coefficients are involved — PBR parameters are evaluated directly.

### Theme Change System

```rust
fn theme_update_system(
    palette: Res<ThemePalette>,
    mut voxel_world: ResMut<VoxelWorld>,
) {
    if palette.is_changed() {
        for (idx, mat_ref) in voxel_world.material_refs.iter() {
            let new_packed = mat_ref.resolve(&palette).pack();
            if voxel_world.nodes[*idx].color_data != new_packed {
                voxel_world.nodes[*idx].color_data = new_packed;
                voxel_world.mark_dirty_node(*idx);
            }
        }
        // Dirty SVO → re-upload → splat kernel re-reads color_data → PBR shows new colors
        // Glass panels re-read palette tints via glass_panel_uniform_system
    }
}
```

A palette change causes: dirty SVO upload → splat kernel emits new material_packed → tiled rasterizer evaluates PBR with new colors. One frame latency via double buffering.

### Presets

```rust
impl ThemePalette {
    pub fn dark_default() -> Self { /* dark voxels, cyan glass, warm key light */ }
    pub fn light_default() -> Self { /* bright voxels, subtle glass, cool lighting */ }
    pub fn high_contrast() -> Self { /* accessibility theme */ }
}
```

## Dependencies
- T1 (scaffold): Bevy App with resource registration
- T6e (PBR material): unpack_material reads color_data packed by this system
- T3 (liquid glass): Glass tints come from palette

## Acceptance Criteria
1. `ThemePalette` resource accessible from any Bevy system
2. Changing `palette.voxel_primary` updates all primary voxels → new PBR appearance
3. Roughness/metallic from palette visible: rough materials → broad specular, smooth → tight
4. Glass panel tints reflect palette changes
5. At least 2 preset themes (dark, light) with distinct visual appearance
6. Theme change propagates within 1 frame (via double-buffered SVO)

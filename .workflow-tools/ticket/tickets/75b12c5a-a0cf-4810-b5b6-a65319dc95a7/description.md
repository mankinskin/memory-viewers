# Motion-Blurred Particle Splatting

## Problem

Rendering hundreds of thousands of particles conventionally as point sprites creates hard edges that don't mix gracefully within the soft, liquid-glass/Voxel SDF aesthetic. Particles need volumetric glow and motion blur without requiring a separate render pass or heavy post-processing.

## Architecture

We unify particle rendering with Voxel Splatting. The same GPU Radix Sort and Tiled Rasterizer computing the world scene can ingest particles as dynamic splats.

### Particle Splat Struct
```rust
struct ParticleSplat {
    position: vec3<f32>,
    color: vec4<f32>,
    scale: f32,
    opacity: f32,
}
```

### Velocity AABB Stretch (Motion Blur)
To emulate motion blur physically in the splatting mathematics, we stretch the screen-space AABB along the `p.velocity` vector before executing AABB screen projection:

```wgsl
fn get_particle_stretch(p: Particle, view_mat: mat4x4<f32>) -> mat2x2<f32> {
    let velocity_stretch = length(p.velocity) * 0.01;
    let dir = normalize(p.velocity);
    // Erzeugt eine Ellipse, die in Bewegungsrichtung gestreckt ist
    return project_particle_to_screen(p.position, velocity_stretch, dir, view_mat);
}
```

### Liquid Glass Interaction
Because particles are rasterized via the Tiled Forward+ pipeline:
1. **Volumetric Glow:** Blending uses alpha and additive accumulation.
2. **Refraction & Caustics:** Particles behind `#liquid-glass` elements are distorted via physical refraction models. Splats effectively act as localized light-sources for glass roughness calculations, illuminating the UI panels when emitting nearby.
3. **RPG Materials:** Different visual particle styles (Ice, Fire) map to different parameter curves (roughness up/down, additive/multiply blends).

## Dependencies
- T2b (Render graph + pipeline) for inserting the dynamic particles into the Splat buffer uniformly.
- T6c (GPU radix sort) so these injected particles are depth-sorted alongside the SVO environment.

## Acceptance Criteria
1. Particles render inherently as splats utilizing the existing Splat renderer.
2. Fast-moving particles appear stretched and elongated (velocity AABB stretch).
3. The renderer successfully processes at least 100k "dust/magic" particles with motion sorting in < 2ms.
4. Particles naturally distort when passing behind Liquid Glass sections of the UI.

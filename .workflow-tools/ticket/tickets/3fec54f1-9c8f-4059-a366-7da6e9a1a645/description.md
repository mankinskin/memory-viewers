# Force Compute Shader & SVO Collision

## Problem

Particles must respond to complex physical forces (explosions, attraction, vortices) efficiently on the GPU. Furthermore, they need to physically collide with the Sparse Voxel Octree (SVO), bouncing realistically without causing a recursive or highly divergent performance hit inside the WebGPU compute shader.

## Architecture

### Force Events
Forces are injected from the Kernel via a `ForceEvent` buffer:
```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ForceEvent {
    origin: [f32; 3],
    radius: f32,
    strength: f32,
    force_type: u32, // 0: Explosion, 1: Attraction, 2: Vortex
    _padding: [f32; 2],
}
```

### Apply Forces Compute Block
A tight loop in WGSL calculates the influence of all active forces on a specific particle, incorporating linear attenuation (`falloff`) and evaluating Euler integration for native speed calculations. Evaluates 100k particles simultaneously using highly parallel compute dispatches.

### Stackless Ray-AABB SVO Collision
Instead of a simple voxel lookup, particles use motion vectors to perform a stackless ray march:
```wgsl
fn check_svo_collision(pos: vec3<f32>, next_pos: vec3<f32>) -> vec3<f32> {
    // Distance-based traversal through empty space
    // t += max(voxel.safe_distance, 0.001);
    // Returns normal on collision
}
```
If a normal `> 0.5` is returned, the shader applies reflection (`reflect(p.velocity, normal) * restitution`) and friction orthogonal to the normal.

### Liquid Glass Shockwaves
High-intensity `ForceEvents` instruct the kernel to pass global jitter values into the Dioxus UI pipeline, which sends noise variables to the Liquid Glass shader. This creates an optical "shake" or refative burst effect upon explosions.

## Dependencies
- T7a (VoxelWorld API) for access to the SVO octree structures and spatial logic.
- T1 (Scaffold) for Kernel to GPU setup and `bytemuck` POD representations.

## Acceptance Criteria
1. Particles are affected by Explosions (push), Attractions (pull), and Vortices (spin) based on the buffer state.
2. Particle paths intersecting solid SVO blocks evaluate stackless AABB raycasts and accurately determine bounding normals.
3. Particles reflect against SVO geometry with appropriate scalar dampening (friction & restitution).
4. Liquid Glass shader responds with a "shockwave jitter" when an explosion force triggers.

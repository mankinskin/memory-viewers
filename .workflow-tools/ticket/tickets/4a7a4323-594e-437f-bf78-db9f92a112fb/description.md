# Particle System & Physics Effects (Rollup)

## Problem

The engine needs a highly optimized particle system to depict voxel destruction, magic skills, and environmental effects. It must be governed by a generalized Force Compute scheme (explosions, vortices) and render seamlessly via Voxel Splatting with motion blur, fully interacting with the SVO tree and liquid glass.

## Scope

This ticket serves as a coordinator/rollup for the particle architecture. Sub-systems are broken down into:

- **T4a (Force Compute Shader & SVO Collision)**: Handles `ForceEvent` injections, the stackless SVO ray-AABB traversal algorithm, and physical feedback (bounce/friction).
- **T4b (Motion-Blurred Particle Splatting)**: Translates physics particles into dynamic, velocity-stretched Voxel Splats pushed into the tiled renderer.

## Dependencies
Epic (e7da478e-b18e-4551-a385-d39e81d09a41) depends on T4.

## Acceptance Criteria
1. All T4 sub-tickets are closed.

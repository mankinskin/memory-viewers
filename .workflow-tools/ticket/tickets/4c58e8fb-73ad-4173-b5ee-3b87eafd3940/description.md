# Feature: SVO LOD Management & GPU Streaming

## Problem

Rendering millions of voxels simultaneously via ray marching limits world size. We need a Level of Detail (LOD) architecture that leverages the SVO tree structure to dynamically group distant geometry into coarse blocks, paired with an LRU SpacetimeDB chunk stream to fit vast sandbox maps cleanly inside the WebGPU VRAM.

## Architecture

### SVO LOD Structure (WASM Kernel)
The SVO tree inherently represents geometry across multiple resolutions. We augment the data model so that parent nodes accumulate the visual average of their descendants.
```rust
pub struct OctreeNode {
    pub child_mask: u8,
    pub child_ptr: u32,
    pub avg_color: u32,     // LOD color for distant rendering
    pub max_roughness: u8,  // Used for glass refraction bounds
}
```

### LOD Ray Traversal (WGSL)
Within the SVO rendering pass, dynamically restrict the maximum depth level based on camera distance `t`.
```wgsl
fn trace_svo_lod(ray: Ray) -> vec4<f32> {
    var t = 0.0;
    for (var i = 0; i < 128; i++) {
        let p = ray.origin + ray.direction * t;
        
        // Target LOD: Near = Depth 10. Far = Depth 3.
        let target_lod = max(3.0, 10.0 - log2(t * detail_factor));
        let voxel = query_octree_at_lod(p, u32(target_lod));
        
        if (voxel.is_solid) { return voxel.avg_color; }
        t += voxel.size;
    }
    return background;
}
```

### Dynamic Texture / Buffer Streaming
1. The kernel maintains an LRU (Least Recently Used) mapping.
2. High-priority (close proximity / camera frustum) chunks are hot-swapped from the network (SpacetimeDB) into the GPU buffers.
3. Distant chunks fall back to their low LOD proxy and eject their level 10 leaf data.

### Liquid Glass Dioxus Optimization
* **Refraction LOD:** The UI kernel calculates panels that blur the background (Frosted glass). The ray-marcher is instructed to bypass deep detail tracing outright behind these panels (saving massive compute time), as the detail inevitably gets swallowed by the blur convolution.

## Dependencies
- T7a (VoxelWorld API) as it builds the foundational octree traversal.
- T2a (GPU buffer infra) to house the dynamic LRU buffer pool.
- T18 (Multiplayer Subscriptions) for pulling chunk ranges.

## Acceptance Criteria
1. `OctreeNode` includes `avg_color` and correctly renders LOD 3-9 proxies at a distance.
2. Rendering performance remains stable when expanding a simulated world from 256³ to 1024³ thanks to dynamic VRAM management and chunk eviction.
3. Obscuring large swaths of the scene using a heavily blurred Liquid Glass UI panel observably boosts framerate (triggers early LOD drop).

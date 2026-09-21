# World: Physics, Voxel Manipulation, Double-Buffered SVO Upload, and Rapier Bridge

> **Coordinator ticket** — this ticket has been decomposed into focused sub-tickets.
> Implementation work happens in the children; this ticket tracks overall completion.
>
> **Sub-tickets:**
> - **T7a** VoxelWorld API — Octree + Manipulation + Raycast — `c241f246-2fc2-47dc-8742-684f5b23f08f`
> - **T7b** Double-Buffered SVO Upload — `b29d49db-f9d9-4e53-a9ab-60de8bd25f80`
> - **T7c** Rapier Collision Bridge — `a5ab9013-94ce-4055-8d03-400236209958`
>
> This ticket is done when all three sub-tickets are closed.

---

## Problem

The context-editor needs physics, voxel manipulation, and a bridge from SVO to Rapier collision shapes. With voxel splatting, the SVO is the **structural authority** (physics + editing) while splats are visual-only. Voxel edits flow through the double-buffered SVO upload, and the voxel splat kernel re-creates visuals from fresh octree data every frame.

## Architecture: SVO ↔ Rapier Bridge + Double-Buffered Upload

### Two-Layer Collision (unchanged by splats)

| Layer | Purpose | Data Source |
|-------|---------|-------------|
| **SVO (GPU)** | Particle collision, splat generation source | Octree storage buffer |
| **Rapier (CPU)** | Character physics, rigid body dynamics | Collision shapes derived from SVO |

splats have NO collision role. They are purely visual.

### Voxel Manipulation API

```rust
impl VoxelWorld {
    pub fn set_voxel(&mut self, pos: IVec3, material: VoxelMaterial) {
        let (node_idx, depth) = self.descend_to(pos);
        self.nodes[node_idx].color_data = material.pack();
        self.mark_dirty(node_idx, depth);
    }

    pub fn apply_sdf_brush(&mut self, center: Vec3, radius: f32, material: VoxelMaterial) -> u32 { /* ... */ }

    pub fn carve_sdf_brush(&mut self, center: Vec3, radius: f32) -> u32 { /* ... */ }

    fn mark_dirty(&mut self, node_idx: usize, _depth: u32) {
        let byte_start = node_idx * std::mem::size_of::<OctreeNode>();
        let byte_end = byte_start + std::mem::size_of::<OctreeNode>();
        self.dirty_ranges.push((byte_start, byte_end));
    }

    pub fn take_dirty_ranges(&mut self) -> Vec<(usize, usize)> {
        self.dirty_ranges.sort_by_key(|r| r.0);
        let merged = merge_ranges(&self.dirty_ranges);
        self.dirty_ranges.clear();
        merged
    }
}
```

### Double-Buffered GPU Upload

The key insight: WASM writes dirty octree regions to the BACK buffer while the GPU reads the FRONT buffer for rendering. After upload completes, swap:

```rust
fn svo_upload_system(
    mut voxel_world: ResMut<VoxelWorld>,
    svo_buffer: Res<SvoDoubleBuffer>,
    render_queue: Res<RenderQueue>,
) {
    let ranges = voxel_world.take_dirty_ranges();
    let node_bytes = bytemuck::cast_slice(&voxel_world.nodes);
    let target = svo_buffer.write_target(); // BACK buffer

    for (start, end) in ranges {
        render_queue.write_buffer(target, start as u64, &node_bytes[start..end]);
    }
}

fn double_buffer_swap_system(mut svo_buffer: ResMut<SvoDoubleBuffer>) {
    svo_buffer.swap(); // BACK → FRONT for next frame's rendering
}
```

**Effect on splats**: The voxel splat kernel reads the FRONT buffer. After a swap, it reads the newly uploaded SVO data and generates fresh splats. Visual update latency = 1 frame (imperceptible at 120 FPS).

**No ruckler guarantee**: If a complex edit (e.g., 1000 voxels) takes longer than a frame, the GPU keeps rendering the last valid FRONT buffer at full FPS. The edit appears on the next swap.

### SVO → Rapier Collision

```rust
fn rapier_rebuild_system(
    voxel_world: Res<VoxelWorld>,
    mut chunks: Query<(&mut VoxelChunkCollider, &mut Collider)>,
) {
    for (mut chunk, mut collider) in chunks.iter_mut() {
        if chunk.dirty {
            *collider = rebuild_chunk_collider(&voxel_world, chunk.chunk_pos, CHUNK_SIZE);
            chunk.dirty = false;
        }
    }
}
```

### Performance

| Operation | Target |
|-----------|--------|
| Dirty upload (5-radius brush) | < 1ms (partial `write_buffer`) |
| Double buffer swap | < 0.01ms (pointer flip) |
| Rapier chunk rebuild | < 2ms (amortized, only dirty chunks) |
| Visual update after edit | 1 frame (next splat generation pass) |

## Scope

### Rust: VoxelWorld API (`src/svo/`)
- `set_voxel()`, `remove_voxel()`, `apply_sdf_brush()`, `carve_sdf_brush()`
- Dirty-range tracking and merging
- Position iteration (sphere, chunk, AABB)

### Rust: Double-Buffered Upload (`src/svo/upload.rs`)
- `svo_upload_system` writing to BACK buffer
- `double_buffer_swap_system`
- Buffer resize when octree grows (recreate both front and back)

### Rust: Rapier Bridge (`src/physics/`)
- `VoxelChunkCollider` component
- `rebuild_chunk_collider()` with greedy box merging
- `rapier_rebuild_system`
- Rapier plugin configuration

## Dependencies
- T1 (scaffold): VoxelWorld resource, svo/ module
- T2 (render init): SvoDoubleBuffer resource
- T6 (3D scene): voxel splat kernel reads FRONT buffer
- T8 (character): Character controller uses Rapier colliders

## Acceptance Criteria
1. `set_voxel()` modifies the octree and marks the region dirty
2. Upload goes to BACK buffer; GPU renders from FRONT buffer (no stalls)
3. After swap, voxel splat kernel produces updated visuals (verify color change visible)
4. Complex edit (1000 voxels) does not drop frames — GPU continues on stale FRONT buffer
5. Dirty upload < 1ms for a 5-radius sphere
6. Rapier chunk colliders rebuild when voxels change
7. Rigid body dropped into scene lands on voxel surface
8. Character stands on voxel terrain via Rapier

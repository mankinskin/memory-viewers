# [context-editor][SDF-DAG] Phase 3: 4D Spatio-Temporal DAG — Keyframed SDF Animation & Replay

## Problem

The current renderer has no concept of time within the voxel structure. Physics and animation
produce a new DAG root each tick, but these are independent snapshots — identical unchanged
regions are re-traversed and re-uploaded every frame. There is no mechanism for:

- Keyframed SDF deformation: smoothly morphing a sphere into a rounded box over N frames.
- Temporal DAG sharing: two frames with identical spatial octants share zero additional memory.
- Replay: recording a session and reproducing it pixel-exact from a serialized edit log.
- Spatio-temporal LOD: coarse-grained time sampling for distant or occluded geometry.

## Design

### 4D Timeline Structure

```rust
// kernel/src/timeline/mod.rs
#[derive(Resource)]
pub struct VoxelTimeline {
    /// The shared 3D DAG node pool — nodes are reused across time slices.
    pub dag: VoxelDag,
    /// Keyframes: (tick, root_slot). Root slots are ref-counted in dag.pool.
    /// Sparse — only ticks where keyframe_interval divides tick, or explicit snapshots.
    pub keyframes: Vec<(u64, u32)>,
    /// Ordered sequence of timed edit operations. Full replay reproduces any state.
    pub edit_log: Vec<TimedEdit>,
    /// Current playback tick.
    pub current_tick: u64,
    /// Keyframe interval (default: 60 ticks = 1s at 60Hz physics).
    pub keyframe_interval: u64,
}

pub struct TimedEdit {
    pub tick: u64,
    pub op: EditOp,
}

pub enum EditOp {
    SetAtom { pos: IVec3, atom: AtomDescriptor },
    RemoveAtom { pos: IVec3 },
    SetAtomsDeform { pos: IVec3, deform: SdfDeform },
    BatchSet { ops: Vec<(IVec3, AtomDescriptor)> },
}
```

### Temporal DAG Sharing

Temporal compression follows directly from Phase 2 COW semantics:

- If tick N and tick N-1 have the same DAG root slot: **zero** additional memory.
- If a small region changes: only the modified path (root → leaf) allocates new slots;
  all other subtrees share their slots with the previous tick's tree.

Compression ratio example: a 600-tick animation (10s at 60Hz) of a 1M-voxel scene where
0.1% of voxels move per tick:
- Full snapshots: 600 × 1M nodes × 8 bytes = 4.8 GB
- Temporal DAG: 600 × 0.001 × 1M × depth(≈8) × 8 bytes ≈ 38 MB

### SDF Keyframe Deformation

Atoms carry optional deformation parameters stored in the atom pool extension:

```rust
pub struct SdfDeform {
    /// Target coefficients for each atom field (linear interpolation target).
    pub target_coeffs: [f32; 8],
    /// Target opacity (for transparent atom morphing).
    pub target_opacity: f32,
    /// Duration of the morph in ticks.
    pub duration_ticks: u32,
    /// Easing: 0 = linear, 1 = ease-in-out, 2 = spring.
    pub easing: u8,
}
```

GPU-side interpolation for between-keyframe rendering:

```wgsl
// Two root buffers uploaded: keyframe_a (current) and keyframe_b (next).
// Blend factor t in [0,1] from a uniform buffer.
fn interpolate_atom_payload(type_id: u32, idx_a: u32, idx_b: u32, t: f32) -> AtomResult {
    // All f16 coefficient fields are linearly interpolated.
    // Orientation fields use shortest-path slerp.
    let pa = read_atom_payload(type_id, idx_a);
    let pb = read_atom_payload(type_id, idx_b);
    return eval_interpolated(type_id, mix_payload(pa, pb, t));
}
```

The voxel splat kernel uploads two node_positions buffers (one per keyframe root) and a
blend uniform. Each leaf resolves its atom by interpolating between the two keyframe payloads.

### Replay

```rust
impl VoxelTimeline {
    /// Seek to `target_tick` by rewinding to the nearest keyframe before it,
    /// then replaying edit_log ops from that checkpoint.
    pub fn seek(&mut self, target_tick: u64) {
        let (base_tick, base_root) = self.nearest_keyframe_before(target_tick);
        // Reset DAG root to base keyframe (cheap — root slot ref already held).
        self.dag.pool.release(self.dag.root);
        self.dag.pool.retain(base_root);
        self.dag.root = base_root;
        // Replay edits from base_tick (exclusive) to target_tick (inclusive).
        for edit in self.edit_log.iter()
            .filter(|e| e.tick > base_tick && e.tick <= target_tick)
        {
            self.dag.apply_edit_op(&edit.op);
        }
        self.current_tick = target_tick;
    }

    /// Record current DAG root as a keyframe at current_tick.
    /// Retains the root slot so it survives future edits.
    pub fn record_keyframe(&mut self) {
        self.dag.pool.retain(self.dag.root);
        self.keyframes.push((self.current_tick, self.dag.root));
    }

    /// Serialise the edit log to CBOR bytes for persistent replay storage.
    pub fn serialize_edit_log(&self) -> Vec<u8>;
    /// Deserialise and replay from previously serialised bytes.
    pub fn deserialize_and_replay(data: &[u8]) -> Self;
}
```

Seek latency bound: with 60-tick keyframe spacing, `seek(T)` replays at most 60 `EditOp`s.
For the expected 0.1% mutation rate and `set_atoms_batch`, this is < 5ms on a modern CPU.

### Physics Integration

The physics tick loop drives `VoxelTimeline`:

```rust
fn physics_tick_system(
    mut timeline: ResMut<VoxelTimeline>,
    physics: Res<PhysicsState>,
    time: Res<Time>,
) {
    let ops = physics.step(time.delta_seconds(), &timeline.dag);
    // Record every op in the edit log for replay.
    for op in ops {
        timeline.edit_log.push(TimedEdit { tick: timeline.current_tick, op: op.clone() });
        timeline.dag.apply_edit_op(&op);
    }
    timeline.current_tick += 1;
    if timeline.current_tick % timeline.keyframe_interval == 0 {
        timeline.record_keyframe();
    }
}
```

Integration with SpacetimeDB multiplayer (from `multiplayer_backend`): each server tick
produces a batch of `EditOp`s authoratively; clients receive the edit log via SSE and apply
`seek` to stay in sync.

### Spatio-Temporal LOD

Extend `LodParams` with a temporal resolution parameter:

```rust
pub struct LodParams {
    // ... existing spatial LOD fields ...
    /// Skip temporal interpolation for voxels farther than this camera-distance.
    /// Beyond this threshold, snap to nearest keyframe (no SDF morph blend).
    pub temporal_lod_distance: f32,
}
```

Beyond `temporal_lod_distance`, the splat kernel uses a single root (nearest keyframe) rather
than dual-root interpolation — reducing GPU bandwidth and compute for distant geometry.

## Implementation Plan

1. **`kernel/src/timeline/mod.rs`** (new): `VoxelTimeline`, `TimedEdit`, `EditOp`, `SdfDeform`.
   Implement `seek`, `record_keyframe`, `apply_edit_op`, serialize/deserialize CBOR.
2. **`kernel/src/timeline/interpolation.rs`** (new): Atom coefficient interpolation (linear
   for scalars, shortest-path slerp for orientations). Easing functions (linear, ease-in-out,
   spring).
3. **`kernel/src/render/voxel_splat_kernel.wgsl`**: Add dual-root mode — second `node_positions`
   buffer + blend factor uniform. Kernel dispatches `interpolate_atom_payload` for leaves.
4. **`kernel/src/svo/upload.rs`**: Extend to support dual-root upload: upload `node_positions_a`
   (current keyframe root) and `node_positions_b` (next keyframe root) + blend factor uniform.
5. **`kernel/src/svo_lod.rs`**: Add `temporal_lod_distance` check — disable dual-root blend
   for splats beyond distance threshold (spatial LOD already reduces these to parent nodes).
6. **`kernel/src/physics/mod.rs`** (extend): Hook `physics_tick_system` into `VoxelTimeline`.
   Integrate with SpacetimeDB tick loop event stream.
7. **Tests**:
   - Two identical consecutive frames: `record_keyframe` followed by `record_keyframe` with no
     edits in between produces zero new node allocations.
   - Seek correctness: `seek(50)` after recording 100 ticks reproduces the same DAG root as
     continuous forward simulation to tick 50.
   - Compression ratio: 600-tick animation with 0.1% mutation rate stores < 10% of naive
     full-snapshot memory.
   - SDF morph: sphere (tick 0) → rounded box (tick 30) interpolation produces smooth results
     with no NaN/Inf in intermediate frames.
   - CBOR round-trip: serialize → deserialize → replay produces bit-identical output.
8. **Bench**: `seek` latency < 10ms for 1M-voxel scene with 60-tick keyframe spacing.

## Acceptance Criteria

1. Two consecutive identical frames (no edits) share 100% of DAG node slots — zero new
   allocations between keyframes.
2. A 600-tick animation of a 1M-voxel scene with 0.1% mutation rate stores < 10% of the
   memory a naive full-snapshot approach would require.
3. `seek(T)` produces a pixel-identical result to continuous forward simulation to tick T.
4. SDF atom morphing (sphere → rounded box over 30 ticks) renders a smooth, artifact-free
   interpolation with no discontinuities at voxel boundaries.
5. Edit log serializes to CBOR; deserialization round-trips correctly and replay from file
   reproduces the original session.
6. SpacetimeDB-driven physics produces a valid `VoxelTimeline` with correct keyframe spacing
   and correct client-side seek from received edit log.
7. GPU dual-root SDF interpolation adds ≤ 1ms overhead vs. single-root rendering at 1080p.
8. Temporal LOD: voxels beyond `temporal_lod_distance` snap to nearest keyframe without
   visual popping artifacts (blend factor snaps at temporal_lod_distance, not abruptly).

## Dependencies

- Phase 2 (DAG-Persistent Edit Operations with Hash Consing) must be complete.
  - `VoxelDag` with `NodePool` and COW semantics is the foundation for temporal sharing.
  - `diff_roots` is reused to compute per-frame GPU upload deltas.
  - `apply_edit_op` is extended here to support `SetAtomsDeform`.
- Phase 1 (Per-Voxel SDF Atom Type System) must be complete.
  - `SdfDeform` extends the Phase 1 atom coefficient layout.
  - Interpolation functions are defined per atom type (sphere radius lerps, orientation slerps).

# [context-editor][SDF-DAG] GPU SDF Collision & Force Kernel — DAG Traversal Physics

## Problem

The current physics pipeline has two major gaps:

1. **`check_svo_collision` in `force_compute.wgsl` is a stub** returning `vec3(0)` — particles
   and entities never actually collide with voxel geometry on the GPU. The TODO comment confirms
   this is unfinished.

2. **Rapier chunk colliders (`physics/mod.rs`) are coarse approximations**: greedy box-merged
   compound colliders from SVO occupancy lose all sub-voxel SDF shape information. A sphere atom
   is treated as a filled cube for collision purposes.

With Phase 1 adding heterogeneous SDF atoms (spheres, toruses, glyphs) and Phase 2 introducing
the hash-consed DAG, the physics kernel must traverse the actual DAG structure and evaluate the
per-atom SDF for precise collision. The same `evaluate_atom` + `sdf_gradient` functions from the
rendering pipeline are reused — zero-copy, no separate collision mesh.

## Design

### Architecture: Zero-Copy Physics/Rendering

The same GPU storage buffers are bound to both the rendering pipeline and the physics compute
shader:

```
@group(0) @binding(0) var<storage, read>       dag_nodes:    array<u32>;   // SVO/DAG node buffer
@group(1) @binding(0..7) var<storage, read>     atom_pool_*:  array<u32>;   // per-type atom pools
@group(2) @binding(0) var<storage, read_write>  particles:    array<Particle>;
@group(2) @binding(1) var<storage, read>        forces:       array<ForceEventGpu>;
@group(2) @binding(2) var<uniform>              physics_params: PhysicsParams;
```

No data is copied between rendering and physics. Both pipelines read the same `dag_nodes` and
`atom_pool_*` buffers.

### DAG Traversal for Collision (`get_sdf_at`)

Iterative DAG descent from root to leaf, evaluating the SDF at the query position:

```wgsl
struct CollisionResult {
    dist:        f32,       // signed distance (negative = inside geometry)
    normal:      vec3f,     // SDF gradient = collision normal
    friction:    f32,       // from atom material properties
    hardness:    f32,       // surface hardness (controls restitution)
}

fn get_sdf_at(pos: vec3f, root_idx: u32) -> CollisionResult {
    var node_idx = root_idx;
    var node_origin = vec3f(0.0);
    var node_half = world_half_size;

    // Iterative descent — max depth = DAG depth (typically 10–14 levels)
    for (var depth = 0u; depth < MAX_DEPTH; depth++) {
        let node = dag_nodes[node_idx * 2u];        // child_pointer
        let atom_ref = dag_nodes[node_idx * 2u + 1u]; // atom_ref (leaf data)

        let is_leaf = (node & LEAF_FLAG) != 0u;
        if is_leaf {
            // Evaluate the atom SDF at this leaf
            let local_p = pos - node_origin;
            let result = evaluate_atom(atom_ref, local_p, node_half);
            let normal = sdf_gradient(atom_ref, local_p, node_half);
            let phys = extract_physics_properties(atom_ref);
            return CollisionResult(result.dist, normal, phys.friction, phys.hardness);
        }

        // Interior node: determine which octant contains the query point
        let child_half = node_half * 0.5;
        let octant = select_octant(pos, node_origin);
        let child_mask = (node >> CHILD_MASK_SHIFT) & 0xFFu;

        // Check if the target octant has a child
        if (child_mask & (1u << octant)) == 0u {
            // Empty octant — no geometry here
            return CollisionResult(node_half, vec3f(0.0, 1.0, 0.0), 0.0, 0.0);
        }

        // Compute child index (popcount of lower bits in child mask)
        let child_offset = countOneBits(child_mask & ((1u << octant) - 1u));
        node_idx = (node & CHILD_PTR_MASK) + child_offset;
        node_origin = node_origin + octant_offset(octant, child_half);
        node_half = child_half;
    }

    return CollisionResult(0.0, vec3f(0.0, 1.0, 0.0), 0.0, 0.0); // fallback
}
```

### Collision Detection & Response

The physics compute shader runs per-particle, checking collision against the DAG:

```wgsl
@compute @workgroup_size(64)
fn physics_main(@builtin(global_invocation_id) gid: vec3u) {
    let idx = gid.x;
    if idx >= arrayLength(&particles) { return; }

    var p = particles[idx];
    var accel = vec3f(0.0, -9.81, 0.0); // gravity

    // Apply external forces (explosion, attraction, vortex — existing system)
    for (var f = 0u; f < force_count; f++) {
        accel += apply_force(p.position, p.velocity, forces[f]);
    }

    // SDF collision with DAG
    let col = get_sdf_at(p.position, dag_root_idx);
    if col.dist < 0.0 {
        // Penetration — push out along gradient normal
        let penetration = -col.dist;
        let penalty = col.normal * penetration * physics_params.stiffness;

        // Velocity reflection with friction and restitution
        let v_normal = dot(p.velocity, col.normal) * col.normal;
        let v_tangent = p.velocity - v_normal;
        let restitution = 1.0 - col.hardness; // hardness → inelastic
        let friction_factor = max(0.0, 1.0 - col.friction * physics_params.dt);

        p.velocity = v_tangent * friction_factor - v_normal * restitution;
        p.position += col.normal * penetration; // depenetrate
        accel += penalty;
    }

    // Euler integration
    p.velocity += accel * physics_params.dt;
    p.position += p.velocity * physics_params.dt;

    // Damping
    p.velocity *= (1.0 - physics_params.damping * physics_params.dt);

    particles[idx] = p;
}
```

### Material-Dependent Physics Properties

Physical properties are extracted from the atom pool based on type:

```wgsl
struct PhysicsProperties {
    friction:    f32,    // 0.0 = frictionless ice, 1.0 = sticky rubber
    hardness:    f32,    // 0.0 = soft/bouncy, 1.0 = rigid/inelastic
    density:     f32,    // kg/voxel for mass computation
    restitution: f32,    // coefficient of restitution
}

fn extract_physics_properties(atom_ref: u32) -> PhysicsProperties {
    let type_id = atom_ref >> 24u;
    let pool_idx = atom_ref & 0xFFFFFFu;

    switch type_id {
        // Full-physics atoms store properties directly in extended payload
        case 7u: { return unpack_fullphys_properties(pool_idx); }
        // Default properties for atoms without explicit physics data
        default: {
            return PhysicsProperties(
                0.5,  // default friction
                0.5,  // default hardness
                1.0,  // default density
                0.3,  // default restitution
            );
        }
    }
}
```

Atoms of type `0x07` (FullPhysics) carry explicit friction, hardness, density, and restitution
in their extended 128-bit pool entry. All other atom types use sensible defaults but can be
overridden by wrapping them in a FullPhysics parent (see Phase 1 `AtomDescriptor::FullPhysics`).

### Glyph Collision

MSDF glyph atoms participate in physics — the collision kernel evaluates `eval_glyph` for
the SDF distance and gradient. Particles bounce off the letter silhouette:

```
Glyph "A" → particles collide with the triangular profile and serif edges.
Extrusion depth determines Z-axis collision boundary.
```

This works automatically because `get_sdf_at` calls `evaluate_atom` which dispatches to
`eval_glyph` for type `0x05`. No special-case collision code is needed.

### CPU-Side Mass Properties

For ECS rigid bodies that interact with Rapier (player, projectiles, vehicles), the CPU
computes mass properties from the SDF atom data:

```rust
// kernel/src/physics/mass.rs
pub fn compute_mass_properties(world: &VoxelWorld, chunk: IVec3) -> MassProperties {
    let mut total_mass = 0.0f32;
    let mut center_of_mass = Vec3::ZERO;
    let voxel_volume = world.voxel_size().powi(3);

    for pos in world.iter_chunk(chunk) {
        if let Some(atom) = world.get_atom(pos) {
            let density = atom.density_or_default(1.0);
            let mass = density * voxel_volume;
            total_mass += mass;
            center_of_mass += pos.as_vec3() * mass;
        }
    }

    if total_mass > 0.0 {
        center_of_mass /= total_mass;
    }

    MassProperties { mass: total_mass, center_of_mass, /* inertia tensor from voxel positions */ }
}
```

### PhysicsParams Uniform

```rust
// kernel/src/physics/params.rs
#[repr(C)]
pub struct PhysicsParams {
    pub dt: f32,
    pub stiffness: f32,     // penalty force multiplier (e.g. 1000.0)
    pub damping: f32,       // velocity damping (e.g. 0.01)
    pub gravity: [f32; 3],  // world gravity direction and magnitude
    pub dag_root_idx: u32,  // root node index into dag_nodes buffer
    pub max_depth: u32,     // maximum DAG traversal depth
}
```

### Integration with Existing Force System

The existing `ForceEvent` system (explosion, attraction, vortex) in `force_compute.rs` / `.wgsl`
is preserved. The new collision kernel is inserted **after** external force accumulation but
**before** Euler integration in the same compute dispatch. No separate compute pass is needed.

```
Frame timeline:
  1. CPU: queue ForceEvents, upload ForceBuffer
  2. GPU dispatch: physics_main
     a. accumulate gravity + external forces (existing)
     b. get_sdf_at → collision detection (NEW)
     c. penalty force + depenetration (NEW)
     d. Euler integration (existing)
  3. GPU dispatch: voxel_splat_kernel (rendering)
```

## Implementation Plan

1. **`kernel/src/render/atom_sdf.wgsl`** (from Phase 1): Confirm `evaluate_atom` and
   `sdf_gradient` are importable from physics shaders.
2. **`kernel/src/render/force_compute.wgsl`**: Replace the `check_svo_collision` stub with
   `get_sdf_at` DAG traversal. Add `extract_physics_properties`. Integrate penalty force
   and depenetration into the existing compute entry point.
3. **`kernel/src/physics/params.rs`** (new): `PhysicsParams` uniform struct, upload system.
4. **`kernel/src/physics/mass.rs`** (new): `compute_mass_properties` from atom density.
5. **`kernel/src/physics/mod.rs`**: Add `physics_params_upload_system`. Keep existing Rapier
   rebuild code for now (hybrid: Rapier for entity-entity, SDF-DAG for particle-voxel).
6. **`kernel/src/force_compute.rs`**: Bind `dag_nodes` and `atom_pool_*` buffers into the
   force compute bind group. Add `PhysicsParams` uniform binding.
7. **Tests**:
   - Unit: sphere atom at origin, particle at surface → `get_sdf_at` returns dist ≈ 0.
   - Unit: particle inside sphere atom → penalty force pushes outward along gradient.
   - Unit: particle above floor plane → gravity pulls down, collision stops at surface.
   - Unit: glyph atom collision — particle deflects off letter silhouette.
   - Integration: 1000 particles dropped onto mixed-atom terrain — no fall-through after 10s.

## Acceptance Criteria

1. `check_svo_collision` stub is replaced with working `get_sdf_at` DAG traversal.
2. Particles collide with sphere atoms along the sphere surface, not the voxel cube.
3. Particles collide with glyph atoms along the letter silhouette.
4. `FullPhysics` atoms with high friction cause particles to slow on contact.
5. `FullPhysics` atoms with low hardness (bouncy) cause particles to bounce higher.
6. Zero-copy: rendering and physics use the same `dag_nodes` and `atom_pool_*` GPU buffers —
   no data duplication.
7. Existing force types (explosion, attraction, vortex) continue to work alongside collision.
8. Penalty force depenetration: particles never tunnel through geometry more than 1 voxel deep.
9. Physics compute dispatches in < 2ms for 100K particles at DAG depth 12.
10. No Rapier regression: entity-entity collisions (player, projectiles) still use Rapier.

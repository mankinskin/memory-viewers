# [context-editor][SDF-DAG] Phase 1: Per-Voxel SDF Atom Type System with Typed Dispatch

## Problem

The current `OctreeNode` stores only a flat `color_data: u32` (R8G8B8 + roughness5 + metallic1)
in leaf nodes, limiting every voxel to an axis-aligned box with a single solid material. This prevents:

- Curved or organic sub-voxel shapes (spheres, toruses, rounded boxes)
- Semi-transparency or volumetric density per voxel
- Glyph or parametric SDF atoms (character rendering, symbolic objects)
- Smooth blending between adjacent heterogeneous SDF surfaces (smooth-min)
- Per-atom orientation and local transform (rotated primitives within voxels)
- Material-dependent physical properties (friction, hardness) embedded in the atom

## Design

### Atom Reference (replaces `color_data`)

```
Bit layout of OctreeNode::atom_ref  (u32, replaces color_data)
  Bits  0–23: index into the typed atom coefficient pool (16M slots)
  Bits 24–31: atom type ID (256 types)

Special case — backward compat:
  type_id == 0 → "Legacy" atom; bits 0–29 carry packed R8G8B8+roughness5+metallic1
  No pool lookup needed for legacy atoms.
```

### 64-Bit Atom Pool Entry Encoding

Each atom pool entry is a compact 64-bit (2× u32) structure. The two u32 words are
bit-packed to maximise information density while keeping GPU access efficient.

```
data_high (u32):
  Bits 24–31: Type ID (8 bits, 256 possible SDF formulae)
  Bits  8–23: Scale/Shape (16 bits, quantised radius/width/height/curvature)
  Bits  0–7:  Orientation index (8 bits, 256 quantised orientations)
              OR packed Euler angles (3+3+2 bits for low-DOF shapes)

data_low (u32):
  Bits  8–31: Material/Aux (24 bits):
              - RGB8 colour (or palette index)
              - OR physical properties: friction[5] + hardness[5] + emission[5] + reserved[9]
  Bits  0–7:  Sub-type flags: roughness[5] + metallic[1] + transparent[1] + physical[1]
```

The `physical` flag bit selects whether material/aux encodes visual (RGB8) or physical
(friction, hardness, emission) properties. When physical properties are needed alongside
visual colour, the atom type must store them in an extended pool entry (see below).

### Extended Pool Entries (> 64 bits)

Some atom types require more than 64 bits. These use a fixed stride per type:

| ID | Type | Entry size | Layout |
|----|------|-----------|--------|
| `0x04` | Polynomial SDF | 128 bits (4× u32) | 8× f16 coefficients + material u32 |
| `0x05` | Glyph SDF (MSDF) | 128 bits (4× u32) | atlas_uv[4× u16] + extrusion f16 + material u32 |
| `0x07` | Full-physics atom | 128 bits (4× u32) | base 64-bit + friction f16 + hardness f16 + density f16 + restitution f16 |

### Atom Type Registry (initial set)

| ID | Name | Pool stride | Payload |
|----|------|-------------|---------|
| `0x00` | Legacy box | 0 (inline) | R8G8B8 + roughness5 + metallic1 (bits 0–29 of atom_ref) |
| `0x01` | Sphere | 8 bytes | `radius: f16`, `center_offset: [i8; 3]`, `orientation: u8`, `material: u16` |
| `0x02` | Rounded box | 8 bytes | `half_extents: [f16; 3]`, `corner_radius: f16`, compressed by scale bits |
| `0x03` | Torus | 8 bytes | `major_r: f16`, `minor_r: f16`, `orientation: u8`, `material: u16` |
| `0x04` | Polynomial SDF | 16 bytes | 8× `f16` coefficients, `material: u32` |
| `0x05` | Glyph SDF (MSDF) | 16 bytes | `atlas_uv: [u16; 4]`, `extrusion: f16`, `material: u32` |
| `0x06` | Semi-transparent | 8 bytes | `density: f16`, `absorption: [u8; 3]`, `scatter: u8`, `color: u16` |
| `0x07` | Full-physics atom | 16 bytes | Any base shape + `friction: f16`, `hardness: f16`, `density: f16`, `restitution: f16` |

### Glyph SDF Atlas (MSDF)

Character glyphs are rendered as multi-channel signed distance field (MSDF) atoms:

- A global GPU texture atlas holds pre-computed MSDF data for all loaded glyphs.
- The `atlas_uv` field indexes into this atlas. The SDF evaluation samples the atlas
  at the local 2D projection of the query point.
- `extrusion: f16` gives the glyph depth (solid character with Z thickness).
- Physics: a glyph atom is a real physical barrier — particles bounce off the letterform.
- Rendering: SDF mathematics reconstructs crisp edges at any zoom level.

```wgsl
fn eval_glyph(pool_idx: u32, local_p: vec3f) -> AtomResult {
    let data = atom_pool_glyph[pool_idx * 4u]; // 4× u32 = 16 bytes
    let uv = unpack_atlas_uv(data);
    let extrusion = unpack_f16(data, 8u);
    let d2d = sample_msdf_atlas(local_p.xy, uv);
    let d3d = max(d2d, abs(local_p.z) - extrusion);
    let mat = unpack_material(data);
    return AtomResult(d3d, mat, 1.0);
}
```

### Rust API Changes

```rust
// kernel/src/atom.rs
pub enum AtomDescriptor {
    Legacy(VoxelMaterial),
    Sphere { radius: f32, center_offset: Vec3, orientation: Quat, material: MaterialDescriptor },
    RoundedBox { half_extents: Vec3, corner_radius: f32, material: MaterialDescriptor },
    Torus { major_r: f32, minor_r: f32, orientation: Quat, material: MaterialDescriptor },
    PolynomialSdf { coeffs: [f32; 8], material: MaterialDescriptor },
    GlyphSdf { atlas_uv: [f32; 4], extrusion: f32, material: MaterialDescriptor },
    SemiTransparent { density: f32, absorption: Vec3, scatter: f32, color: Vec3 },
    FullPhysics {
        base: Box<AtomDescriptor>,
        friction: f32,
        hardness: f32,
        density: f32,
        restitution: f32,
    },
}

pub struct MaterialDescriptor {
    pub base_color: Vec3,
    pub roughness: f32,
    pub metallic: bool,
    pub opacity: f32,
}

pub struct AtomPool {
    /// Per-type flat byte arrays. pool[type_id] = flat encoded payloads.
    /// Entry stride is fixed per type (8 or 16 bytes).
    pub pools: Vec<Vec<u8>>,
    /// Stride in bytes for each pool type.
    pub strides: Vec<usize>,
    /// Dirty byte ranges per type for incremental GPU upload.
    pub dirty: Vec<Vec<(usize, usize)>>,
}

impl AtomPool {
    /// Encode an AtomDescriptor into the correct per-type pool.
    /// Returns (type_id, pool_index) for packing into atom_ref.
    pub fn insert(&mut self, atom: AtomDescriptor) -> (u8, u32);

    /// Decode an atom back from (type_id, pool_index).
    pub fn get(&self, type_id: u8, pool_idx: u32) -> AtomDescriptor;
}

// VoxelWorld additions
impl VoxelWorld {
    pub fn set_atom(&mut self, pos: IVec3, atom: AtomDescriptor);
    pub fn remove_atom(&mut self, pos: IVec3);
    pub fn set_voxel(&mut self, pos: IVec3, mat: VoxelMaterial) {
        self.set_atom(pos, AtomDescriptor::Legacy(mat));
    }
}
```

### GPU Buffer Layout

```
GPU storage buffers (one per active atom type, variable stride):
  @group(1) @binding(0)  var<storage, read>  atom_pool_legacy:      array<u32>;
  @group(1) @binding(1)  var<storage, read>  atom_pool_sphere:      array<u32>;  // 2× u32 per atom
  @group(1) @binding(2)  var<storage, read>  atom_pool_rounded_box: array<u32>;  // 2× u32 per atom
  @group(1) @binding(3)  var<storage, read>  atom_pool_torus:       array<u32>;  // 2× u32 per atom
  @group(1) @binding(4)  var<storage, read>  atom_pool_polynomial:  array<u32>;  // 4× u32 per atom
  @group(1) @binding(5)  var<storage, read>  atom_pool_glyph:       array<u32>;  // 4× u32 per atom
  @group(1) @binding(6)  var<storage, read>  atom_pool_semitrans:   array<u32>;  // 2× u32 per atom
  @group(1) @binding(7)  var<storage, read>  atom_pool_fullphys:    array<u32>;  // 4× u32 per atom
  @group(1) @binding(8)  var               glyph_atlas:           texture_2d<f32>;
  @group(1) @binding(9)  var               glyph_sampler:         sampler;
```

### WGSL Dispatch (Rendering & Physics — shared evaluator)

```wgsl
struct SdfAtom {
    data_low:  u32,
    data_high: u32,
}

struct AtomResult {
    dist:     f32,
    normal:   vec3f,     // SDF gradient (for physics penalty forces & lighting)
    material: Material,
    opacity:  f32,
}

fn evaluate_atom(atom_ref: u32, local_p: vec3f, voxel_half: f32) -> AtomResult {
    let type_id  = atom_ref >> 24u;
    let pool_idx = atom_ref & 0xFFFFFFu;
    switch type_id {
        case 0u: { return eval_legacy(pool_idx, local_p, voxel_half); }
        case 1u: { return eval_sphere(pool_idx, local_p); }
        case 2u: { return eval_rounded_box(pool_idx, local_p); }
        case 3u: { return eval_torus(pool_idx, local_p); }
        case 4u: { return eval_polynomial(pool_idx, local_p); }
        case 5u: { return eval_glyph(pool_idx, local_p); }
        case 6u: { return eval_semitransparent(pool_idx, local_p); }
        case 7u: { return eval_fullphysics(pool_idx, local_p); }
        default: { return eval_legacy(pool_idx, local_p, voxel_half); }
    }
}

/// Transform query point into atom-local space using the 8-bit orientation index.
fn transform_to_local(p: vec3f, orientation_idx: u32) -> vec3f {
    let q = orientation_lut[orientation_idx];
    return rotate_by_quat(p, conjugate(q));
}

/// Compute SDF gradient via central differences (shared by physics and rendering).
fn sdf_gradient(atom_ref: u32, local_p: vec3f, voxel_half: f32) -> vec3f {
    let eps = 0.01;
    let dx = evaluate_atom(atom_ref, local_p + vec3f(eps, 0.0, 0.0), voxel_half).dist
           - evaluate_atom(atom_ref, local_p - vec3f(eps, 0.0, 0.0), voxel_half).dist;
    let dy = evaluate_atom(atom_ref, local_p + vec3f(0.0, eps, 0.0), voxel_half).dist
           - evaluate_atom(atom_ref, local_p - vec3f(0.0, eps, 0.0), voxel_half).dist;
    let dz = evaluate_atom(atom_ref, local_p + vec3f(0.0, 0.0, eps), voxel_half).dist
           - evaluate_atom(atom_ref, local_p - vec3f(0.0, 0.0, eps), voxel_half).dist;
    return normalize(vec3f(dx, dy, dz));
}
```

The `evaluate_atom` function is **shared between rendering (tiled rasterizer) and physics
(collision kernel)**. This is the zero-copy design: both the fragment shader and the physics
compute shader call the same function against the same GPU buffer.

The `AtomResult` now includes `normal` (SDF gradient), which the physics kernel uses for
penalty forces and the rasterizer uses for PBR lighting — no duplicate gradient computation.

### Smooth Blending

```wgsl
fn blend_neighborhood(center_result: AtomResult, neighbors: array<AtomResult, 6>,
                      blend_k: f32) -> AtomResult { ... }
```

Sample 6-neighborhood, apply `sdf_smooth_union` with `blend_radius = voxel_half * 0.25`
to avoid hard seams between voxel atoms.

The `sd_box` path in the fragment shader (current implementation) becomes the `eval_legacy` case —
fully backward compatible.

Semi-transparency: the existing `remaining_alpha` front-to-back loop already composites correctly;
atoms with `opacity < 1.0` let light pass through to deeper splats in the tile.

## Implementation Plan

1. **`kernel/src/atom.rs`** (new file): `AtomDescriptor`, `AtomPool`, `MaterialDescriptor`,
   64-bit encode/decode for all 8 types incl. stride-per-type. Unit tests for bit-packing.
2. **`kernel/src/svo/mod.rs`**: Rename leaf field `color_data` → `atom_ref`. Add `atom_pool`
   field to `VoxelWorld`. Add `set_atom`, `remove_atom`. Keep `set_voxel` as shim.
3. **`kernel/src/svo/upload.rs`**: Extend `svo_upload_system` to also upload `AtomPool` dirty
   ranges. New `AtomPoolBuffer` Bevy resource (array of GPU buffers, one per atom type).
4. **`kernel/src/gpu/mod.rs`**: Allocate per-type atom pool GPU storage buffers + glyph atlas
   texture + sampler. Add `atom_pool_bind_group` at group(1) in the rasterizer bind group layout.
5. **`kernel/src/render/atom_sdf.wgsl`** (new file): SDF evaluator functions for all 8 types +
   `evaluate_atom` dispatch + `sdf_gradient` + `blend_neighborhood` smooth-min blending +
   `transform_to_local` orientation lookup.
6. **`kernel/src/render/tiled_raster.wgsl`**: Import `atom_sdf.wgsl`. Replace the per-splat
   `unpack_material`+`sd_box` call with `evaluate_atom`. Add opacity to the blending formula.
7. **`kernel/src/render/voxel_splat_kernel.wgsl`**: Pass `atom_ref` (was `material_packed`)
   unchanged from leaf node through to `ProjectedSplat`.
8. **Glyph atlas**: Create `kernel/src/glyph_atlas.rs` — loads MSDF texture, manages atlas
   regions, provides `atlas_uv` coordinates for glyph atoms.
9. **Tests (cargo test)**:
   - Encode/decode round-trip for all 8 `AtomDescriptor` variants (including 64-bit packing).
   - SDF correctness: sphere at origin → `eval_sphere` returns 0 at surface, negative inside.
   - Legacy compat: `VoxelMaterial::new(255,0,0,16)` before and after produce same `atom_ref` bits.
   - Orientation quantisation: 256 orientations cover SO(3) with < 7° max deviation.
   - Glyph atom: atlas UV round-trips correctly through u16 packing.
10. **Visual smoke test**: render glyph "A" atom — visible character silhouette with extrusion.

## Acceptance Criteria

1. `AtomDescriptor::Sphere` renders as a smooth sphere — no cubic box artifacts at voxel boundary.
2. `AtomDescriptor::RoundedBox` shows beveled edges visible in close-up fragment shader output.
3. `AtomDescriptor::Torus` produces a donut silhouette when ray-marched within the voxel.
4. `AtomDescriptor::GlyphSdf` renders a crisp extruded character from the MSDF atlas.
5. `AtomDescriptor::SemiTransparent` blends correctly — geometry behind is visible with correct
   alpha attenuation.
6. `AtomDescriptor::FullPhysics` stores friction/hardness/density/restitution, readable by
   the physics collision kernel via the same atom pool buffer.
7. Legacy `VoxelMaterial` (type `0x00`) renders identically to pre-Phase-1 output — no visual
   regression.
8. Smooth-min blending: two adjacent sphere atoms merge into a blob rather than a hard seam.
9. 64-bit atom encoding round-trips correctly for all types (orientation, scale, material).
10. GPU atom pool upload is incremental — only dirty pool entries are uploaded per frame.
11. `sdf_gradient` returns correct surface normals for all atom types (verified against
    analytical normals for sphere and box).
12. Cargo unit tests pass for all encode/decode, SDF correctness, and orientation quantisation.
13. No performance regression: Tiled Forward+ remains < 5ms at 1080p for 1M legacy-type voxels.

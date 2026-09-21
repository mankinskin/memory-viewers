# Voxel Splat PBR Material System: Cook-Torrance/GGX, Compact u32 Material Encoding, BRDF LUT & Soft Shadows

## Problem

Every visible voxel splat needs physically-based shading. The material is packed into a single `u32` in the SVO's `color_data` field. This ticket provides the shared WGSL functions that the tiled rasterizer (T6d) calls per-pixel: `unpack_material()`, `evaluate_pbr()`, and BRDF LUT sampling. The goal is photorealistic voxel rendering with correct metallic/dielectric distinction, energy conservation, and soft shadows — all evaluated in screen-space fragment shaders.

## Scope

### Compact Material Encoding (u32)

The `OctreeNode.color_data` field packs all material parameters into 32 bits:

```
Bits 0–7:   R (8 bits, 0–255)
Bits 8–15:  G (8 bits, 0–255)
Bits 16–23: B (8 bits, 0–255)
Bits 24–28: Roughness (5 bits, 0–31 → 0.0–1.0)
Bits 29–29: Metallic (1 bit, 0 = dielectric, 1 = metallic)
Bits 30–31: Reserved (emission flag, translucency hint)
```

```wgsl
struct Material {
    base_color: vec3f,
    roughness: f32,
    metallic: f32,
}

fn unpack_material(packed: u32) -> Material {
    let r = f32(packed & 0xFFu) / 255.0;
    let g = f32((packed >> 8u) & 0xFFu) / 255.0;
    let b = f32((packed >> 16u) & 0xFFu) / 255.0;
    let roughness = f32((packed >> 24u) & 0x1Fu) / 31.0;
    let metallic = f32((packed >> 29u) & 1u);
    return Material(vec3f(r, g, b), roughness, metallic);
}

fn pack_material(base_color: vec3f, roughness: f32, metallic: bool) -> u32 {
    let r = u32(clamp(base_color.r * 255.0, 0.0, 255.0));
    let g = u32(clamp(base_color.g * 255.0, 0.0, 255.0));
    let b = u32(clamp(base_color.b * 255.0, 0.0, 255.0));
    let rough_q = u32(clamp(roughness * 31.0, 0.0, 31.0));
    let metal_bit = select(0u, 1u, metallic);
    return r | (g << 8u) | (b << 16u) | (rough_q << 24u) | (metal_bit << 29u);
}
```

### Cook-Torrance/GGX BRDF

```wgsl
const PI: f32 = 3.14159265359;

fn evaluate_pbr(
    mat: Material,
    normal: vec3f,
    view_dir: vec3f,
    light_dir: vec3f,
    light_color: vec3f,
) -> vec3f {
    let h = normalize(view_dir + light_dir);
    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let n_dot_v = max(dot(normal, view_dir), 0.001);
    let n_dot_h = max(dot(normal, h), 0.0);
    let v_dot_h = max(dot(view_dir, h), 0.0);

    // F0: reflectance at normal incidence
    let f0 = mix(vec3f(0.04), mat.base_color, mat.metallic);

    // D: GGX/Trowbridge-Reitz normal distribution
    let alpha = mat.roughness * mat.roughness;
    let alpha2 = alpha * alpha;
    let d = ggx_distribution(n_dot_h, alpha2);

    // F: Schlick Fresnel approximation
    let f = fresnel_schlick(v_dot_h, f0);

    // G: Smith's geometry function (GGX)
    let g = geometry_smith(n_dot_v, n_dot_l, alpha);

    // Specular BRDF: D * F * G / (4 * NdotV * NdotL)
    let specular = (d * f * g) / max(4.0 * n_dot_v * n_dot_l, 0.001);

    // Diffuse: Lambertian, energy-conserved
    let k_s = f;
    let k_d = (vec3f(1.0) - k_s) * (1.0 - mat.metallic);
    let diffuse = k_d * mat.base_color / PI;

    return (diffuse + specular) * light_color * n_dot_l;
}

fn ggx_distribution(n_dot_h: f32, alpha2: f32) -> f32 {
    let denom = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    return alpha2 / (PI * denom * denom);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3f) -> vec3f {
    return f0 + (vec3f(1.0) - f0) * pow(1.0 - cos_theta, 5.0);
}

fn geometry_schlick_ggx(n_dot: f32, k: f32) -> f32 {
    return n_dot / (n_dot * (1.0 - k) + k);
}

fn geometry_smith(n_dot_v: f32, n_dot_l: f32, alpha: f32) -> f32 {
    let k = (alpha + 1.0) * (alpha + 1.0) / 8.0;  // direct lighting remapping
    return geometry_schlick_ggx(n_dot_v, k) * geometry_schlick_ggx(n_dot_l, k);
}
```

### BRDF LUT (Pre-computed Texture)

For image-based lighting (IBL), a 2D lookup texture indexed by `(NdotV, roughness)` encodes the split-sum integral:

```wgsl
@group(1) @binding(0) var brdf_lut: texture_2d<f32>;
@group(1) @binding(1) var brdf_sampler: sampler;

fn evaluate_ibl(
    mat: Material,
    normal: vec3f,
    view_dir: vec3f,
    env_irradiance: vec3f,
    env_prefiltered: vec3f,
) -> vec3f {
    let n_dot_v = max(dot(normal, view_dir), 0.0);
    let f0 = mix(vec3f(0.04), mat.base_color, mat.metallic);

    let brdf = textureSample(brdf_lut, brdf_sampler, vec2f(n_dot_v, mat.roughness));
    let specular = env_prefiltered * (f0 * brdf.x + brdf.y);

    let k_d = (1.0 - mat.metallic);
    let diffuse = k_d * mat.base_color * env_irradiance;

    return diffuse + specular;
}
```

The BRDF LUT is generated once at startup (128×128, RG16F format) using the standard Hammersley/importance-sampling compute shader.

### Soft Shadows (Screen-Space Approximation)

For v1, approximate shadow softness using the SDF distance field itself:

```wgsl
fn soft_shadow_factor(
    ray_origin: vec3f,
    light_dir: vec3f,
    max_dist: f32,
    k: f32,  // softness: 8.0 = sharp, 2.0 = very soft
) -> f32 {
    var shadow = 1.0;
    var t = 0.02;  // avoid self-intersection
    for (var i = 0u; i < 32u; i++) {
        let p = ray_origin + light_dir * t;
        let d = scene_sdf(p);  // query nearest voxel SDF
        if d < 0.001 { return 0.0; }  // hard shadow
        shadow = min(shadow, k * d / t);
        t += d;
        if t > max_dist { break; }
    }
    return shadow;
}
```

This produces penumbra-like softening near shadow edges without requiring a separate shadow map pass.

### Ambient Occlusion (Short-Range SDF)

```wgsl
fn sdf_ao(pos: vec3f, normal: vec3f) -> f32 {
    var ao = 0.0;
    var scale = 1.0;
    for (var i = 1u; i <= 5u; i++) {
        let sample_pos = pos + normal * f32(i) * 0.05;
        let d = scene_sdf(sample_pos);
        ao += (f32(i) * 0.05 - d) * scale;
        scale *= 0.5;
    }
    return clamp(1.0 - ao, 0.0, 1.0);
}
```

## Implementation Plan

1. Create `pbr_material.wgsl` as a shared shader include
2. Implement `unpack_material` / `pack_material` in both Rust (`VoxelMaterial`) and WGSL
3. Implement Cook-Torrance/GGX functions
4. Generate BRDF LUT texture at startup (compute shader or embedded pre-computed data)
5. Integrate `evaluate_pbr` into T6d's fragment shader
6. Add soft shadows and AO as optional quality tiers
7. Rust-side: ensure `VoxelMaterial` struct packs to same layout as WGSL `unpack_material`

## Dependencies
- T6a (voxel splat kernel): `material_packed` field in `VoxelSplat` output
- T6d (tiled rasterizer): calls `evaluate_pbr()` per-pixel, provides normal + view direction
- T2a (GPU buffer infra): BRDF LUT texture + sampler bind group

## Acceptance Criteria
1. `unpack_material` correctly extracts R8G8B8, 5-bit roughness, 1-bit metallic from u32
2. Cook-Torrance/GGX produces correct specular highlights: rough → broad, smooth → tight
3. Metallic/dielectric distinction visible: metals tint specular by base color, dielectrics have white specular
4. Energy conservation: diffuse + specular never exceeds incoming light
5. Fresnel effect visible at grazing angles (edge brightening)
6. BRDF LUT generated correctly (matches reference: bright diagonal for smooth metals)
7. Soft shadows produce visible penumbra near shadow edges
8. Total PBR evaluation < 0.5ms overhead per frame at 1080p

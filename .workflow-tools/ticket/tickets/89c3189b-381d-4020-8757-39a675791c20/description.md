# Skill System: Spell SDFs, Procedural Shader Effects & Volumetric Magic

## Problem

Magic spells in this RPG are not pre-canned animations — they are transient SDF volumes injected into the ray-marching loop, generating real-time volumetric lighting, refraction, and physical effects on the voxel world. Each spell category manipulates the render pipeline differently: gravity distortion warps rays, destruction erodes voxels, frost modifies glass roughness, and fire emits light into the scene.

## Architecture: Spells as Transient SDF + Shader Modifiers

### Spell Lifecycle

```
Player casts spell (client input)
    ↓
SpacetimeDB reducer: cast_spell(spell_type, direction, power)
    ↓
Server validates mana cost, cooldown, creates ActiveSpell row
    ↓
All clients in range receive ActiveSpell via subscription
    ↓
Client: spawn SpellEntity with SDF params + shader modifiers
    ↓
Ray-marching evaluates spell SDF each frame (GPU)
    ↓
Server tick: update spell position/lifetime, check SDF-vs-player collision
    ↓
Spell expires or hits target → server removes ActiveSpell row
    ↓
Client: despawn SpellEntity
```

### SpacetimeDB Tables

```rust
#[spacetimedb::table(name = active_spell, public)]
pub struct ActiveSpell {
    #[primary_key]
    #[auto_inc]
    pub spell_id: u64,
    pub caster: Identity,
    pub spell_type: u8,           // enum: Fireball, Shield, Frost, Gravity, Lightning
    pub position: (f32, f32, f32),
    pub direction: (f32, f32, f32),
    pub power: f32,
    pub radius: f32,
    pub spawn_tick: u64,
    pub lifetime_ticks: u64,
}
```

### Spell SDF Categories

```rust
pub enum SpellSdf {
    /// Sphere + FBM noise for fire/energy
    Fireball {
        center: Vec3,
        radius: f32,
        noise_scale: f32,
        emission_color: Vec3,
        emission_intensity: f32,
    },
    /// Sphere shell for defensive barrier
    Shield {
        center: Vec3,
        outer_radius: f32,
        thickness: f32,
        ior: f32,               // refractive index (glass-like)
        tint: Vec3,
    },
    /// Expanding sphere that modifies roughness
    Frost {
        center: Vec3,
        radius: f32,
        roughness_boost: f32,   // added to glass roughness in affected area
    },
    /// Sphere that warps ray direction (gravitational lensing)
    Gravity {
        center: Vec3,
        radius: f32,
        strength: f32,          // negative = repel, positive = attract
    },
    /// Cylinder beam (lightning bolt)
    Lightning {
        origin: Vec3,
        direction: Vec3,
        length: f32,
        radius: f32,
        branch_noise: f32,
    },
}
```

### WGSL Shader Integration

Spells are evaluated in the ray-marching loop alongside SVO, glass, and player SDFs:

```wgsl
struct SpellData {
    spell_type: u32,
    pos: vec3<f32>,
    dir: vec3<f32>,
    radius: f32,
    power: f32,
    time: f32,          // elapsed since spawn, for animation
    color: vec3<f32>,
    intensity: f32,
};

@group(3) @binding(0) var<storage, read> spells: array<SpellData>;
@group(3) @binding(1) var<uniform> spell_count: u32;

fn evaluate_spell_fireball(p: vec3<f32>, spell: SpellData) -> SpellHit {
    let d = sd_sphere(p - spell.pos, spell.radius);
    // FBM noise creates turbulent fire surface
    let noise = fbm_3d(p * 5.0 + spell.time * 3.0, 4u);
    let density = smoothstep(0.1, 0.0, d + noise * 0.3 * spell.radius);
    let emission = spell.color * spell.intensity * density;
    return SpellHit(d, emission, density);
}

fn evaluate_spell_shield(p: vec3<f32>, spell: SpellData) -> SpellHit {
    let d_outer = sd_sphere(p - spell.pos, spell.radius);
    let d_inner = sd_sphere(p - spell.pos, spell.radius - 0.1);
    let d = max(d_outer, -d_inner);  // shell
    // Shield acts like glass: refracts rays passing through
    return SpellHit(d, spell.color * 0.2, 0.0);
}

fn evaluate_spell_gravity(ray_pos: vec3<f32>, ray_dir: ptr<function, vec3<f32>>, spell: SpellData) {
    let to_center = spell.pos - ray_pos;
    let dist = length(to_center);
    if dist < spell.radius {
        // Warp ray direction toward/away from center (gravitational lensing)
        let warp = normalize(to_center) * spell.power / (dist * dist + 0.1);
        *ray_dir = normalize(*ray_dir + warp * 0.1);
    }
}

fn evaluate_spell_frost(p: vec3<f32>, spell: SpellData, roughness: ptr<function, f32>) {
    let d = sd_sphere(p - spell.pos, spell.radius);
    if d < 0.0 {
        // Increase roughness inside frost volume → glass becomes frosted
        *roughness += spell.power * smoothstep(0.0, -1.0, d);
    }
}

fn evaluate_spell_lightning(p: vec3<f32>, spell: SpellData) -> SpellHit {
    // Cylinder SDF with noise displacement for branching effect
    let d = sd_cylinder(p, spell.pos, spell.pos + spell.dir * spell.radius, 0.05);
    let branch = fbm_3d(p * 20.0 + spell.time * 10.0, 3u) * 0.1;
    let density = smoothstep(0.05, 0.0, d + branch);
    let emission = vec3(0.6, 0.8, 1.0) * spell.intensity * density;
    return SpellHit(d, emission, density);
}
```

### Physical World Effects

Spells don't just look pretty — they modify the voxel world:

| Spell | World Effect |
|-------|-------------|
| Fireball | On impact: `remove_sphere()` at hit position, debris particles glow orange |
| Lightning | On impact: line of voxels scorched (color changed to black/grey) |
| Frost | Nearby glass panels increase roughness (frosted appearance via mipmap blur) |
| Gravity | No voxel change, but particles in area get velocity impulse |
| Shield | Blocks incoming spell SDFs (SDF intersection check server-side) |

### Server-Side Spell Collision

```rust
#[spacetimedb::reducer(repeat = 50ms)]
pub fn spell_tick(ctx: &ReducerContext) {
    for spell in ctx.db.active_spell().iter() {
        // Move projectile spells
        if is_projectile(spell.spell_type) {
            let new_pos = advance_spell_position(&spell);
            // Check collision vs players
            for target in ctx.db.player().iter() {
                if target.identity == spell.caster { continue; }
                let dist = sd_capsule_point(Vec3::from(new_pos), ...);
                if dist < spell.radius {
                    apply_spell_damage(ctx, &spell, &target);
                    ctx.db.active_spell().spell_id().delete(spell.spell_id);
                    break;
                }
            }
            // Check collision vs terrain (SVO bounds)
            if terrain_collision(new_pos) {
                apply_terrain_effect(ctx, &spell, new_pos);
                ctx.db.active_spell().spell_id().delete(spell.spell_id);
            }
        }

        // Expire old spells
        if ctx.timestamp - spell.spawn_tick > spell.lifetime_ticks {
            ctx.db.active_spell().spell_id().delete(spell.spell_id);
        }
    }
}
```

### Mana System

```rust
#[spacetimedb::reducer]
pub fn cast_spell(ctx: &ReducerContext, spell_type: u8, dir: (f32,f32,f32), power: f32) -> Result<(), String> {
    let mut player = ctx.db.player().identity().find(ctx.sender)
        .ok_or("Not authenticated")?;

    let mana_cost = spell_mana_cost(spell_type, power);
    if player.mana < mana_cost {
        return Err("Not enough mana".into());
    }
    player.mana -= mana_cost;
    ctx.db.player().identity().update(player);

    ctx.db.active_spell().insert(ActiveSpell {
        spell_id: 0, caster: ctx.sender, spell_type,
        position: player.position, direction: dir,
        power, radius: spell_base_radius(spell_type) * power,
        spawn_tick: ctx.timestamp,
        lifetime_ticks: spell_lifetime(spell_type),
    });
    Ok(())
}
```

## Dependencies
- T22 (Combat): Damage application, CombatEvent table for spell impact VFX
- T6d (Tiled rasterizer): Ray-marching loop where spell SDFs are evaluated
- T3b (Glass VFX): Frost spell modifies glass roughness; shield spell uses glass refraction
- T2b (Render graph): Spell uniform buffer uploaded as part of render graph

## Acceptance Criteria
1. Fireball: visible volumetric sphere with FBM turbulence, emits light into scene
2. Shield: transparent refractive shell (glass-like), blocks incoming spells
3. Frost: glass panels in radius become frosted (increased mipmap blur roughness)
4. Gravity: rays passing through volume are visibly warped (lensing effect)
5. Lightning: branching beam with noise displacement, scorches terrain on hit
6. Mana cost deducted server-side; cast fails if insufficient
7. Spell projectiles move and collide server-side (~50ms tick)
8. All spell SDFs render at 60fps with 10+ simultaneous active spells

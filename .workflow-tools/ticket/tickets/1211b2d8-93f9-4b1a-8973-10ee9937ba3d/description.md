# Combat System: SDF Hit Detection, Damage Model & Voxel Destruction VFX

## Problem

Combat in this RPG uses real-time SDF collision between weapon swings and player/NPC capsules. There are no hitboxes or animation frames — a weapon's SDF sweep volume is tested against target SDFs on the server. Impacts trigger voxel destruction particles and damage numbers rendered as 3D elements in the world.

## Architecture: SDF Sweep → Server Validation → VFX Response

### Combat Data Flow

```
Player presses attack
    ↓
Client computes weapon sweep arc (SDF)
    ↓
SpacetimeDB reducer: perform_attack(direction, weapon_slot)
    ↓
Server: validates range + SDF overlap with nearby player/NPC capsules
    ↓
Server: applies damage, spawns VoxelDestructionEvent
    ↓
Client subscription: receives damage + VFX events
    ↓
Local GPU: particle burst, damage numbers, voxel debris
```

### Weapon SDF Model

Weapons are not meshes — they're SDF primitives derived from their voxel shape:

```rust
pub enum WeaponSdf {
    /// Sword: rotated box SDF with length/width from voxel bounds
    Sword { half_extents: Vec3, reach: f32 },
    /// Axe: combination of box (handle) + rounded-box (head)
    Axe { handle_length: f32, head_size: Vec3 },
    /// Hammer: sphere at end of capsule
    Hammer { handle_length: f32, head_radius: f32 },
    /// Fist: capsule (default, no weapon equipped)
    Fist { radius: f32 },
}

impl WeaponSdf {
    /// Generate from item blueprint's voxel bounds
    pub fn from_blueprint(bp: &ItemBlueprint) -> Self {
        let bounds = compute_voxel_bounds(&bp.voxel_data);
        // Classify by aspect ratio
        if bounds.x > bounds.z * 3.0 { WeaponSdf::Sword { .. } }
        else if bounds.y > bounds.x * 2.0 { WeaponSdf::Hammer { .. } }
        else { WeaponSdf::Axe { .. } }
    }
}
```

### Server-Side Hit Detection

```rust
#[spacetimedb::reducer]
pub fn perform_attack(
    ctx: &ReducerContext,
    direction: (f32, f32, f32),
    weapon_slot: u8,
) -> Result<(), String> {
    let attacker = ctx.db.player().identity().find(ctx.sender)
        .ok_or("Not authenticated")?;

    // Get weapon stats from equipped inventory slot
    let weapon = get_equipped_weapon(ctx, ctx.sender, weapon_slot)?;
    let reach = weapon.reach();

    // Sweep test: find all players/NPCs within reach
    let dir = Vec3::from(direction).normalize();
    let sweep_origin = Vec3::from(attacker.position) + dir * 0.5;

    for target in ctx.db.player().iter() {
        if target.identity == ctx.sender { continue; }

        let target_pos = Vec3::from(target.position);
        let capsule_dist = sd_capsule_point(sweep_origin, target_pos, target_pos + Vec3::Y * 1.8, 0.3);

        if capsule_dist < reach {
            // Calculate damage based on weapon stats + target armor
            let damage = compute_damage(&weapon, &target);
            apply_damage(ctx, target.identity, damage);

            // Spawn destruction event for VFX
            ctx.db.combat_event().insert(CombatEvent {
                event_id: 0,
                attacker: ctx.sender,
                target: target.identity,
                damage,
                hit_pos: (sweep_origin + dir * capsule_dist).into(),
                tick: ctx.timestamp,
            });
        }
    }
    Ok(())
}
```

### Damage Model

```rust
pub fn compute_damage(weapon: &WeaponStats, target: &Player) -> i32 {
    let base = weapon.base_damage;
    let weight_bonus = (weapon.weight * 2.0) as i32;  // heavier = more damage
    let total = base + weight_bonus;
    // Armor reduction (future: equipment system)
    total.max(1)  // minimum 1 damage
}

pub fn apply_damage(ctx: &ReducerContext, target_id: Identity, damage: i32) {
    if let Some(mut target) = ctx.db.player().identity().find(target_id) {
        target.hp = (target.hp - damage).max(0);
        ctx.db.player().identity().update(target);

        if target.hp <= 0 {
            trigger_death(ctx, target_id);
        }
    }
}
```

### Client-Side VFX

#### Voxel Destruction Particles

When a weapon hits voxels (terrain destruction), the affected voxels are removed from the SVO and converted to physics particles:

```rust
fn spawn_impact_particles_system(
    mut events: EventReader<CombatVfxEvent>,
    mut particle_emitter: ResMut<ParticleEmitterSet>,
    mut world: ResMut<VoxelWorld>,
) {
    for event in events.read() {
        // Remove voxels in impact radius from SVO
        let destroyed = world.remove_sphere(event.hit_pos, event.impact_radius);

        // Convert destroyed voxels to particles with initial velocity
        for (pos, color) in destroyed {
            particle_emitter.emit(ParticleSpawn {
                position: pos.as_vec3(),
                velocity: (pos.as_vec3() - event.hit_pos).normalize() * 5.0,
                color,
                lifetime: 2.0,
                size: 0.1,
            });
        }
    }
}
```

#### 3D Damage Numbers

Damage numbers are SDF text rendered in world space, floating upward from the hit position:

```rust
#[derive(Component)]
pub struct DamageNumber {
    pub value: i32,
    pub world_pos: Vec3,
    pub velocity: Vec3,     // float upward
    pub lifetime: f32,
    pub opacity: f32,
}
```

These are rendered as screen-space Dioxus overlays positioned via world-to-screen projection, with the glass refraction of the UI causing the numbers to "wobble" on impact.

#### Battle-Glass Effect

On taking damage, the player's Liquid Glass UI panels briefly increase their distortion (refraction offset) and tint red, creating a visceral "impact shake" effect that's physically motivated by the glass shader.

### RPG Stats Display

HP/Mana bars are Dioxus overlay elements (T9 bridge) positioned at the bottom of the screen. They read from the local Player table subscription and animate smoothly toward current values.

## Dependencies
- T17 (SpacetimeDB): CombatEvent table, perform_attack reducer, damage/death logic
- T20 (Multiplayer characters): Remote player capsule SDFs as attack targets
- T4 (Particle system): Voxel debris particles from impacts
- T7a (VoxelWorld API): `remove_sphere()` for terrain destruction at impact points

## Acceptance Criteria
1. Attack with weapon: server validates SDF overlap, applies damage
2. Weapon reach derived from voxel item shape (longer weapon = more reach)
3. Damage numbers float upward from hit position as 3D overlays
4. Voxel terrain destruction at impact point spawns debris particles
5. Debris particles collide with remaining SVO terrain (T4 particle collision)
6. Battle-glass: UI panels distort + tint red on damage taken
7. HP/Mana bars update in real-time from SpacetimeDB subscription
8. Death handling: player respawn at spawn point with HP restored

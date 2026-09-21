# LLM Integration: Text-to-Voxel/Shader Generation, Naga Validation & Runtime Hot-Reload

## Problem

Players can type natural-language descriptions into the UI to procedurally generate voxel structures, custom shader effects, or skill modifiers. An LLM translates the prompt into either: (a) voxel coordinate + material lists for SVO insertion, or (b) WGSL shader snippets for custom spell/effect SDFs. All generated code is validated through Naga before reaching the GPU, and SpacetimeDB persists both the prompt and the result for multiplayer sharing.

## Architecture: Prompt → LLM → Validate → Apply

### Generation Pipeline

```
Player types prompt in Dioxus text input
    ↓
Client sends to SpacetimeDB procedure: ai_generate(prompt, mode)
    ↓
Procedure calls LLM API (OpenAI/Anthropic) with structured system prompt
    ↓
LLM returns JSON: { voxels: [...], shader: "...", params: {...} }
    ↓
Server validates: bounds check (voxels), naga parse (shaders)
    ↓
Server stores result in GeneratedContent table
    ↓
Reducer applies: voxel deltas inserted OR shader code broadcasted
    ↓
Clients compile shader / insert voxels locally
```

### SpacetimeDB Tables

```rust
#[spacetimedb::table(name = generated_content, public)]
pub struct GeneratedContent {
    #[primary_key]
    #[auto_inc]
    pub content_id: u64,
    pub creator: Identity,
    pub prompt: String,
    pub mode: u8,                  // 0=voxel_structure, 1=shader_effect, 2=spell_modifier
    pub result_json: String,       // LLM output (validated)
    pub created_tick: u64,
}

#[spacetimedb::table(name = custom_shader, public)]
pub struct CustomShader {
    #[primary_key]
    #[auto_inc]
    pub shader_id: u64,
    pub creator: Identity,
    pub wgsl_source: String,       // validated WGSL snippet
    pub function_name: String,     // e.g., "sd_crystal_palace"
    pub active: bool,
}
```

### LLM System Prompt

The system prompt constrains the LLM to produce only valid, safe output:

```
You are a WebGPU world generator for a voxel sandbox game.

MODE: voxel_structure
Respond ONLY with JSON: { "voxels": [[x,y,z,color_hex], ...] }
- Coordinates must be integers in range [-64, 64] relative to player position
- color_hex is a 32-bit RGBA packed as "0xRRGGBBAA"
- Maximum 4096 voxels per generation
- Create the structure the user describes

MODE: shader_effect
Respond ONLY with valid WGSL code defining a single SDF function:
- Function signature: fn sd_custom(p: vec3<f32>, time: f32) -> f32
- Use only: length, dot, cross, normalize, clamp, smoothstep, min, max, abs, sin, cos
- NO texture sampling, NO buffer access, NO loops over external data
- Return signed distance (negative = inside, positive = outside)
```

### Server-Side Validation

#### Voxel Validation

```rust
fn validate_voxels(json: &str, player_pos: Vec3) -> Result<Vec<(IVec3, u32)>, String> {
    let data: VoxelGenResult = serde_json::from_str(json)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    if data.voxels.len() > MAX_GENERATED_VOXELS {
        return Err(format!("Too many voxels: {} > {}", data.voxels.len(), MAX_GENERATED_VOXELS));
    }

    let mut result = Vec::new();
    for [x, y, z, color] in &data.voxels {
        let pos = IVec3::new(*x, *y, *z);
        // Bounds check: must be within generation radius of player
        if pos.as_vec3().distance(player_pos) > MAX_GENERATION_RADIUS {
            return Err(format!("Voxel at {:?} too far from player", pos));
        }
        result.push((pos, *color as u32));
    }
    Ok(result)
}
```

#### Shader Validation (Naga)

```rust
fn validate_wgsl_snippet(wgsl: &str) -> Result<(), String> {
    // Parse with naga (the WGSL parser behind wgpu)
    let module = naga::front::wgsl::parse_str(wgsl)
        .map_err(|e| format!("WGSL parse error: {:?}", e))?;

    // Validate: no external bindings, no buffer access
    let info = naga::valid::Validator::new(Default::default(), Default::default())
        .validate(&module)
        .map_err(|e| format!("WGSL validation error: {:?}", e))?;

    // Check function signature exists
    let has_sd_custom = module.functions.iter()
        .any(|(_, f)| f.name.as_deref() == Some("sd_custom"));
    if !has_sd_custom {
        return Err("Missing required function: sd_custom".into());
    }

    // Safety: reject any function with >1000 instructions (DoS prevention)
    for (_, func) in module.functions.iter() {
        if func.body.len() > MAX_SHADER_INSTRUCTIONS {
            return Err("Shader too complex".into());
        }
    }

    Ok(())
}
```

### Client-Side Shader Hot-Reload

When a `CustomShader` row arrives via subscription, the client compiles it into the render pipeline:

```rust
fn apply_custom_shader_system(
    mut shader_events: EventReader<CustomShaderEvent>,
    device: Res<WgpuDevice>,
    mut custom_sdf_pipeline: ResMut<CustomSdfPipeline>,
) {
    for event in shader_events.read() {
        // Inject the custom function into the base ray-marching shader template
        let full_wgsl = format!(
            "{}\n{}\n{}",
            SHADER_HEADER,          // common types + utility functions
            event.wgsl_source,       // the LLM-generated sd_custom function
            SHADER_FOOTER,          // main ray-march loop that calls sd_custom
        );

        // Compile at runtime
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("custom_sdf"),
            source: wgpu::ShaderSource::Wgsl(full_wgsl.into()),
        });

        custom_sdf_pipeline.update(module);
    }
}
```

### Multiplayer Sharing

Since `GeneratedContent` and `CustomShader` tables are public and subscribed, all players in range see:
- Voxel structures appear as they're generated (via normal voxel delta subscription)
- Custom shader effects activate in their local render pipeline

### History & Undo

The `GeneratedContent` table preserves all prompts. A "regenerate" UI button re-runs the prompt with a different temperature. An "undo" button deletes the voxel deltas associated with a specific `content_id`.

### Rate Limiting

Server-side: max 1 generation per player per 30 seconds. LLM API costs are managed by the server operator (API key in SpacetimeDB environment variables, never exposed to clients).

## Dependencies
- T17 (SpacetimeDB): GeneratedContent + CustomShader tables, ai_generate procedure
- T7a (VoxelWorld API): Voxel insertion for generated structures
- T2b (Render graph): Custom SDF pipeline slot for hot-reloaded shaders

## Acceptance Criteria
1. "Create a castle" → LLM returns voxel list → castle appears in world within 5s
2. "Create an SDF that looks like a spinning crystal" → WGSL function generated, validated, compiled, visible
3. Naga validation catches invalid WGSL before it reaches the GPU (no crashes)
4. Voxel bounds enforced: no generation beyond MAX_GENERATION_RADIUS from player
5. Shader instruction limit prevents DoS via overly complex shaders
6. Generated content visible to all players in range (multiplayer sync)
7. History: prompt + result stored, re-generation possible
8. Rate limit: server rejects rapid-fire generation requests

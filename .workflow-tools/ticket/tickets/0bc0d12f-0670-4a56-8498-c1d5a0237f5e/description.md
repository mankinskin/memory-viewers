# Code Viewer: Source Code as Glass Panels Refracting Voxel Splats

## Problem

Source code files are displayed as glass panels in the 3D Voxel-splatted world. Code panels use moderate roughness — enough frosting to keep syntax-highlighted text readable, but transparent enough to show the voxel-splatted scene behind for spatial context.

## Architecture

### Code Panel Entity

```rust
#[derive(Component)]
pub struct CodePanel {
    pub file_path: String,
    pub language: Language,
    pub visible_range: (usize, usize),  // line range
    pub scroll_offset: f32,
    pub highlight_lines: Vec<usize>,     // e.g., search results, breakpoints
}

#[derive(Bundle)]
pub struct CodePanelBundle {
    pub code: CodePanel,
    pub world_panel: WorldPanel,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
}
```

### Visual Properties

```rust
fn create_code_panel(file: &SourceFile, pos: Vec3) -> CodePanelBundle {
    CodePanelBundle {
        code: CodePanel {
            file_path: file.path.clone(),
            language: file.language,
            visible_range: (0, 50),
            scroll_offset: 0.0,
            highlight_lines: vec![],
        },
        world_panel: WorldPanel {
            half_extents: Vec2::new(2.5, 3.0),
            corner_radius: 0.05,
            content_texture: render_code_content(file),
            roughness: 0.4,    // moderate frost — voxel-splatted scene softly visible behind code
            tint: Vec3::new(0.1, 0.1, 0.12),  // dark tint for code readability
            anchor: PanelAnchor::WorldFixed(pos, Quat::IDENTITY),
        },
        transform: Transform::from_translation(pos),
        global_transform: GlobalTransform::default(),
    }
}
```

### Glass Effects on Code Panels

With roughness 0.4, the voxel-splatted scene behind the code panel is visible but blurred (mipmap level ~3.6). This creates a sense of depth — you can see colorful voxel splat blobs (graph nodes, voxel terrain) through the code, providing spatial context:
- Where you are in the 3D world
- Which graph nodes are nearby
- What code relates to nearby context

The dark tint ensures white/colored text remains readable over the blurred voxel-splatted background.

### Syntax Highlighting

Code is rendered to the content texture with syntax highlighting via a tree-sitter or regex-based highlighter. Colors come from the theme palette (T5) so code colors are consistent with the world's material palette.

### Highlighted lines (search results, errors) get a semi-transparent overlay:

```rust
fn render_code_content(file: &SourceFile, panel: &CodePanel) -> TextureHandle {
    let mut surface = RenderSurface::new(panel_pixel_size);
    for (i, line) in file.lines[panel.visible_range.0..panel.visible_range.1].iter().enumerate() {
        if panel.highlight_lines.contains(&(panel.visible_range.0 + i)) {
            surface.fill_rect(line_rect(i), highlight_color);
        }
        surface.draw_highlighted_text(line, &syntax_theme);
    }
    surface.to_texture()
}
```

### Navigation

- Click line in code panel → select line, show in context
- Ctrl+Click function/type → navigate to definition (open new code panel at target location)
- Scroll within panel → virtual scrolling
- Code panels can be linked to graph nodes → clicking a node reveals related code

## Dependencies
- T10 (3D UI): WorldPanel, glass SDF
- T3 (liquid glass): Mipmap frosted blur behind code
- T9 (bridge): Dioxus→Taffy for text layout in content texture
- T5 (theme): Syntax highlighting colors from palette

## Acceptance Criteria
1. Code panels render as dark-tinted glass in 3D world
2. Syntax highlighting visible and readable over frosted voxel-splatted background
3. Scroll works within panels
4. Line highlighting for search results / errors
5. Click-to-navigate between code panels
6. voxel-splatted scene softly visible through panel (spatial context)

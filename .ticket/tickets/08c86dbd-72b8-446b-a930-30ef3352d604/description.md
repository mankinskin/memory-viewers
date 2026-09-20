Create comprehensive Playwright E2E test coverage for the four graph improvements across all memory-viewers:

**Test Coverage Areas:**
1. **Selection focus/deselection**: 
   - Outside-click clears selection in ticket-viewer, spec-viewer, log-viewer
   - Focus neighborhood transparency works correctly
   - Selected node and neighbors remain emphasized

2. **Property-based rendering tiers**:
   - 5-level LOD system responds to distance and hover
   - Hover promotion advances nodes one tier
   - Camera-mode specific minimal glyphs (Fixed2D vs Orbit3D)

3. **Panel-aware framing**:
   - Graph nodes stay behind UI panels in all viewers
   - Viewport insets work correctly for each viewer's panel layout
   - Edge overlay styling for panel overlap

4. **2D mode/keyframing**:
   - Fixed2D camera mode switching works
   - Presentation keyframing functions
   - Camera projection transitions are smooth

**Test Implementation:**
- Create shared test utilities in `viewer-api/frontend/dioxus/e2e/shared/`
- Add viewer-specific test suites:
  - `ticket-viewer/e2e-release/graph-improvements.spec.ts`
  - `spec-viewer/e2e-release/graph-improvements.spec.ts`  
  - `log-viewer/e2e/graph-improvements.spec.ts`
- Include screenshot validation for visual regression testing
- Test cross-viewer consistency of interaction patterns

**Acceptance Criteria:**
1. All four graph improvements have E2E test coverage in all three viewers
2. Tests run successfully in CI pipeline
3. Screenshot validation captures visual behavior
4. Cross-viewer consistency is verified
5. Test failures provide clear diagnostics for debugging
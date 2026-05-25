## Open/close contract

- Clicking the header `Theme settings` action MUST open a visible modal overlay, not just mount the panel in the DOM.
- The overlay MUST use `.modal-backdrop[role="dialog"][aria-label="Theme settings"]` with fixed positioning, a non-transparent backdrop tint, and a centered `.modal-panel.theme-settings-modal` container.
- The underlying page MUST remain visibly dimmed behind the modal while the theme settings panel is open.

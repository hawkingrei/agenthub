# Team UI Shell And Bundle Refinement

## Summary

- simplified the Team selector into a slimmer chooser that more closely follows the Slock information hierarchy while keeping a more neutral Notion-like visual tone
- reduced Team and Agent ACP shell chrome so the main content area gets more width and less repeated metadata
- aligned the Team channel and ACP viewport/rich-text foundations through shared `thread_viewport` and `thread_rich_text` building blocks
- moved Team channel read state into a bottom-right radial indicator with hover details for read/unread recipients
- improved initial web bundle behavior by splitting route-level chunks and removing `vendor-markdown` from the initial preload chain

## Details

### Team and ACP UI

- narrowed the Teams sidebar/workspace ratio to match the Slock-style "narrow rail, wide content" layout
- kept the color system closer to Notion-style neutrals instead of adopting the brighter Slock palette
- made ACP input docking behave like the main `/agents` workbench so the composer stays pinned at the bottom of the panel
- reduced default ACP content density by collapsing larger payload sections and trimming preview sizes

### Channel read-state treatment

- replaced text `N seen` actions with a hoverable radial progress marker rendered in the bottom-right corner of each message bubble
- kept pending delivery in the same bottom-right status position so message delivery/read feedback uses one consistent affordance

### Bundle loading

- lazy-loaded `Join`, `Admin`, and `Teams` pages through route-level chunk loading
- extracted `escapeHtml` into a lightweight helper so route-independent code no longer imports the heavy markdown renderer path
- moved highlight.js theme CSS out of the entry bundle
- changed Vite preload filtering so `route-auth`, `route-teams`, and `vendor-markdown` are no longer preloaded from `index.html`

## Validation

- `make build-web`
- inspected `web/dist/index.html` to confirm the initial preload set no longer includes `route-auth`, `route-teams`, or `vendor-markdown`

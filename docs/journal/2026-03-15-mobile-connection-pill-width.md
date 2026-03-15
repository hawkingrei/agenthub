# Mobile Connection Pill Width

## Summary

- Prevent the workbench connection status pill from inheriting the legacy `.session > span` truncation rule on narrow screens.
- Keep the badge on a single line and opt it out of flex shrinking so `ONLINE · SSE IDLE` remains visible on mobile.

## Implementation

- Updated `web/src/app.tsx` to render the connection badge with a `div` wrapper instead of a `span`.
- Added `shrink-0` and `whitespace-nowrap` to the shared workbench status pill class.

## Validation

- `cd web && npm run lint -- src/app.tsx`
- `cd web && npm run build`
- Chrome DevTools MCP baseline at `390x844` on `https://agenthub.hawkingrei.com/` shows the full `ONLINE · SSE IDLE` label in the header banner.

## Notes

- The Chrome MCP check validates the current deployed shell state. The local branch change hardens the same header path against the legacy truncation selector before deployment.

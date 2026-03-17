# Workbench Account Menu Compression

## Summary

- Compressed the workbench header controls into a single top-level menu trigger.
- Removed the separate inline `Agents / Teams` switch, username, settings button, and logout button from the header row.

## Implementation

- Added a shared compact Mantine `Menu` trigger component for the workbench header.
- Moved `Agents`, `Teams`, `Settings`, and `Logout` into the same dropdown.
- Applied the shared menu to both `web/src/app.tsx` and `web/src/pages/team_page.tsx`.
- Kept the username inside the menu label instead of consuming persistent header width.
- Removed the retired `WorkbenchModeSwitch` component and its test after both pages stopped using it.

## Validation

- `cd web && npm run lint -- src/app.tsx`
- `cd web && npm run build`
- Chrome DevTools MCP baseline on `https://agenthub.hawkingrei.com/` at `390x844` confirms the current mobile shell density before deployment; local header change is limited to the authenticated account controls in the same banner row.

## Notes

- This change is aimed at mobile compression first. Persistent top-row controls are reduced to status plus a single menu trigger.

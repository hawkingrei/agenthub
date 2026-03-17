# Workbench Connection Badge Unification

## Summary

- replaced the static Teams header badge with the shared connection badge used by the Agents workbench
- extracted a shared workbench connection badge component so both pages render the same shell affordance
- moved navigator online detection into the shared connection-status helper

## Details

- added `WorkbenchConnectionBadge` as the shared header badge renderer for workbench pages
- updated the agents page header to render through the shared badge component without changing its data source
- updated the teams page header to derive its label from the shared connection badge helper instead of showing the static `Team console` text
- kept teams on the shared `Online · SSE idle` / offline fallback semantics for now; transport-specific refinement can be added later without changing header structure

## Validation

- local regression should verify that both `/` and `/teams` show the same header badge wording for online and offline states
- Chrome DevTools MCP against `https://agenthub.hawkingrei.com` currently shows the old static teams header badge until this change is deployed

# Team Page Modal Extraction

- extracted the large inline Team create/edit dialogs and the forge-agent wrapper out of `web/src/pages/team_page.tsx` into `web/src/pages/team/team_management_modals.tsx`;
- kept the existing Team styling contract by passing the same workbench chrome class names down into the extracted dialog components instead of restyling the modals;
- added focused modal rendering coverage in `web/src/pages/team/team_management_modals.test.tsx`.

## Why

`web/src/pages/team_page.tsx` had grown past `5000` lines and still owned several self-contained dialog surfaces. The create-Team dialog, edit-member profile dialog, and forge-agent wrapper were all stable UI surfaces with explicit props and no need to stay inline with the main Team route orchestration. Extracting them reduces the blast radius of future Team workbench changes without changing runtime behavior.

## Validation

- `cd web && npm run test -- src/pages/team/team_management_modals.test.tsx src/pages/team_page.smoke.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- --ignore-pattern dist-debug --ignore-pattern dist-debug-current`
- `cd web && npm run build`

## Follow-up

- `web/src/pages/team_page.tsx` still owns the main Team route orchestration and several workspace toolbars; the next lowest-risk extraction target is now the selected-workspace header/menu action group rather than the dialogs.

- Extracted Team workspace header/action chrome into `web/src/pages/team/team_workspace_header.tsx`, added focused render coverage, and reduced `team_page.tsx` further while preserving Team workbench behavior.

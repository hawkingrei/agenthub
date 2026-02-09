# ACP Permission Option ID Mapping

## Background
ACP permission options are serialized from the server using camelCase (`optionId`), but the frontend expects `option_id`. This mismatch results in the UI sending `null` option IDs, which the backend interprets as `cancelled`, so approvals are always rejected.

## Scope
- Accept both `option_id` and `optionId` in the frontend permission UI.
- Disable option buttons when no option ID is present to avoid accidental cancellation.

## Key Decisions
- Normalize the option ID in the UI layer to keep API responses backward compatible.
- Keep the server payload unchanged to avoid breaking existing ACP clients.

## Validation
- Trigger `/permission-demo` and click **Allow once** or **Always**.
- Verify the permission response outcome is `selected` and the tool call completes successfully.

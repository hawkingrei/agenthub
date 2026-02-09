# ACP Permission Option ID Mapping

## Background
ACP permission options are serialized from the server using camelCase (`optionId`), but the frontend expects `option_id`. This mismatch results in the UI sending `null` option IDs, which the backend interprets as `cancelled`, so approvals are always rejected.

## Scope
- Normalize permission options to `option_id` on the server for new requests.
- Normalize legacy stored payloads so the API always returns `option_id`.
- Keep the frontend UI `option_id`-only and disable buttons when the ID is missing.

## Key Decisions
- Emit `option_id` in server permission payloads to align with the API and UI.
- Allow legacy `optionId` data to deserialize server-side, then re-serialize as `option_id`.
- Add a UI test that rejects empty option IDs.

## Validation
- Trigger `/permission-demo` and click **Allow once** or **Always**.
- Verify the permission response outcome is `selected` and the tool call completes successfully.
- Run `pnpm test` (or `npm test`) and confirm `permission_modal.test.tsx` passes.

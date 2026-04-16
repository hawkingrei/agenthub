# Approval Persistence Guidance

- Updated the runtime-injected actor skill guidance to prefer reusable approvals over one-time
  approvals when ACP offers the same least-privilege scope for frequently repeated trusted command
  families such as `agenthub actor ...`.
- Mirrored the same guidance in the Team leader and worker role skills so permission-review
  decisions stay consistent with the runtime-injected prompt text.
- The intent is prompt-level only: do not broaden approval scope, but avoid repeated one-time
  approvals when the same safe prefix will predictably be used many times in the same workflow.

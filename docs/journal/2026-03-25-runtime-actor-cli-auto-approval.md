# Runtime Actor CLI Auto Approval

- Added a narrow ACP-side exec auto-approval path for the runtime-injected
  actor CLI prefix: commands are allowed when they resolve to the same
  binary as `AGENTHUB_ACTOR_CLI` and the second argv segment is `actor`.
- The matcher canonicalizes the injected actor CLI path and only auto-approves
  commands whose executable path resolves to the same binary and whose second
  argv segment is exactly `actor`.
- Shell-wrapped invocations such as `/bin/zsh -lc '<actor_cli> actor inbox ...'`
  are also recognized, because Codex exec requests often flow through `-lc`.
- Bare `agenthub actor ...` also auto-approves, but only when `PATH`
  resolves `agenthub` to that same runtime binary.
- This avoids repeated Team runtime approval prompts without broadening trust
  to unrelated `agenthub` subcommands or arbitrary binaries.

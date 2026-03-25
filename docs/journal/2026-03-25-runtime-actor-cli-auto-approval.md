# Runtime Actor CLI Auto Approval

- Added a narrow ACP-side exec auto-approval path for the runtime-injected
  canonical actor CLI prefix: `<AGENTHUB_ACTOR_CLI> actor ...`.
- The matcher canonicalizes the injected actor CLI path and only auto-approves
  commands whose executable path resolves to the same binary and whose second
  argv segment is exactly `actor`.
- Shell-wrapped invocations such as `/bin/zsh -lc '<actor_cli> actor inbox ...'`
  are also recognized, because Codex exec requests often flow through `-lc`.
- This avoids repeated Team runtime approval prompts without broadening trust to
  unrelated `agenthub` subcommands or arbitrary binaries.

# Team Memory Index Rolling Upgrade

## Summary

- extended the Team workspace memory contract with an explicit rolling-upgrade compatibility
  section for `.cache/context/` and other machine-read memory/index files
- defined versioning, additive evolution, and reader/writer compatibility expectations so prompt
  tail slimming can keep moving state behind filesystem pointers without forcing one-shot workspace
  migrations

## Why

The current Team prompt-tail work already treats `.cache/context/state.md` and run-scoped notes as
pointer-first runtime indexes, but the spec did not yet say how those files should evolve across
mixed-version runtime rollouts.

Without that rule, every new memory/index field risks becoming a de facto breaking change for older
readers or restart paths that still expect the previous shape.

## Changes

- added `Rolling Upgrade Compatibility Contract` to
  `docs/features/team-workspace-memory-contract.md`
- defined these compatibility requirements:
  - machine-read index files are versioned file protocols
  - additive changes are preferred over in-place breaking rewrites
  - readers must tolerate unknown extra fields and older optional shapes
  - writers may move forward, but should preserve compatibility-facing fields during rollout
  - startup should not require a full workspace migration just to read Team context
- clarified that `state.md` is the compatibility-facing current-state index, while higher-fidelity
  details continue to live under `.cache/context/run/<run_id>/...`
- documented the recommended rollout order:
  1. land backward-compatible readers
  2. switch writers to the new schema or pointed-to file
  3. keep dual-read or compatibility fields during rollout
  4. remove legacy fields only after the old reader generation is retired

## Follow-Up

- future Team runtime changes that add machine-read state files should declare `schema_family` and
  `schema_version` in the file contract instead of relying on implicit field guessing
- the next prompt-tail slimming changes should follow this contract when introducing additional
  `state.md` pointers or run-scoped note formats

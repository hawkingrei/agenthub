# Team Workspace Memory Contract

## Summary

- promoted the scattered Team context/memory rules into a stable feature doc:
  - `docs/features/team-workspace-memory-contract.md`
- separated the v1 ownership boundary between:
  - `.cache/context/` as runtime-owned continuity, indexes, append-only trails, and run artifacts
  - `.agenthubmemory/` as worker-authored durable project memory
- documented the stable index files, append-only files, run artifact layout, and promotion rules so
  prompt-tail slimming has one canonical filesystem target

## Why

The recent prompt/runtime compaction work already moved Team prompts and ACP runtime continuity
toward pointer-first state, but the stable memory contract was still spread across old journals and
feature docs.

That made it too easy for prompts, skills, and runtime guidance to each describe `.cache/context`
and `.agenthubmemory/` differently.

## Changes

- added `docs/features/team-workspace-memory-contract.md` as the canonical v1 memory contract
- defined the file-level roles for:
  - `.cache/context/AGENTS.md`
  - `.cache/context/TODO.md`
  - `.cache/context/state.md`
  - append-only `decisions.md`, `errors.md`, `log.md`
  - `.cache/context/memory/*.md`
  - `.cache/context/run/<run_id>/...`
- defined the worker-facing `.agenthubmemory/` layout:
  - `TODO.md`
  - `journal/`
  - `note/`
  - `scratch/`
- clarified promotion and pointer rules between prompt text, `L1` run artifacts, and `L2` durable
  memory

## Follow-Up

- runtime prompt-tail slimming can now move additional dynamic state into the documented index files
  instead of inventing new ad-hoc prompt prose
- future implementation work should link concrete runtime writes and flush behavior back to this doc
  rather than duplicating memory rules in more journals

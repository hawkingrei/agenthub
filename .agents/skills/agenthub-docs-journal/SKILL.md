---
name: agenthub-docs-journal
description: "Journal writing for rollout notes and validation evidence."
---

# AgentHub Journal Writing

Use this skill when writing or reviewing `docs/journal/YYYY-MM-DD-topic.md` files, especially for rollout notes, implementation checkpoints, or validation evidence.

## Goal

Keep journals as concise, dated implementation records:

- what changed
- why it changed
- what was validated
- what still remains open

Journals are not canonical specs and are not historical dumping grounds for every scratch note.

## Surface Rules

- Journal files live under `docs/journal/`
- Filenames must use `YYYY-MM-DD-topic.md`
- One topic per file; append to an existing journal when the follow-up is part of the same rollout
- Stable contracts belong in `docs/features/`, not only in a journal
- Open follow-up work belongs in `docs/todo.md`, not buried in prose

## Required Structure

Keep the structure operational and predictable. A normal journal should contain:

- `Summary`
- `Background`
- `Scope`
- `Key Decisions`
- `Validation`
- `Follow-Ups`

You may add a short `Files` section for implementation-heavy notes, but do not turn the journal into a file-by-file changelog unless that is the point of the record.

## Writing Rules

### 1. Record the post-change truth

Do not leave pre-fix wording in place once the same PR already changed the code or doc.

Examples:

- do not say a file "still remains" if the same change already updated it
- do not leave migration steps marked as pending when this journal already records them as done
- do not describe the audit as complete if only one slice landed

### 2. Keep effort and follow-up estimates aligned

If `docs/todo.md` references a journal estimate, the journal and TODO must agree.

Do not let:

- `docs/todo.md` say `~12h`
- while the linked journal says `~8h`

### 3. Validation must be concrete

List the focused checks that actually support the journal entry.

Good examples:

```bash
cargo test -p agenthub-acp prompt_prefix_blocks_append_runtime_context_after_skills -- --nocapture
cd web && npm exec vitest -- run src/pages/team_page.smoke.test.tsx
gh pr checks 468 | cat
```

Bad examples:

- "tested locally"
- "CI should pass"
- "verified"

### 4. Follow-ups should describe remaining work only

If a step landed in the same change, remove it from the remaining-work list or rewrite it as the narrower unresolved tail.

### 5. Keep chronology factual

- Use the real implementation date in the filename
- Do not use future-dated journals
- Do not split one tiny documentation cleanup into many micro-journals without distinct decisions

## Compaction Guidance

When journals start repeating the same surface:

1. Extract stable conclusions into `docs/features/<topic>.md`
2. Keep journals for rollout evidence, CI notes, and decision checkpoints
3. Remove stale or contradictory detail from older journal prose when the newer journal supersedes it

## Decision Boundary

Write a journal when:

- implementation behavior changed
- a rollout checkpoint needs validation evidence
- a PR leaves meaningful engineering follow-up context

Do not create a journal for:

- one-line typo-only fixes with no workflow significance
- stable product contracts that belong directly in `docs/features/`

## Before Finishing

Check these:

- filename uses `YYYY-MM-DD-topic.md`
- wording matches the actual post-change state
- validation commands are explicit
- remaining work is really still remaining
- any stable contract change also updated the relevant `docs/features/` file

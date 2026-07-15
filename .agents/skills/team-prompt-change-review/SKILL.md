---
name: team-prompt-change-review
description: Use before editing Team system prompts, runtime prompt tails, prompt-linked skills, or prompt tests to keep prompts bounded, tool-neutral, and pointer-first.
---

# Team Prompt Change Review

Use this skill as the review gate for Team prompt, skill, and checklist changes. The goal is to keep
runtime prompts small while making repeated procedures discoverable through skills and specs.

## Classification

Classify the change before editing:

- Role boundary: who owns a decision, action, or output.
- Runtime tail: current objective, next action, allowed-action gate, blocker, or recovery pointer.
- Skill/checklist pointer: repeated workflow entrypoint.
- Output contract: structured payload, evidence receipt, blocker report, or visible reply shape.
- Documentation-only: stable spec, journal evidence, or TODO follow-up.

If the change is only a repeated procedure, prefer a skill or checklist instead of prompt prose.

## Prompt Budget Gate

A prompt change is acceptable only when it adds or corrects one of:

- authority boundary;
- allowed or denied action;
- recovery pointer;
- required skill/checklist entrypoint;
- user-visible communication contract;
- output payload contract.

Do not add:

- long logs or history;
- full troubleshooting recipes;
- raw tool output;
- private memory tool names;
- provider-specific wording unless the provider boundary is the feature being changed.

## Tool-Neutrality Gate

Open-source prompt and docs must stay tool-neutral:

- say external searchable memory or durable knowledge system when the backend is not a repository
  contract;
- avoid requiring private tools by name;
- keep personal workflow details out of repository instructions;
- use repository-owned terms: SOP, skill, checklist, feature spec, journal, TODO, artifact pointer.

## Skill Extraction Gate

Before adding prompt prose for a repeated workflow, answer:

1. Can this become a `.agents/skills/<name>/SKILL.md` entrypoint?
2. Can the prompt name the entrypoint instead of carrying the steps?
3. Does the workflow have trigger conditions, decision boundary, validation, and fallback behavior?
4. Does an existing feature spec already define the stable contract?

If the answers are yes, add or update the skill/checklist first and keep prompt text to one trigger
line.

## Test Gate

For every prompt-facing change, update the narrowest guard:

- Prompt template behavior: `cargo test -p agenthub-team-prompts -- --nocapture`.
- New required prompt phrase or pointer: focused assertion in
  `crates/agenthub-team-prompts/src/lib.rs`.
- New skill: validate frontmatter and run `git diff --check`.
- New stable contract: update `docs/features/team-system-prompt-contract.md` or the owning spec.
- Dated rollout evidence: update `docs/journal/`.

## Evidence Receipt

When reporting the change, include:

- classification;
- files changed;
- whether prompt text grew, shrank, or stayed unchanged;
- validation commands;
- what the validation proves and what remains unverified.

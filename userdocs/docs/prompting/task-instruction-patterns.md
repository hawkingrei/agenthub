---
sidebar_position: 9
---

# Task Instruction Patterns

Good prompts reduce retries and improve review quality.

## Recommended Template

Use a compact, structured instruction:

```text
Goal:
Constraints:
Scope:
Validation:
Output format:
```

## Example

```text
Goal: Add user docs for notifications and troubleshooting.
Constraints: Keep existing API behavior unchanged; English docs only.
Scope: userdocs/docs/operations/*
Validation: Provide steps to build docs and verify sidebar links.
Output format: Patch summary + modified file list.
```

## Patterns That Work Well

- One clear goal per turn
- Explicit file/module scope
- Explicit verification expectation
- Explicit non-goals to avoid overreach

## Patterns to Avoid

- Multiple unrelated goals in one instruction
- Ambiguous constraints like "improve everything"
- Missing validation requirements

## Follow-up Prompt Pattern

When refining output:

```text
Keep current structure. Only adjust X.
Do not change Y.
Add tests or checks for Z.
```

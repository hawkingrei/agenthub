---
sidebar_position: 8
---

# Review and Apply Changes

After an agent completes a coding task, review output before you accept changes.

## Recommended Review Flow

1. Read final summary in **Conversation**
2. Confirm changed files and rationale are explicit
3. Ask for test commands and expected outcomes
4. Run checks locally before merge or release

## Suggested Follow-up Prompt

```text
List exact changed files, high-risk points, and tests to run.
Keep the response under 12 bullet points.
```

## Risk Checklist

- Any public API or schema change?
- Any persistence format change?
- Any irreversible operation introduced?
- Any missing tests for non-trivial logic?

If any answer is yes, request a focused second-pass review.

## Minimal Validation Command Pattern

Use project-specific commands, for example:

```bash
# Rust
cargo test

# Web docs
cd userdocs
npm run build
```

## When to Reject and Rework

Reject current output and ask for revision when:

- Scope drift happened (unrequested files changed)
- Critical constraints were ignored
- Validation plan is missing or weak

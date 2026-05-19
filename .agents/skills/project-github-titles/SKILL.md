---
name: project-github-titles
description: "Trigger when writing commit titles, PR titles, or summary first lines for this repository; enforce `type(scope): subject`."
---

# Project GitHub Titles

Trigger when writing commit titles, pull request titles, or the first summary line in a PR/commentary block for this repository.

## Title Format

- Always use: `type(scope): subject`
- Never prefix titles with `[codex]`
- Keep `type`, `scope`, and `subject` in English
- Keep the subject concise and specific
- Do not end the subject with a period

## Allowed Types

- `feat`
- `fix`
- `test`
- `chore`
- `docs`

If the change is mostly structural or maintenance work and none of the behavior-oriented types fit cleanly, use `chore`.

## Scope Rules

- Use the primary component touched by the change
- Keep scope short and lower-case
- Prefer stable product or domain scopes over file names

Common scopes in this repository:

- `actor`
- `team`
- `auth`
- `api`
- `acp`
- `web`
- `ci`
- `docs`
- `skills`

## Subject Rules

- Describe the main effect, not the mechanics
- Prefer imperative or result-oriented phrasing
- Avoid vague subjects such as `update stuff` or `misc cleanup`

Good examples:

- `fix(actor): split receive from inbox`
- `feat(team): add shared-thread task note flow`
- `test(api): cover read-only inbox contract`
- `docs(skills): add project PR title convention`
- `chore(acp): normalize thread event imports`

## Summary Comment Rule

- If a PR template, summary comment, or commit body needs a single-line summary, reuse the same `type(scope): subject` form on the first non-empty line.
- The rest of the body may use normal markdown prose.

## Selection Guidance

- If the branch contains one dominant functional change plus small doc or test updates, title the PR after the dominant change.
- If the branch is mostly tests, use `test(...)`.
- If the branch is mostly documentation or repo instructions, use `docs(...)`.
- Do not use `refactor(...)` for PR titles in this repository; map that intent to `fix(...)` or `chore(...)` based on user-visible impact.

# Documentation Guide

This folder contains engineering-facing documentation for AgentHub contributors.

## Structure

- `docs/features/`: append-only implementation notes with date-prefixed filenames
- `docs/todo.md`: follow-up verification and backlog checklist
- `docs/api_naming.md`: payload naming conventions for AgentHub-owned APIs
- `userdocs/`: end-user documentation site (Docusaurus, generated static pages)

## When You Change Code

Use this checklist for every non-trivial change:

1. Add or update a feature note in `docs/features/`.
2. Add a follow-up item in `docs/todo.md` when verification or continuation is needed.
3. If API payloads changed, ensure naming still conforms to `docs/api_naming.md`.
4. If behavior is user-visible, update `userdocs/docs/` accordingly.

## Feature Note Convention

- Filename: `YYYY-MM-DD-topic.md`
- Keep notes concise and operational:
  - Summary
  - Background
  - Scope
  - Key decisions
  - Validation
  - Follow-ups

## Working With User Docs

`userdocs/` is the static documentation site generator target.

```bash
cd userdocs
npm install
npm run start
npm run build
```

Build artifacts are generated at `userdocs/build/`.

# Team Memory Index Parser Leading Blank Lines

- Date: 2026-04-18
- PR: #383

## Summary

The Team memory/index markdown parser now tolerates leading blank lines before the machine-read title line. This keeps the parser resilient to harmless formatting drift while preserving the existing title and metadata normalization contract.

## Changes

- Update `parse_machine_read_markdown_metadata(...)` to skip leading empty lines before matching the expected markdown title.
- Add a regression test that parses `# Team Runtime State` content with leading blank lines.

## Validation

- `cargo test -p agenthub-team-domain -- --nocapture` remains the intended focused validation target for the parser helpers in this crate.

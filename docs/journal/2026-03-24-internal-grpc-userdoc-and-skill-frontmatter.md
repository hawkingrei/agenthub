# Internal gRPC Userdoc And Team Skill Frontmatter

## Summary

- added missing `description` front matter to the six static Team runtime
  `SKILL.md` sources so managed-skill installs remain valid
- expanded the managed-skills regression test to require `name +
  description` front matter for every managed skill document
- updated user-facing docs to explain when `internal_grpc` is required and why
  `agenthub actor ...` needs an explicit `shared_secret` in `config.toml`

## Why

- Team runtime skill loading was skipping six static skill docs because their
  source front matter only declared `name`
- the recent authority-side actor CLI change made `agenthub actor ...` depend
  on the internal gRPC control plane
- the server can persist a generated internal auth secret, but the actor CLI
  loopback client only reads `config.toml`, so docs needed to call out that
  `shared_secret` must be present for local authority-side actor CLI usage

## Changed Files

- `skills/team/*.SKILL.md`
- `crates/agenthub-managed-skills/src/lib.rs`
- `userdocs/docs/getting-started/configuration-basics.md`
- `userdocs/docs/core/agent-nodes.md`
- `userdocs/docs/deployment/overview-and-topology.md`

## Validation

- `cargo test -p agenthub-managed-skills managed_skill_docs_include_expected_frontmatter -- --nocapture`

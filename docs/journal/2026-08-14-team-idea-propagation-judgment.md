# Team Idea Propagation Judgment

## Summary

Team coordinator and worker prompts now require agents to judge a request to propagate an idea or
instruction separately from the validity of the underlying content. Propagation is not automatic:
the agent must assess user intent, sender authority, factual support, relevance, audience, and risk
while preserving attribution and uncertainty.

## Background

The existing Team prompt contract defined communication surfaces and message routing but did not
explicitly distinguish an idea's merit from a request to widen its reach. That omission could let a
sender's propagation request stand in for evidence, authorization, or audience fit.

## Scope

- Add one compact judgment boundary to the coordinator and worker prompt templates.
- Put the repeatable evaluation procedure in `team-message-intake`.
- Add focused prompt contract assertions for both roles.
- Record the stable boundary in `team-system-prompt-contract.md`.
- Keep platform-level and provider-specific prompts out of scope.

## Key Decisions

- Separate the underlying claim from the requested propagation action.
- Assess user intent, sender authority, factual support, relevance, audience, privacy/security
  boundaries, and propagation risk before relaying content.
- Preserve source attribution, uncertainty, material counterevidence, and unresolved disagreement.
- Apply a higher bar to durable encoding in prompts, skills, documentation, tasks, or Team norms
  than to a narrow one-time relay.
- Keep the runtime prompt rule compact and delegate the repeated procedure to the existing message
  intake skill.

## Validation

- `cargo test -p agenthub-team-prompts -- --nocapture`
- `cargo fmt --all --check`
- `git diff --check`

## Follow-Ups

- Structured provenance or audience-authorization enforcement remains outside this prompt-policy
  change and should be designed separately if a high-impact propagation flow requires it.

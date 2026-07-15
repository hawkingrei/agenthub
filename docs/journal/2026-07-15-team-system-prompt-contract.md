# Team System Prompt Contract

## Summary

Added a canonical Team system prompt contract that defines prompt layers, pointer-first runtime
tails, skill/checklist entry points, and tool-neutral durable knowledge boundaries.

## Background

The repository already had prompt-tail slimming, workspace memory, and Team workflow organization
docs. The missing piece was a focused contract for what belongs in Team coordinator/worker system
prompts versus what belongs in skills, checklists, runtime context files, workspace memory, feature
specs, journals, or artifact pointers.

## Scope

- Added `docs/features/team-system-prompt-contract.md`.
- Added `.agents/skills/team-message-intake/SKILL.md`.
- Added `.agents/skills/team-prompt-change-review/SKILL.md`.
- Linked the new prompt contract from feature and architecture indexes.
- Added a focused `agenthub-team-prompts` regression test for role boundaries, recovery pointers,
  skill/checklist pointers, output contracts, and tool-neutral prompt wording.

## Key Decisions

- Team prompts stay as bounded working sets, not full operating manuals.
- Static role prompts own role authority, communication contracts, required skill entry points, and
  output payload contracts.
- Runtime tails own only current objective, next action, allowed-action gate, compact blocker state,
  and recovery pointers.
- Repeated workflows should become skills or checklists before their steps are embedded in prompt
  prose.
- Team message routing now has a dedicated skill for choosing between visible replies, thread
  replies, mailbox updates, task notes, and canonical tasks.
- Prompt updates now have a dedicated review skill for classification, prompt budget,
  tool-neutrality, skill extraction, and test evidence.
- Open-source prompt contracts require tool-neutral searchable knowledge and pointer-addressable
  artifacts, not a named private memory backend.

## Validation

Validated with:

```bash
cargo fmt --check
cargo test -p agenthub-team-prompts -- --nocapture
git diff --check
ruby -ryaml -e 'ARGV.each { |path| text = File.read(path); fm = text.split("---", 3)[1]; data = YAML.safe_load(fm); raise "missing name" unless data["name"] =~ /\A[a-z0-9-]+\z/; raise "missing description" unless data["description"].is_a?(String) && !data["description"].empty?; puts "OK #{path}: #{data["name"]}" }' .agents/skills/team-message-intake/SKILL.md .agents/skills/team-prompt-change-review/SKILL.md
```

## Follow-Ups

- Extract the next repeated Team workflow into a skill/checklist before adding more prompt prose.
- Consider shrinking coordinator and worker prompt text further once the repeated workflow entry
  points are stable.

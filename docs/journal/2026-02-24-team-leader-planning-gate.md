# 2026-02-24 Team Leader Planning Gate

## Context

We reviewed external planner-oriented prompt design and aligned AgentHub Team leader guidance with a stronger planning quality gate.

## Goal

- keep leader in architect/reviewer role
- improve delegation quality before worker execution
- prevent ambiguous planning handoff

## Changes

1. Added planning quality gate to leader skill:
   - `Decision Complete`
   - `Explore Before Asking`
   - unknown split (discoverable facts vs preference/tradeoff)
   - delegation clearance checklist

2. Synced canonical Team role-skill runtime spec with the same contract.

3. Synced default Team leader prompt (Rust + Web) with the same gate wording.

4. Added regression assertions to keep planning-gate lines from drifting.

5. Added TODO verification entry for the planning-gate contract.

## Validation Plan

- Rust prompt crate tests
- Web team create helper tests

## Notes

This update is prompt/skill contract hardening. Runtime dispatch semantics are unchanged.

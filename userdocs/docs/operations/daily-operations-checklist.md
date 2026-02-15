---
sidebar_position: 1
---

# Daily Operations Checklist

Use this checklist to keep AgentHub runs consistent and low-risk.

## Before Starting Work

- Confirm AgentHub is reachable
- Confirm target repository path is within `safe_paths`
- Confirm a clean goal for each session

## During Execution

- Keep one main objective per prompt
- Watch status transitions (`running` -> terminal)
- Use **Interrupt** when output drifts or hangs

## Before Accepting Results

- Verify changed files match requested scope
- Verify validation commands are provided
- Run critical tests locally

## End of Day

- Stop stale or idle agents
- Clean up obsolete worktrees
- Keep failed sessions that may be useful for debugging

## Weekly Hygiene

- Review path/safety settings with the team
- Review recurring failures in troubleshooting logs
- Refine prompt templates for repeated tasks

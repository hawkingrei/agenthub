# ACP Rollout Integration

## Summary

Combine the original ACP slices 1-15 in a main-targeted integration branch. Native runtime slices
16-18 and their upstream prerequisites are deferred by the user. This checkpoint does not claim
production readiness before the remaining recovery and acceptance work is complete.

## Background

The earlier implementation PRs were merged into dependency branches, so their merged status did
not put the implementation into main. The pre-native head of PR #1162,
`7c671973cf116fa900bb7df290ee37e225e14233`, preserves the complete ACP implementation. The current
remote slice-15 branch subsequently received native transport and is not a suitable ACP-only base.

## Scope

Durable activation lifecycle and scheduling, offline configuration, canonical task/message intake,
shared journaled MCP, scoped Mem, role prompts, history and UI, versioned Apps and signed events.
Local Linux, explicit opt-in and default fresh sessions remain the supported initial boundary.

## Key Decisions

- Merge main normally into the pre-native implementation; do not rewrite or replay published history.
- Preserve main's dependency updates and the implemented feature contracts and journal navigation.
- Keep one main-targeted review surface with focused follow-up commits and an explicit slice map.
- Retain uncertain execution ownership until cleanup is verified. Restart recovery is a separate
  implementation gate, not a documentation-only readiness claim.
- Use ACP for this rollout. Preserve the deferred draft and uncommitted upstream work independently.

## Validation

PR #1162's applicable checks passed at its historical head. Those results do not validate this new
integration head. The merge introduces four documentation resolutions and retains main's web
dependency updates. Runtime source is unchanged from the reviewed ACP snapshot.

Current-head CI, installed-adapter acceptance and assembled-product checks remain pending. No local
Bazel command or build configuration change is part of this integration checkpoint.

## Follow-Ups

- Verify cleanup and recovery across daemon loss, including uncertain descendants and stale authority.
- Prove the installed ACP adapter path with a reproducible local provider fixture.
- Exercise scoped tools, events and retained browser history together.
- Update user/operator guidance to the actual supported behavior and finalize current-head CI.

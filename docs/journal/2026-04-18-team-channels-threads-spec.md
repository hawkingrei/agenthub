# Team Channels / Threads Spec

## Summary

- studied the Slock channel/thread page as a structure reference instead of copying its domain
  model verbatim
- extracted the parts AgentHub should learn:
  - compact Team-local `Channels` directory
  - center `channel timeline`
  - right-side `ThreadPane`
  - composer-adjacent lightweight task affordance
- clarified that `Open thread` should converge into a Team actor capability instead of remaining a
  front-end-only button
- clarified that Team should support multiple descriptive channels, with `# all` as the default
  broad coordination lane
- preserved the parts AgentHub must keep:
  - canonical Team task materialization through leader/runtime
  - `Kanban` as the primary task lane
  - `Execution Runs` as the execution/debug lane

## Output

- added [features/team-channels-threads.md](../features/team-channels-threads.md)
- linked the channel/thread direction back into
  [features/workspace-unified-ia.md](../features/workspace-unified-ia.md)
- added one focused follow-up item near the top of [todo.md](../todo.md)

## Notes

- this is intentionally a shell/information-architecture spec first
- implementation should start with routing and right-pane thread behavior before introducing
  multiple Team channels or composer task-draft affordances

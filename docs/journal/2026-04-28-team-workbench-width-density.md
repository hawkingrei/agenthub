## Team workbench width density

- Removed the inner `lg:max-w-[1180px]` cap from `web/src/pages/team/team_workbench_content.tsx`.
- The team workspace already sits inside `TEAM_PAGE_ROOT_CLASS` with `max-w-[1680px]`.
- Keeping both caps caused excessive left/right whitespace on wide and curved displays, especially compared with the denser Slock layout.
- The team workbench now expands to the shared root shell width instead of applying a second narrow content column.
- Follow-up: removed the outer `max-w-[1680px]` cap from `TEAM_PAGE_ROOT_CLASS` and trimmed large-screen horizontal shell padding so the Team workbench can use the available desktop width more like Slock.

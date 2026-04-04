export const AUTH_PAGE_CLASS =
  "app min-h-[var(--agenthub-vh,100vh)] px-4 py-8 md:px-6 md:py-10";

export const AUTH_CARD_BASE_CLASS =
  "auth mx-auto w-full max-w-md rounded-2xl border border-ui-border/80 bg-ui-surface/90 p-6 shadow-sm backdrop-blur";

export const AUTH_FORM_CARD_CLASS = `${AUTH_CARD_BASE_CLASS} flex flex-col gap-3`;

export const AUTH_INPUT_CLASS =
  "w-full rounded-lg border border-ui-border-strong bg-ui-surface px-ctrl-x py-ctrl-y text-ui-sm text-ui-text-primary outline-none transition focus:border-ui-border-emphasis focus:ring-2 focus:ring-ui-border";

export const AUTH_ACTIONS_CLASS = "auth-actions mt-1 flex flex-wrap gap-2";

export const AUTH_PRIMARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg bg-brand-primary px-ctrl-x py-ctrl-y text-ui-sm font-medium text-ui-text-inverse transition hover:bg-brand-primary-hover";

export const AUTH_SECONDARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg border border-ui-border-strong bg-ui-surface px-ctrl-x py-ctrl-y text-ui-sm font-medium text-ui-text-primary transition hover:border-ui-border-emphasis";

export const TEAM_PANEL_CARD_CLASS =
  "card team-panel-card min-h-0 rounded-[18px] border border-black/[0.05] bg-white/88 shadow-[0_1px_4px_rgba(15,23,42,0.04)] backdrop-blur-sm";

export const TEAM_PANEL_TOOLBAR_CLASS =
  "mb-3 flex flex-wrap items-center justify-between gap-2 rounded-[14px] border border-black/[0.05] bg-white/82 px-3 py-2 shadow-[0_1px_2px_rgba(15,23,42,0.03)]";

export const TEAM_PANEL_TOOLBAR_ACTIONS_CLASS =
  "actions flex w-full flex-wrap items-center gap-2 sm:w-auto sm:justify-end";

export const TEAM_PANEL_PRIMARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-[14px] border border-brand-primary bg-brand-primary px-ctrl-x py-ctrl-y text-ui-sm font-semibold text-ui-text-inverse shadow-[0_8px_16px_rgba(15,23,42,0.12)] transition hover:border-brand-primary-hover hover:bg-brand-primary-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ui-border-strong disabled:cursor-not-allowed disabled:opacity-60";

export const TEAM_PANEL_SECONDARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-[12px] border border-black/[0.06] bg-white/92 px-ctrl-x py-ctrl-y text-ui-sm font-semibold text-ui-text-secondary shadow-[0_1px_2px_rgba(15,23,42,0.04)] transition hover:border-black/[0.1] hover:bg-black/[0.03] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ui-border disabled:cursor-not-allowed disabled:opacity-60";

export const TEAM_PANEL_GHOST_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-[12px] border border-black/[0.06] bg-white/92 px-ctrl-x py-ctrl-y text-ui-sm font-semibold text-ui-text-secondary shadow-[0_1px_2px_rgba(15,23,42,0.04)] transition hover:border-black/[0.1] hover:bg-black/[0.03] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ui-border disabled:cursor-not-allowed disabled:opacity-60";

export const TEAM_PANEL_REFRESH_BUTTON_CLASS =
  "inline-flex items-center gap-1.5 rounded-[12px] border border-black/[0.06] bg-white/92 px-2.5 py-ctrl-y-sm text-ui-sm font-semibold text-ui-text-secondary shadow-[0_1px_2px_rgba(15,23,42,0.04)] transition hover:border-black/[0.1] hover:bg-black/[0.03] disabled:cursor-not-allowed disabled:opacity-60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ui-border";

export const TEAM_PANEL_INPUT_CLASS =
  "w-full rounded-lg border border-ui-border-strong bg-ui-surface px-ctrl-x py-ctrl-y text-ui-sm text-ui-text-primary outline-none transition focus:border-ui-border-emphasis focus:ring-2 focus:ring-ui-border";

export const TEAM_PANEL_TEXTAREA_CLASS =
  `mono min-h-24 ${TEAM_PANEL_INPUT_CLASS}`;

export const TEAM_PANEL_TITLE_CLASS = "text-lg font-semibold tracking-tight text-ui-text-primary";

export const TEAM_PANEL_PRE_CLASS =
  "mono max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border border-ui-border bg-ui-surface-soft p-2";

export const TEAM_TAB_BAR_CLASS =
  "tab-bar team-tab-bar flex min-w-0 gap-2 overflow-x-auto rounded-[16px] border border-black/[0.05] bg-white/84 p-1 shadow-[0_1px_3px_rgba(15,23,42,0.04)]";

const TEAM_TAB_BUTTON_BASE_CLASS =
  "tab shrink-0 rounded-[12px] px-3 py-1.5 text-ui-sm font-semibold transition";

export const TEAM_TAB_BUTTON_ACTIVE_CLASS =
  `${TEAM_TAB_BUTTON_BASE_CLASS} active border border-black/[0.06] bg-white text-ui-text-primary shadow-[0_1px_2px_rgba(15,23,42,0.04)]`;

export const TEAM_TAB_BUTTON_IDLE_CLASS =
  `${TEAM_TAB_BUTTON_BASE_CLASS} text-ui-text-muted hover:bg-black/[0.03] hover:text-ui-text-primary`;

export const TEAM_LIST_ITEM_BASE_CLASS =
  "team-item flex w-full min-w-0 flex-col items-start gap-1 rounded-[16px] border border-ui-border/90 bg-white/82 px-ctrl-x py-ctrl-y text-left text-ui-text-primary shadow-[0_4px_12px_rgba(15,23,42,0.03)] transition";

export const TEAM_LIST_ITEM_ACTIVE_CLASS =
  `${TEAM_LIST_ITEM_BASE_CLASS} border-ui-border-strong bg-[linear-gradient(180deg,rgba(255,255,255,0.94),rgba(248,250,252,0.96))] ring-1 ring-ui-border/70`;

export const TEAM_LIST_ITEM_IDLE_CLASS =
  `${TEAM_LIST_ITEM_BASE_CLASS} hover:border-ui-border-strong hover:bg-ui-surface-soft`;

export const TEAM_LIST_ITEM_TITLE_CLASS =
  "team-name w-full min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-sm font-semibold";

export const TEAM_LIST_ITEM_META_CLASS =
  "team-id mono w-full min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-xs opacity-90";

export const TEAM_CREATE_MODAL_BACKDROP_CLASS =
  "modal-backdrop team-create-modal-backdrop fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-brand-primary/40 px-3 py-6 sm:py-10";

export const TEAM_CREATE_MODAL_CARD_CLASS =
  "modal team-create-modal w-full max-w-5xl rounded-2xl border border-ui-border bg-ui-surface p-4 shadow-2xl sm:p-5";

export const TEAM_CREATE_STAGE_BADGE_CLASS =
  "badge rounded-full border border-ui-border-strong bg-ui-surface-muted px-2.5 py-1 text-ui-xs font-medium text-ui-text-secondary";

export const TEAM_CREATE_PANEL_CARD_CLASS =
  "team-create-panel rounded-xl border border-ui-border bg-ui-surface-soft/70 p-4";

export const TEAM_CREATE_NOTE_INFO_CLASS =
  "team-create-stage-note mt-2 rounded-lg border border-state-info-border bg-state-info-bg px-ctrl-x py-ctrl-y text-ui-sm text-state-info-text";

export const TEAM_CREATE_NOTE_WARNING_CLASS =
  "team-create-stage-note mt-2 rounded-lg border border-state-warning-border bg-state-warning-bg px-ctrl-x py-ctrl-y text-ui-sm text-state-warning-text";

export const TEAM_CREATE_STEP_PREVIEW_CLASS =
  "teams-step-body mono mt-3 rounded-lg border border-ui-border bg-ui-surface px-ctrl-x py-ctrl-y text-ui-xs text-ui-text-muted";

export const TEAM_CREATE_STEP_PREVIEW_MUTED_CLASS =
  "teams-step-body mono mt-3 rounded-lg border border-ui-border bg-ui-surface-soft px-ctrl-x py-ctrl-y text-ui-xs text-ui-text-muted";

export const TEAM_CREATE_SKILL_TAG_SELECTED_CLASS =
  "team-skill-tag selected rounded-full border border-brand-primary bg-brand-primary px-ctrl-x py-1 text-ui-xs font-medium text-ui-text-inverse transition";

export const TEAM_CREATE_SKILL_TAG_IDLE_CLASS =
  "team-skill-tag rounded-full border border-ui-border-strong bg-ui-surface px-ctrl-x py-1 text-ui-xs font-medium text-ui-text-secondary transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft";

export const TEAM_CREATE_WORKER_CARD_CLASS =
  "teams-worker-card rounded-xl border border-ui-border bg-ui-surface p-3 shadow-sm";

export const TEAM_CREATE_ACTIONS_BAR_CLASS =
  "modal-actions team-create-actions mt-4 flex flex-wrap items-center justify-end gap-2 border-t border-ui-border pt-3";

export const ACP_PANEL_ROOT_CLASS =
  "acp relative flex min-h-0 flex-1 flex-col bg-white";

export const ACP_PANEL_HEAD_CLASS =
  "acp-head minimal sticky top-0 z-20 flex flex-wrap items-center justify-between gap-3 bg-white/80 px-4 py-2 backdrop-blur-md sm:px-6 max-[720px]:px-3";

export const ACP_PANEL_TABS_CLASS =
  "acp-tabs flex items-center gap-1 p-0.5 max-[720px]:w-full";

export const ACP_TAB_BUTTON_BASE_CLASS =
  "inline-flex min-h-[28px] items-center rounded-md px-2.5 py-1 text-[13px] font-medium transition hover:bg-notion-hover";

export const ACP_TAB_BUTTON_ACTIVE_CLASS =
  `${ACP_TAB_BUTTON_BASE_CLASS} bg-notion-hover text-notion-text`;

export const ACP_TAB_BUTTON_IDLE_CLASS =
  `${ACP_TAB_BUTTON_BASE_CLASS} text-notion-text-muted`;

export const ACP_TAB_BADGE_CLASS =
  "acp-tab-badge ml-1.5 rounded-sm bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold text-notion-text-muted";

export const ACP_JUMP_BOTTOM_BUTTON_CLASS =
  "acp-jump-bottom absolute bottom-24 right-6 z-40 inline-flex h-9 w-9 items-center justify-center rounded-full border border-notion-border bg-white shadow-md text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text active:translate-y-px";

export const ACP_DEBUG_ROOT_CLASS =
  "acp-debug flex min-h-0 flex-1 flex-col gap-6 p-4 sm:p-8 sm:max-w-4xl sm:mx-auto";

export const ACP_DEBUG_TABS_CLASS =
  "acp-debug-tabs flex flex-wrap gap-1 rounded-lg bg-notion-sidebar p-1 border border-notion-border";

export const ACP_DEBUG_SECTION_CLASS =
  "space-y-4 rounded-xl border border-notion-border bg-white p-4 sm:p-6";

export const ACP_DEBUG_EMPTY_CLASS =
  "empty rounded-lg border border-dashed border-notion-border bg-notion-sidebar px-4 py-8 text-center text-sm text-notion-text-muted";

export const ACP_DEBUG_PERMISSION_TOGGLE_CLASS =
  "acp-permission-toggle flex min-w-0 flex-1 flex-col items-start gap-1 rounded-lg border border-transparent px-3 py-2 text-left transition hover:bg-notion-hover disabled:cursor-not-allowed disabled:opacity-60";

export const ACP_DEBUG_PERMISSION_SUBMETA_CLASS =
  "acp-permission-submeta mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-notion-text-muted";

export const ACP_DEBUG_PERMISSION_WARNING_CLASS =
  "acp-permission-options mono mt-2 rounded-lg border border-state-warning-border bg-state-warning-bg px-3 py-2 text-[11px] text-state-warning-text";

export const ACP_DEBUG_RAW_PRE_CLASS =
  "acp-content mt-2 overflow-auto rounded-lg border border-notion-border bg-white p-3 text-[12px] leading-relaxed text-notion-text";

export const ACP_CONVERSATION_TOP_HINT_CLASS =
  "acp-conversation-top-hint mx-auto my-6 rounded-full border border-notion-border bg-notion-sidebar px-4 py-1 text-[11px] font-medium text-notion-text-muted";

export const ACP_BUBBLE_THINKING_CLASS =
  "acp-bubble agent_thinking rounded-lg border border-notion-border/60 bg-notion-sidebar/40 px-4 py-3 italic text-notion-text-muted/80";

export const ACP_BUBBLE_PLAN_CLASS =
  "acp-bubble agent_plan rounded-lg border border-notion-border bg-white px-4 py-3 shadow-sm";

export const ACP_PLAN_INDEX_BADGE_CLASS =
  "acp-plan-index inline-flex h-5 w-5 items-center justify-center rounded-md bg-notion-hover text-[11px] font-bold text-notion-text-muted";

export const ACP_PLAN_PRIORITY_BADGE_CLASS =
  "acp-plan-priority rounded-md bg-state-warning-bg px-2 py-0.5 text-[10px] font-bold text-state-warning-text uppercase tracking-wider";

export const ACP_PLAN_STATUS_BADGE_CLASS =
  "acp-plan-status rounded-md bg-notion-hover px-2 py-0.5 text-[10px] font-bold text-notion-text-muted uppercase tracking-wider";

export const ACP_SEGMENTED_NOTE_WARNING_CLASS =
  "acp-segmented-note rounded-lg border border-state-warning-border bg-state-warning-bg px-3 py-2 text-[12px] text-state-warning-text";

export const ACP_SEGMENTED_BUTTON_CLASS =
  "acp-segmented-button inline-flex items-center justify-center rounded-md border border-notion-border bg-white px-3 py-1.5 text-[12px] font-medium text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text";

export const ACP_TERMINAL_PRE_CLASS =
  "acp-content rounded-lg border border-notion-border bg-brand-primary p-3 text-[12px] text-white shadow-inner";

export const ACP_DIFF_PRE_CLASS =
  "acp-content acp-diff-view m-0 overflow-auto whitespace-pre rounded-lg border border-slate-700 bg-slate-950 p-3 text-[12px] text-slate-100 leading-relaxed max-[720px]:text-[11px]";

export const ACP_TOOL_STATUS_CLASS =
  "acp-tool-status ml-auto mt-px inline-flex shrink-0 items-center self-start whitespace-nowrap rounded-md border px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider leading-tight";

export const ACP_TOOL_STATUS_SINGLE_DEFAULT_CLASS =
  "border-state-warning-border bg-state-warning-bg text-state-warning-text";

export const ACP_TOOL_STATUS_GROUP_DEFAULT_CLASS =
  "border-state-warning-border bg-state-warning-bg text-state-warning-text";

export const ACP_TOOL_STATUS_GROUP_SUCCESS_CLASS =
  "border-state-success-border bg-state-success-bg text-state-success-text";

export const ACP_TOOL_STATUS_GROUP_FAILURE_CLASS =
  "border-state-info-border bg-state-info-bg text-state-info-text";

export const ACP_TOOL_STATUS_GROUP_RUNNING_CLASS =
  "border-notion-accent/30 bg-notion-accent-bg text-notion-accent";

export const TEAM_MUTED_TEXT_CLASS = "muted text-[13px] text-notion-text-muted";

export const TEAM_SIDEBAR_ROOT_CLASS =
  "teams-sidebar flex min-h-0 min-w-0 flex-col gap-4 overflow-x-hidden overflow-y-auto bg-notion-sidebar p-4 border-r border-notion-border";

export const TEAM_SIDEBAR_SWITCHER_BUTTON_CLASS =
  "flex w-full items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left transition hover:bg-notion-hover";

export const TEAM_SIDEBAR_SWITCHER_PANEL_CLASS =
  "mt-1 space-y-1 border-l border-notion-border ml-3 pl-3";

export const TEAM_SIDEBAR_SWITCHER_ACTIONS_CLASS = "mt-4 flex flex-wrap gap-2 px-2";

export const TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS =
  "inline-flex h-8 w-8 items-center justify-center rounded-md border border-notion-border bg-white text-notion-text-muted shadow-sm transition hover:bg-notion-hover hover:text-notion-text";

export const TEAM_SIDEBAR_SCOPE_SWITCH_CLASS =
  "flex items-center gap-1 p-1 bg-notion-hover rounded-lg mx-2";

const TEAM_SIDEBAR_SCOPE_BUTTON_BASE_CLASS =
  "inline-flex min-w-0 flex-1 items-center justify-center rounded-md px-2 py-1 text-[11px] font-bold uppercase tracking-wider transition";

export const TEAM_SIDEBAR_SCOPE_BUTTON_ACTIVE_CLASS =
  `${TEAM_SIDEBAR_SCOPE_BUTTON_BASE_CLASS} bg-white text-notion-text shadow-sm`;

export const TEAM_SIDEBAR_SCOPE_BUTTON_IDLE_CLASS =
  `${TEAM_SIDEBAR_SCOPE_BUTTON_BASE_CLASS} text-notion-text-muted hover:text-notion-text`;

export const TEAM_SIDEBAR_SECTION_CLASS = "flex min-w-0 flex-col gap-1";

export const TEAM_SIDEBAR_SECTION_TOGGLE_CLASS =
  "appearance-none border-0 bg-transparent shadow-none flex w-full items-center justify-between gap-2 px-2 py-1 text-left text-[11px] font-bold uppercase tracking-widest text-notion-text-muted transition hover:text-notion-text";

export const TEAM_SIDEBAR_NAV_LIST_CLASS =
  "flex min-w-0 flex-col gap-0.5";

const TEAM_SIDEBAR_NAV_ITEM_BASE_CLASS =
  "flex w-full min-w-0 flex-col items-start gap-0.5 rounded-md px-3 py-1.5 text-left transition";

export const TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS =
  `${TEAM_SIDEBAR_NAV_ITEM_BASE_CLASS} bg-notion-hover text-notion-text font-medium`;

export const TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS =
  `${TEAM_SIDEBAR_NAV_ITEM_BASE_CLASS} text-notion-text-muted hover:bg-notion-hover hover:text-notion-text`;

export const TEAM_SIDEBAR_NAV_ITEM_META_CLASS =
  "mono w-full min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-[10px] opacity-60";

export const TEAM_SIDEBAR_WORKFLOW_ACTIVE_CLASS =
  "flex w-full items-center gap-2 rounded-md bg-notion-hover px-3 py-1.5 text-left transition text-notion-text font-medium";

export const TEAM_SIDEBAR_WORKFLOW_IDLE_CLASS =
  "flex w-full items-center gap-2 rounded-md bg-transparent px-3 py-1.5 text-left transition text-notion-text-muted hover:bg-notion-hover hover:text-notion-text";

export const TEAM_SIDEBAR_WORK_CLASS =
  "truncate pl-5 text-[11px] leading-relaxed text-notion-text-muted opacity-80";

export const TEAM_SIDEBAR_BADGE_CLASS =
  "shrink-0 rounded-sm bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold text-notion-text-muted";

export const TEAM_SIDEBAR_INDICATOR_DOT_CLASS = "h-2 w-2 shrink-0 rounded-full mt-0.5";

export const TEAM_TASK_COMPOSER_PANEL_CLASS =
  "flex shrink-0 flex-col gap-2 border-t border-notion-border bg-white/90 px-4 py-3 shadow-lg backdrop-blur-md";

export const TEAM_TASK_ACTIVITY_LIST_CLASS = "min-h-0 flex-1 overflow-y-auto";

export const TEAM_TASK_ACTIVITY_SHELL_CLASS = "bg-white min-h-full py-4";

export const TEAM_TASK_ACTIVITY_STACK_CLASS = "flex w-full flex-col";

export const TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS =
  "group relative flex w-full max-w-full gap-4 px-4 py-2 sm:px-8 transition-colors hover:bg-notion-hover/30";

export const TEAM_TASK_ACTIVITY_ITEM_HUMAN_CLASS = TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS;

export const TEAM_TASK_ACTIVITY_ITEM_AGENT_CLASS = TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS;

export const TEAM_TASK_ACTIVITY_AUTHOR_CLASS =
  "text-[13px] font-bold text-notion-text";

export const TEAM_TASK_ACTIVITY_TIME_CLASS =
  "text-[10px] font-bold uppercase tracking-wider text-notion-text-muted opacity-0 group-hover:opacity-100 transition-opacity";

export const TEAM_TASK_ACTIVITY_BODY_CLASS =
  "text-[15px] leading-relaxed text-notion-text";

export const TEAM_TASK_ACTIVITY_COMMAND_BODY_CLASS =
  "mono mt-2 max-w-full overflow-x-auto whitespace-pre rounded-lg border border-notion-border bg-notion-sidebar p-3 text-[12px] leading-relaxed text-notion-text";

export const TEAM_TASK_PERMISSION_CARD_CLASS =
  "mt-2 rounded-lg border border-notion-border bg-notion-sidebar/30 p-4 shadow-sm";

export const TEAM_TASK_PERMISSION_CARD_COMPACT_CLASS =
  "mt-2 rounded-lg border border-notion-border bg-white px-3 py-2 shadow-sm";

export const TEAM_TASK_PERMISSION_CARD_HEADER_CLASS =
  "flex flex-wrap items-center justify-between gap-2";

export const TEAM_TASK_PERMISSION_CARD_TITLE_CLASS =
  "text-[13px] font-bold tracking-tight text-notion-text";

export const TEAM_TASK_PERMISSION_CARD_STATUS_CLASS =
  "inline-flex items-center rounded-sm bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted";

export const TEAM_TASK_PERMISSION_CARD_COMPACT_PREVIEW_CLASS =
  "mono mt-1 max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-[10px] text-notion-text-muted opacity-70";

export const TEAM_TASK_PERMISSION_CARD_BODY_CLASS =
  "mt-3 space-y-3 text-[14px] leading-relaxed text-notion-text";

export const TEAM_TASK_PERMISSION_CARD_REASON_CLASS =
  "text-[11px] font-bold uppercase tracking-wider text-notion-text-muted";

export const TEAM_TASK_PERMISSION_CARD_ACTIONS_CLASS =
  "mt-4 flex flex-wrap items-center gap-2";

export const TEAM_TASK_PERMISSION_CARD_SECONDARY_BUTTON_CLASS =
  "inline-flex h-8 items-center justify-center rounded-md border border-notion-border bg-white px-3 text-[12px] font-medium text-notion-text shadow-sm transition hover:bg-notion-hover active:translate-y-px";

export const TEAM_TASK_JUMP_BUTTON_CLASS =
  "inline-flex h-9 w-9 items-center justify-center rounded-full border border-notion-border bg-white text-notion-text-muted shadow-md transition hover:bg-notion-hover hover:text-notion-text active:translate-y-px";

export const TEAM_MEMBER_CARD_CLASS =
  "flex min-w-0 flex-col gap-1 rounded-lg border border-notion-border bg-white px-3 py-2 shadow-sm transition-all hover:bg-notion-hover/30";

export const TEAM_MEMBER_NAME_CLASS = "min-w-0 flex-1 truncate text-[14px] font-bold text-notion-text";

export const TEAM_MEMBER_META_CLASS = "mono truncate text-[10px] text-notion-text-muted opacity-70";

export const TEAM_MEMBER_SUMMARY_CLASS = "flex flex-wrap items-center gap-1.5 text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";

export const ADMIN_APP_CLASS =
  "app min-h-screen bg-white text-notion-text";

export const ADMIN_HEADER_CLASS =
  "flex items-center justify-between border-b border-notion-border bg-white/90 px-5 py-3 backdrop-blur sticky top-0 z-30";

export const ADMIN_TITLE_CLASS = "text-lg font-bold tracking-tight text-notion-text";

export const ADMIN_SESSION_CLASS = "session flex items-center gap-2 text-sm text-notion-text-muted";

export const ADMIN_SECTION_CLASS =
  "admin mx-auto flex w-full max-w-5xl flex-col gap-6 px-4 py-8";

export const ADMIN_TOOLBAR_CLASS =
  "admin-toolbar flex flex-wrap items-center justify-between gap-3 rounded-xl border border-notion-border bg-notion-sidebar/30 px-6 py-4 shadow-sm";

export const ADMIN_TAB_BAR_CLASS =
  "admin-tab-bar flex flex-wrap gap-1 rounded-lg bg-notion-sidebar p-1 border border-notion-border";

export const ADMIN_TAB_BUTTON_BASE_CLASS =
  "admin-tab-button inline-flex items-center rounded-md px-3 py-1.5 text-[13px] font-bold uppercase tracking-wider transition";

export const ADMIN_TAB_BUTTON_ACTIVE_CLASS =
  "bg-white text-notion-text shadow-sm";

export const ADMIN_TAB_BUTTON_IDLE_CLASS =
  "text-notion-text-muted hover:text-notion-text hover:bg-notion-hover";

export const ADMIN_CARD_CLASS =
  "admin-card rounded-xl border border-notion-border bg-white p-6 shadow-sm";

export const ADMIN_CARD_TITLE_CLASS = "mb-4 text-[11px] font-bold uppercase tracking-widest text-notion-text-muted";

export const ADMIN_FORM_ROW_CLASS = "form-row mb-4 flex flex-wrap items-center gap-3";

export const ADMIN_INPUT_CLASS =
  "min-w-[16rem] flex-1 rounded-md border border-notion-border bg-white px-3 py-2 text-[14px] text-notion-text outline-none transition focus:border-notion-accent focus:ring-2 focus:ring-notion-accent/10";

export const ADMIN_PRIMARY_BUTTON_CLASS =
  "inline-flex h-9 items-center justify-center rounded-md bg-notion-accent px-4 text-[13px] font-bold text-white shadow-sm transition hover:bg-notion-accent/90 disabled:opacity-50 disabled:cursor-not-allowed active:translate-y-px";

export const ADMIN_SECONDARY_BUTTON_CLASS =
  "inline-flex h-9 items-center justify-center rounded-md border border-notion-border bg-white px-4 text-[13px] font-medium text-notion-text shadow-sm transition hover:bg-notion-hover disabled:opacity-50 active:translate-y-px";

export const ADMIN_DANGER_BUTTON_CLASS =
  "inline-flex h-9 items-center justify-center rounded-md border border-red-200 bg-red-50 px-4 text-[13px] font-medium text-red-700 transition hover:bg-red-100 active:translate-y-px";

export const ADMIN_LIST_CLASS = "space-y-1.5";

export const ADMIN_LIST_ITEM_CLASS =
  "flex flex-wrap items-center gap-3 rounded-lg border border-notion-border bg-notion-sidebar/20 px-4 py-3 transition-colors hover:bg-notion-hover/40";

export const ADMIN_MUTED_TEXT_CLASS = "text-[13px] leading-relaxed text-notion-text-muted";

export const ADMIN_QR_CLASS = "mt-4 max-w-xs rounded-xl border border-notion-border bg-white p-4 shadow-md";

export const ADMIN_KV_LIST_CLASS = "kv-list space-y-3";

export const ADMIN_KV_ROW_CLASS =
  "kv-row grid gap-1 sm:grid-cols-[10rem_1fr] sm:items-start pb-3 border-b border-notion-border/50 last:border-0";

export const ADMIN_LABEL_CLASS = "label text-[11px] font-bold uppercase tracking-widest text-notion-text-muted mt-1";

export const ADMIN_VALUE_CLASS = "value break-all text-[14px] font-medium text-notion-text";

export const ADMIN_EMPTY_TEXT_CLASS = "text-sm text-notion-text-muted italic py-4";

export const TEAM_SIDEBAR_FORGE_CARD_CLASS =
  "teams-form teams-create-launch flex flex-col gap-3 rounded-xl border border-notion-border bg-white p-4 shadow-sm";

export const TEAM_SIDEBAR_INFO_CARD_CLASS =
  "rounded-lg border border-notion-border bg-white px-3 py-2 shadow-sm";

export const TEAM_SIDEBAR_INFO_LABEL_CLASS =
  "text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";

export const TEAM_SIDEBAR_INFO_TEXT_CLASS = "mt-0.5 text-[13px] text-notion-text";

export const TEAM_SIDEBAR_META_GRID_CLASS =
  "teams-create-launch-meta mono mt-3 grid gap-1 text-[10px] text-notion-text-muted";

export const AGENTS_PANEL_EXPANDED_CLASS =
  "workspace-left rounded-lg border-r border-notion-border bg-notion-sidebar p-2 text-notion-text backdrop-blur-sm transition-all duration-300";

export const AGENTS_PANEL_COLLAPSED_CLASS =
  "workspace-left collapsed rounded-lg border-r border-notion-border bg-notion-sidebar p-2 text-notion-text backdrop-blur-sm transition-all duration-300";

export const AGENTS_TOOLBAR_CLASS = "mb-4 flex items-center justify-between px-2 pt-2";
export const AGENTS_TOOLBAR_ACTIONS_CLASS = "flex items-center gap-1.5";

export const AGENTS_CREATE_BUTTON_CLASS =
  "flex h-8 items-center gap-1.5 rounded-md border border-notion-border bg-white px-3 text-xs font-semibold text-notion-text shadow-sm transition hover:bg-notion-hover hover:text-notion-accent active:translate-y-px";

export const AGENTS_ROW_CLASS =
  "agents-workbench-row group relative flex flex-col gap-1 rounded-md border border-transparent px-3 py-2 text-notion-text transition-all hover:bg-notion-hover";

export const AGENTS_ROW_ACTIVE_CLASS =
  "agents-workbench-row active group relative flex flex-col gap-1 rounded-md border border-transparent bg-notion-hover px-3 py-2 text-notion-text ring-1 ring-notion-accent/20";

export const OUTPUT_HEADER_ROOT_CLASS =
  "output-header sticky top-0 z-30 grid grid-cols-[minmax(0,1fr)_auto] grid-rows-[auto_auto] items-center justify-between gap-x-3 gap-y-1 bg-white/80 px-4 py-3 backdrop-blur-md transition-all sm:px-6 sm:py-4 max-[720px]:px-3 max-[720px]:py-2";

export const OUTPUT_HEADER_TITLE_CLASS =
  "output-title col-start-1 row-start-1 inline-flex min-w-0 w-fit max-w-[clamp(240px,45vw,640px)] items-center gap-3 max-[720px]:w-full max-[720px]:max-w-full";

export const OUTPUT_HEADER_TITLE_TEXT_CLASS =
  "output-title-text flex min-w-0 flex-col gap-0";

export const OUTPUT_HEADER_TITLE_MAIN_CLASS =
  "output-title-main flex min-w-0 flex-wrap items-center gap-3 max-[720px]:gap-2";

export const OUTPUT_HEADER_TITLE_HEADING_CLASS =
  "m-0 overflow-hidden text-ellipsis whitespace-nowrap text-[22px] font-bold tracking-tight text-notion-text sm:text-[32px] sm:leading-[1.1]";

export const OUTPUT_HEADER_META_CLASS =
  "output-meta col-start-2 row-start-1 flex max-w-[40vw] flex-nowrap items-center justify-self-end gap-3 overflow-hidden whitespace-nowrap text-[11px] text-notion-text-muted max-[720px]:hidden [&>*]:min-w-0 [&>*]:overflow-hidden [&>*]:text-ellipsis [&>*]:whitespace-nowrap";

export const OUTPUT_HEADER_DETAILS_ROOT_CLASS = "output-header-details relative";

export const OUTPUT_HEADER_DETAILS_SUMMARY_CLASS =
  "inline-flex cursor-pointer list-none items-center rounded-md border border-notion-border bg-white/72 px-2 py-0.5 text-[11px] font-medium text-notion-text-muted transition hover:border-notion-accent/30 hover:bg-notion-hover hover:text-notion-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-notion-accent/10";

export const OUTPUT_HEADER_DETAILS_PANEL_CLASS =
  "absolute right-0 top-[calc(100%+8px)] z-20 min-h-0 min-w-[220px] rounded-lg border border-notion-border bg-white/95 px-3 py-2 shadow-xl backdrop-blur-md";

export const OUTPUT_HEADER_DETAILS_LIST_CLASS = "flex flex-col gap-1.5";

export const OUTPUT_HEADER_DETAILS_ITEM_CLASS =
  "grid grid-cols-[auto_minmax(0,1fr)] items-start gap-x-2 gap-y-0.5 text-[11px] text-notion-text";

export const OUTPUT_HEADER_DETAILS_LABEL_CLASS =
  "text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";

export const OUTPUT_HEADER_DETAILS_VALUE_CLASS =
  "mono min-w-0 break-all text-[11px] leading-5 text-notion-text opacity-80";

export const OUTPUT_HEADER_PILL_CLASS =
  "output-pill rounded-md border border-notion-border bg-notion-sidebar px-2 py-0.5 text-[11px] font-medium text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text";

export const OUTPUT_HEADER_SESSION_CLASS = "output-session mono text-[11px] text-notion-text-muted/60";

export const OUTPUT_HEADER_UPDATED_CLASS = "output-updated text-[11px] text-notion-text-muted/60";

export const OUTPUT_HEADER_SUBTITLE_ROW_CLASS =
  "output-subtitle-row col-start-1 row-start-2 mt-0.5 flex min-w-0 items-center justify-self-start max-w-full max-[720px]:hidden";

export const OUTPUT_HEADER_SUBTITLE_CLASS =
  "output-subtitle overflow-hidden text-ellipsis whitespace-nowrap text-[13px] text-notion-text-muted leading-relaxed";

export const OUTPUT_BODY_ROOT_CLASS =
  "output-body flex flex-col min-h-0 flex-1 bg-white";

export const OUTPUT_BODY_ACP_ROOT_CLASS =
  "output-body output-body-acp min-h-0 flex-1 bg-white";

export const OUTPUT_BODY_LOADING_CLASS =
  "output-loading flex h-full min-h-40 flex-col items-center justify-center gap-3 text-notion-text-muted";

export const OUTPUT_BODY_EMPTY_CLASS =
  "output-empty flex h-full min-h-40 flex-col items-center justify-center gap-2 px-6 text-center";

export const INPUT_DOCK_ROOT_CLASS =
  "input docked sticky bottom-6 left-1/2 -translate-x-1/2 w-[min(calc(100vw-32px),840px)] z-50 flex flex-col gap-2 rounded-xl border border-notion-border bg-white/90 p-3 shadow-xl backdrop-blur-md transition-all sm:p-4";

export const INPUT_DOCK_INTERRUPT_BUTTON_CLASS =
  "acp-interrupt-button input-interrupt-button rounded-md border border-state-warning-border bg-state-warning-bg px-3 py-1 text-[12px] font-bold text-state-warning-text hover:bg-state-warning-bg/80 transition active:translate-y-px";

export const INPUT_DOCK_HISTORY_BUTTON_CLASS =
  "history-toggle rounded-md border border-notion-border bg-white px-3 py-1 text-[12px] font-bold text-notion-text-muted hover:bg-notion-hover transition active:translate-y-px";

export const INPUT_DOCK_HISTORY_MENU_CLASS =
  "input-history-menu rounded-xl border border-notion-border bg-white p-1.5 shadow-2xl backdrop-blur-md";

export const INPUT_DOCK_HISTORY_ITEM_CLASS =
  "input-history-item block w-full rounded-lg px-3 py-2 text-left text-[13px] text-notion-text hover:bg-notion-hover transition";

export const INPUT_DOCK_TEXTAREA_CLASS =
  "min-h-[2.5rem] w-full bg-transparent px-1 py-1 text-[15px] text-notion-text placeholder-notion-text-muted/50 outline-none transition resize-none";

export const INPUT_DOCK_SEND_BUTTON_CLASS =
  "inline-flex h-8 items-center justify-center rounded-md bg-notion-accent px-4 py-1 text-[13px] font-bold text-white transition hover:bg-notion-accent/90 disabled:opacity-50 disabled:cursor-not-allowed active:translate-y-px";

export const TEAM_PAGE_ROOT_CLASS =
  "mx-auto flex h-[var(--agenthub-vh,100vh)] w-full max-w-[1680px] flex-col gap-3 overflow-y-auto overscroll-y-contain bg-white px-3 py-2 sm:px-4 lg:px-6 [&>*]:shrink-0";

export const TEAM_SECTION_CARD_CLASS =
  "min-h-0 min-w-0 rounded-lg border border-notion-border bg-white px-3 py-2 shadow-sm transition-all";

export const TEAM_SECTION_CARD_LARGE_CLASS =
  "min-h-0 rounded-xl border border-notion-border bg-white p-4 shadow-md transition-all sm:p-6";

export const TEAM_SECTION_HEADING_CLASS =
  "text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";

export const TEAM_SECTION_TITLE_CLASS = "text-lg font-bold tracking-tight text-notion-text";

export const TEAM_SECTION_BODY_TEXT_CLASS = "mt-2 text-[14px] leading-relaxed text-notion-text";

export const TEAM_SECTION_HINT_TEXT_CLASS = "mt-2 text-[12px] leading-relaxed text-notion-text-muted";

export const TEAM_DEBUG_TABS_CLASS =
  "flex flex-wrap items-center gap-1 bg-notion-sidebar p-1 rounded-lg border border-notion-border";

export const TEAM_DEBUG_TAB_ACTIVE_CLASS =
  "inline-flex items-center rounded-md bg-white px-3 py-1 text-[11px] font-bold uppercase tracking-wider text-notion-text shadow-sm";

export const TEAM_DEBUG_TAB_IDLE_CLASS =
  "inline-flex items-center rounded-md px-3 py-1 text-[11px] font-bold uppercase tracking-wider text-notion-text-muted transition hover:text-notion-text hover:bg-notion-hover";

export const TEAM_WORKBENCH_PANEL_CLASS =
  "rounded-xl border border-notion-border bg-white p-2 shadow-lg backdrop-blur-md";

export const TEAM_WORKBENCH_HEADER_SHELL_CLASS =
  "flex flex-wrap items-center justify-between gap-3 bg-white px-4 py-3 sm:px-6 sm:py-4 border-b border-notion-border transition-all";

export const TEAM_WORKBENCH_HEADER_ICON_BUTTON_CLASS =
  "inline-flex h-8 w-8 items-center justify-center rounded-md text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text";

export const TEAM_WORKBENCH_HEADER_STATUS_CLASS =
  "inline-flex items-center gap-1.5 rounded-md border border-notion-border bg-notion-sidebar px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted transition hover:bg-notion-hover";

export const TEAM_WORKBENCH_WORKSPACE_SHELL_CLASS =
  "rounded-xl border border-notion-border bg-white px-4 py-3 shadow-sm transition-all";

export const TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS =
  "min-w-0 bg-white px-4 py-3 border-r border-notion-border last:border-r-0";

export const TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS =
  "text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";

export const TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS =
  "mt-1 text-[13px] leading-relaxed text-notion-text";

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
  "card team-panel-card min-h-0 rounded-2xl border border-ui-border bg-ui-surface shadow-sm";

export const TEAM_PANEL_TOOLBAR_CLASS =
  "mb-3 flex flex-wrap items-center justify-between gap-2";

export const TEAM_PANEL_TOOLBAR_ACTIONS_CLASS =
  "actions flex w-full flex-wrap items-center gap-2 sm:w-auto sm:justify-end";

export const TEAM_PANEL_PRIMARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg bg-brand-primary px-ctrl-x py-ctrl-y text-ui-sm font-medium text-ui-text-inverse shadow-sm transition hover:bg-brand-primary-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ui-border-strong disabled:cursor-not-allowed disabled:opacity-60";

export const TEAM_PANEL_SECONDARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg border border-ui-border-strong bg-ui-surface px-ctrl-x py-ctrl-y text-ui-sm font-medium text-ui-text-secondary shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ui-border disabled:cursor-not-allowed disabled:opacity-60";

export const TEAM_PANEL_GHOST_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg border border-ui-border-strong bg-ui-surface px-ctrl-x py-ctrl-y text-ui-sm font-medium text-ui-text-secondary shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ui-border disabled:cursor-not-allowed disabled:opacity-60";

export const TEAM_PANEL_REFRESH_BUTTON_CLASS =
  "inline-flex items-center gap-1.5 rounded-lg border border-ui-border-strong bg-ui-surface px-2.5 py-ctrl-y-sm text-ui-sm font-medium text-ui-text-secondary shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft disabled:cursor-not-allowed disabled:opacity-60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ui-border";

export const TEAM_PANEL_INPUT_CLASS =
  "w-full rounded-lg border border-ui-border-strong bg-ui-surface px-ctrl-x py-ctrl-y text-ui-sm text-ui-text-primary outline-none transition focus:border-ui-border-emphasis focus:ring-2 focus:ring-ui-border";

export const TEAM_PANEL_TEXTAREA_CLASS =
  `mono min-h-24 ${TEAM_PANEL_INPUT_CLASS}`;

export const TEAM_PANEL_TITLE_CLASS = "text-lg font-semibold tracking-tight text-ui-text-primary";

export const TEAM_PANEL_PRE_CLASS =
  "mono max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border border-ui-border bg-ui-surface-soft p-2";

export const TEAM_TAB_BAR_CLASS =
  "tab-bar team-tab-bar flex min-w-0 gap-2 overflow-x-auto rounded-xl border border-ui-border bg-ui-surface p-2 shadow-sm";

const TEAM_TAB_BUTTON_BASE_CLASS =
  "tab shrink-0 rounded-lg px-ctrl-x py-ctrl-y text-ui-sm font-medium transition";

export const TEAM_TAB_BUTTON_ACTIVE_CLASS =
  `${TEAM_TAB_BUTTON_BASE_CLASS} active bg-brand-primary text-ui-text-inverse shadow-sm`;

export const TEAM_TAB_BUTTON_IDLE_CLASS =
  `${TEAM_TAB_BUTTON_BASE_CLASS} text-ui-text-muted hover:bg-ui-surface-muted hover:text-ui-text-primary`;

export const TEAM_LIST_ITEM_BASE_CLASS =
  "team-item flex w-full min-w-0 flex-col items-start gap-1 rounded-lg border border-ui-border bg-ui-surface px-ctrl-x py-ctrl-y text-left text-ui-text-primary transition";

export const TEAM_LIST_ITEM_ACTIVE_CLASS =
  `${TEAM_LIST_ITEM_BASE_CLASS} border-ui-border-strong bg-ui-surface-soft ring-1 ring-ui-border`;

export const TEAM_LIST_ITEM_IDLE_CLASS =
  `${TEAM_LIST_ITEM_BASE_CLASS} hover:border-ui-border-strong`;

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
  "acp relative flex min-h-0 flex-1 flex-col rounded-2xl border border-ui-border bg-ui-surface/90 shadow-sm max-[720px]:shadow-none";

export const ACP_PANEL_HEAD_CLASS =
  "acp-head minimal flex flex-wrap items-start justify-between gap-3 border-b border-ui-border px-ctrl-x py-ctrl-y sm:px-4 max-[720px]:gap-1.5 max-[720px]:border-b-0 max-[720px]:px-2 max-[720px]:py-1.5";

export const ACP_PANEL_TABS_CLASS =
  "acp-tabs flex items-center gap-2 rounded-lg border border-ui-border bg-ui-surface-soft p-1 max-[720px]:w-full max-[720px]:rounded-[10px] max-[720px]:px-[2px] max-[720px]:py-[2px]";

export const ACP_TAB_BUTTON_BASE_CLASS =
  "inline-flex min-h-[30px] items-center rounded-md px-ctrl-x py-ctrl-y-sm text-ui-xs font-medium leading-tight transition sm:text-ui-sm";

export const ACP_TAB_BUTTON_ACTIVE_CLASS =
  `${ACP_TAB_BUTTON_BASE_CLASS} bg-brand-primary text-ui-text-inverse shadow-sm`;

export const ACP_TAB_BUTTON_IDLE_CLASS =
  `${ACP_TAB_BUTTON_BASE_CLASS} text-ui-text-muted hover:bg-ui-surface hover:text-ui-text-primary`;

export const ACP_TAB_BADGE_CLASS =
  "acp-tab-badge rounded-full border border-current/30 px-1.5 py-0.5 text-[10px] font-semibold leading-none sm:text-xs";

export const ACP_JUMP_BOTTOM_BUTTON_CLASS =
  "acp-jump-bottom absolute bottom-3 right-3 z-20 inline-flex h-8 w-8 items-center justify-center rounded-full border border-ui-border-strong bg-ui-surface text-ui-text-secondary shadow-md transition hover:border-ui-border-emphasis focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ui-border-strong";

export const ACP_DEBUG_ROOT_CLASS =
  "acp-debug flex min-h-0 flex-1 flex-col gap-3 p-3 sm:p-4";

export const ACP_DEBUG_TABS_CLASS =
  "acp-debug-tabs flex flex-wrap gap-2 rounded-lg border border-ui-border bg-ui-surface-soft p-1";

export const ACP_DEBUG_SECTION_CLASS =
  "space-y-3 rounded-xl border border-ui-border bg-ui-surface p-3 shadow-sm sm:p-4";

export const ACP_DEBUG_EMPTY_CLASS =
  "empty rounded-lg border border-dashed border-ui-border-strong bg-ui-surface-soft px-ctrl-x py-5 text-ui-sm text-ui-text-muted";

export const ACP_DEBUG_PERMISSION_TOGGLE_CLASS =
  "acp-permission-toggle flex min-w-0 flex-1 flex-col items-start gap-1 rounded-lg border border-transparent px-2 py-1 text-left transition hover:border-ui-border-strong hover:bg-ui-surface disabled:cursor-not-allowed disabled:opacity-60";

export const ACP_DEBUG_PERMISSION_SUBMETA_CLASS =
  "acp-permission-submeta mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-ui-xs text-ui-text-muted";

export const ACP_DEBUG_PERMISSION_WARNING_CLASS =
  "acp-permission-options mono mt-2 rounded-lg border border-state-warning-border bg-state-warning-bg px-2 py-ctrl-y-sm text-ui-xs text-state-warning-text";

export const ACP_DEBUG_RAW_PRE_CLASS =
  "acp-content mt-2 overflow-auto rounded-md border border-ui-border bg-ui-surface p-2 text-ui-xs leading-5 text-ui-text-secondary";

export const ACP_CONVERSATION_TOP_HINT_CLASS =
  "acp-conversation-top-hint rounded-lg border border-ui-border bg-ui-surface-soft px-ctrl-x py-ctrl-y text-ui-xs text-ui-text-muted";

export const ACP_BUBBLE_THINKING_CLASS =
  "acp-bubble agent_thinking rounded-xl border border-[#ddd9cf] bg-[#f7f4ed] px-ctrl-x py-ctrl-y shadow-[0_1px_0_rgba(15,23,42,0.04)]";

export const ACP_BUBBLE_PLAN_CLASS =
  "acp-bubble agent_plan rounded-xl border border-[#ddd9cf] bg-[#f7f4ed] px-3 py-2 shadow-[0_1px_0_rgba(15,23,42,0.04)]";

export const ACP_PLAN_INDEX_BADGE_CLASS =
  "acp-plan-index inline-flex h-5 w-5 items-center justify-center rounded-full border border-ui-border-strong text-ui-xs font-semibold text-ui-text-muted";

export const ACP_PLAN_PRIORITY_BADGE_CLASS =
  "acp-plan-priority rounded-full border border-state-warning-border bg-state-warning-bg px-2 py-0.5 text-[11px] font-medium text-state-warning-text";

export const ACP_PLAN_STATUS_BADGE_CLASS =
  "acp-plan-status rounded-full border border-ui-border-strong bg-ui-surface px-2 py-0.5 text-[11px] font-medium text-ui-text-muted";

export const ACP_SEGMENTED_NOTE_WARNING_CLASS =
  "acp-segmented-note rounded-md border border-state-warning-border bg-state-warning-bg px-2 py-ctrl-y-sm text-ui-xs text-state-warning-text";

export const ACP_SEGMENTED_BUTTON_CLASS =
  "acp-segmented-button inline-flex items-center justify-center rounded-md border border-ui-border-strong bg-ui-surface px-2.5 py-1 text-ui-xs font-medium text-ui-text-secondary transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft";

export const ACP_TERMINAL_PRE_CLASS =
  "acp-content rounded-md border border-ui-border bg-brand-primary p-2 text-ui-xs text-ui-text-inverse";

export const ACP_DIFF_PRE_CLASS =
  "acp-content acp-diff-view m-0 overflow-auto whitespace-pre rounded-md border border-slate-700 bg-slate-950 p-2 text-ui-xs text-slate-100 leading-[1.45]";

export const ACP_TOOL_STATUS_CLASS =
  "acp-tool-status ml-auto mt-px inline-flex shrink-0 items-center self-start whitespace-nowrap rounded-full border px-[5px] py-0.5 text-[10px] leading-tight sm:px-1.5 sm:text-[11px]";

export const ACP_TOOL_STATUS_SINGLE_DEFAULT_CLASS =
  "border-[rgba(138,90,19,0.26)] bg-[#ffe7c2] text-[#8a5a13]";

export const ACP_TOOL_STATUS_GROUP_DEFAULT_CLASS =
  "border-[rgba(138,90,19,0.26)] bg-[#ffe3bf] text-[#8a5a13]";

export const ACP_TOOL_STATUS_GROUP_SUCCESS_CLASS =
  "border-[rgba(31,122,61,0.32)] bg-[rgba(31,122,61,0.14)] text-[#1f7a3d]";

export const ACP_TOOL_STATUS_GROUP_FAILURE_CLASS =
  "border-[rgba(180,35,24,0.32)] bg-[rgba(180,35,24,0.14)] text-[#b42318]";

export const ACP_TOOL_STATUS_GROUP_RUNNING_CLASS =
  "border-[rgba(37,99,235,0.32)] bg-[rgba(37,99,235,0.14)] text-[#1d4ed8]";

export const TEAM_MUTED_TEXT_CLASS = "muted text-ui-sm text-ui-text-muted";

export const TEAM_SIDEBAR_ROOT_CLASS =
  "teams-sidebar flex min-h-0 min-w-0 flex-col gap-3 overflow-x-hidden overflow-y-auto rounded-2xl border border-ui-border bg-ui-surface p-4 shadow-sm";

export const TEAM_SIDEBAR_SWITCHER_BUTTON_CLASS =
  "flex w-full items-center justify-between gap-2 rounded-md border border-transparent bg-transparent px-1.5 py-1.5 text-left transition hover:bg-ui-surface-soft/70";

export const TEAM_SIDEBAR_SWITCHER_PANEL_CLASS =
  "mt-1 space-y-2 border-l border-ui-border/70 pl-3";

export const TEAM_SIDEBAR_SWITCHER_ACTIONS_CLASS = "mt-3 flex flex-wrap gap-2";

export const TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS =
  "inline-flex h-8 w-8 items-center justify-center rounded-md border border-ui-border bg-ui-surface text-ui-text-secondary transition hover:border-ui-border-emphasis hover:text-ui-text-primary";

export const TEAM_SIDEBAR_SCOPE_SWITCH_CLASS =
  "flex items-center gap-1.5 border-b border-ui-border/70 pb-2";

const TEAM_SIDEBAR_SCOPE_BUTTON_BASE_CLASS =
  "inline-flex min-w-0 flex-1 items-center justify-center rounded-full border border-transparent px-2.5 py-1.5 text-[10px] font-semibold uppercase tracking-[0.18em] transition";

export const TEAM_SIDEBAR_SCOPE_BUTTON_ACTIVE_CLASS =
  `${TEAM_SIDEBAR_SCOPE_BUTTON_BASE_CLASS} border-ui-border/70 bg-ui-surface-soft text-ui-text-primary shadow-sm`;

export const TEAM_SIDEBAR_SCOPE_BUTTON_IDLE_CLASS =
  `${TEAM_SIDEBAR_SCOPE_BUTTON_BASE_CLASS} text-ui-text-muted hover:border-ui-border/60 hover:bg-ui-surface-soft/60 hover:text-ui-text-primary`;

export const TEAM_SIDEBAR_SECTION_CLASS = "flex min-w-0 flex-col gap-2";

export const TEAM_SIDEBAR_SECTION_TOGGLE_CLASS =
  "appearance-none border-0 bg-transparent shadow-none flex w-full items-center justify-between gap-2 px-1 py-1 text-left text-[10px] font-semibold uppercase tracking-[0.22em] text-ui-text-muted transition hover:text-ui-text-primary";

export const TEAM_SIDEBAR_NAV_LIST_CLASS =
  "flex min-w-0 flex-col gap-1 border-l border-ui-border/70 pl-2";

const TEAM_SIDEBAR_NAV_ITEM_BASE_CLASS =
  "flex w-full min-w-0 flex-col items-start gap-1 rounded-lg border border-transparent px-2.5 py-2 text-left transition";

export const TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS =
  `${TEAM_SIDEBAR_NAV_ITEM_BASE_CLASS} bg-ui-surface-soft text-ui-text-primary`;

export const TEAM_SIDEBAR_NAV_ITEM_IDLE_CLASS =
  `${TEAM_SIDEBAR_NAV_ITEM_BASE_CLASS} bg-transparent hover:bg-ui-surface-soft/80`;

export const TEAM_SIDEBAR_NAV_ITEM_META_CLASS =
  "mono w-full min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-ui-xs text-ui-text-muted";

export const TEAM_SIDEBAR_FORGE_CARD_CLASS =
  "teams-form teams-create-launch flex flex-col gap-2 rounded-xl border border-ui-border bg-ui-surface-soft/70 p-4";

export const TEAM_SIDEBAR_INFO_CARD_CLASS =
  "rounded-lg border border-ui-border bg-ui-surface px-ctrl-x py-ctrl-y";

export const TEAM_SIDEBAR_INFO_LABEL_CLASS =
  "text-[11px] font-semibold uppercase tracking-wide text-ui-text-muted";

export const TEAM_SIDEBAR_INFO_TEXT_CLASS = "mt-1 text-ui-sm text-ui-text-secondary";

export const TEAM_SIDEBAR_META_GRID_CLASS =
  "teams-create-launch-meta mono mt-3 grid gap-1 text-ui-xs text-ui-text-muted";

export const AGENTS_PANEL_EXPANDED_CLASS =
  "workspace-left rounded-[28px] border-[3px] border-black bg-[#f3efe4] p-3 text-[#1f252c] shadow-[0_3px_0_rgba(0,0,0,0.12)] backdrop-blur";

export const AGENTS_PANEL_COLLAPSED_CLASS =
  "workspace-left collapsed rounded-[24px] border-[3px] border-black bg-[#f3efe4] p-3 text-[#1f252c] shadow-[0_3px_0_rgba(0,0,0,0.12)] backdrop-blur";

export const AGENTS_TOOLBAR_CLASS = "mb-3 flex items-center justify-between gap-2";
export const AGENTS_TOOLBAR_ACTIONS_CLASS = "flex items-center gap-2";

export const AGENTS_CREATE_BUTTON_CLASS =
  "rounded-[14px] border-[3px] border-black bg-[#203b2d] px-4 py-2 text-sm font-semibold text-white shadow-[0_2px_0_rgba(0,0,0,0.16)] transition hover:-translate-y-[1px]";

export const AGENTS_ROW_CLASS =
  "agents-workbench-row rounded-[18px] border-[2px] border-black/15 bg-[#fcfbf7] px-ctrl-x py-3 text-[#1f252c] shadow-[0_1px_0_rgba(0,0,0,0.08)] transition hover:-translate-y-[1px] hover:bg-[#f6f1e7] hover:shadow-[0_2px_0_rgba(0,0,0,0.12)]";

export const AGENTS_ROW_ACTIVE_CLASS =
  "agents-workbench-row active rounded-[18px] border-[2px] border-[#203b2d]/35 bg-[#e7efe8] px-ctrl-x py-3 text-[#13261d] shadow-[0_2px_0_rgba(0,0,0,0.12)]";

export const OUTPUT_HEADER_ROOT_CLASS =
  "output-header grid grid-cols-[minmax(0,1fr)_auto] grid-rows-[auto_auto] items-center justify-between gap-x-3 gap-y-2 rounded-[18px] border-[2px] border-black bg-[#f3f1eb] px-3 py-2 shadow-[0_1px_0_rgba(0,0,0,0.12)] sm:rounded-[24px] sm:border-[3px] sm:px-4 sm:py-3 sm:shadow-[0_3px_0_rgba(0,0,0,0.16)] max-[720px]:grid-cols-[minmax(0,1fr)_auto] max-[720px]:grid-rows-[auto] max-[720px]:gap-x-2 max-[720px]:gap-y-1 max-[720px]:rounded-b-none max-[720px]:border-b-0 max-[720px]:px-2.5 max-[720px]:py-1.5 max-[720px]:shadow-none";

export const OUTPUT_HEADER_TITLE_CLASS =
  "output-title col-start-1 row-start-1 inline-flex min-w-0 w-fit max-w-[clamp(160px,32vw,360px)] items-center gap-2 max-[720px]:w-full max-[720px]:max-w-full";

export const OUTPUT_HEADER_TITLE_TEXT_CLASS =
  "output-title-text flex min-w-0 flex-col gap-0.5";

export const OUTPUT_HEADER_TITLE_MAIN_CLASS =
  "output-title-main flex min-w-0 flex-wrap items-center gap-2 max-[720px]:gap-1.5";

export const OUTPUT_HEADER_TITLE_HEADING_CLASS =
  "m-0 overflow-hidden text-ellipsis whitespace-nowrap text-[15px] font-semibold leading-[1.1] text-black sm:text-[18px]";

export const OUTPUT_HEADER_META_CLASS =
  "output-meta col-start-2 row-start-1 flex max-w-[52vw] flex-nowrap items-center justify-self-end gap-2 overflow-hidden whitespace-nowrap text-ui-xs text-black/60 max-[720px]:col-start-2 max-[720px]:row-start-1 max-[720px]:max-w-[40vw] max-[720px]:justify-self-end [&>*]:min-w-0 [&>*]:overflow-hidden [&>*]:text-ellipsis [&>*]:whitespace-nowrap max-[720px]:[&>*:nth-child(n+2)]:hidden";

export const OUTPUT_HEADER_PILL_CLASS =
  "output-pill rounded-full border-[2px] border-black bg-[#fcfbf7] px-3 py-1 text-ui-xs font-medium text-black shadow-[0_1px_0_rgba(0,0,0,0.12)]";

export const OUTPUT_HEADER_SESSION_CLASS = "output-session mono text-[11px] text-black/58";

export const OUTPUT_HEADER_UPDATED_CLASS = "output-updated text-[11px] text-black/58";

export const OUTPUT_HEADER_SUBTITLE_ROW_CLASS =
  "output-subtitle-row col-start-1 row-start-2 mt-1 flex min-w-0 items-center justify-self-start max-w-full max-[720px]:hidden";

export const OUTPUT_HEADER_SUBTITLE_CLASS =
  "output-subtitle overflow-hidden text-ellipsis whitespace-nowrap text-[13px] text-black/58";

export const OUTPUT_BODY_ROOT_CLASS =
  "output-body rounded-2xl border border-ui-border/80 bg-ui-surface/85 shadow-sm backdrop-blur";

export const OUTPUT_BODY_ACP_ROOT_CLASS =
  "output-body output-body-acp min-h-0 flex-1 bg-transparent max-[720px]:-mt-[1px]";

export const OUTPUT_BODY_LOADING_CLASS =
  "output-loading flex h-full min-h-40 flex-col items-center justify-center gap-2 text-ui-text-muted";

export const OUTPUT_BODY_EMPTY_CLASS =
  "output-empty flex h-full min-h-40 flex-col items-center justify-center gap-1 rounded-xl border border-dashed border-ui-border-strong bg-ui-surface-soft/60 p-4 text-center";

export const INPUT_DOCK_ROOT_CLASS =
  "input docked rounded-2xl border border-ui-border/80 bg-ui-surface/85 p-2.5 shadow-sm backdrop-blur max-[720px]:rounded-[16px] max-[720px]:p-2 max-[720px]:shadow-none";

export const INPUT_DOCK_INTERRUPT_BUTTON_CLASS =
  "acp-interrupt-button input-interrupt-button rounded-lg border border-state-warning-border bg-state-warning-bg px-ctrl-x py-ctrl-y text-ui-sm font-medium text-state-warning-text hover:border-ui-border-emphasis max-[720px]:px-2 max-[720px]:py-1 max-[720px]:text-[11px]";

export const INPUT_DOCK_HISTORY_BUTTON_CLASS =
  "history-toggle rounded-lg border border-ui-border-strong bg-ui-surface px-ctrl-x py-ctrl-y text-ui-sm font-medium text-ui-text-secondary hover:border-ui-border-emphasis max-[720px]:px-2 max-[720px]:py-1 max-[720px]:text-[11px]";

export const INPUT_DOCK_HISTORY_MENU_CLASS =
  "input-history-menu rounded-lg border border-ui-border-strong bg-ui-surface p-1 shadow max-[720px]:rounded-[10px] max-[720px]:p-0.5";

export const INPUT_DOCK_HISTORY_ITEM_CLASS =
  "input-history-item block w-full rounded-md px-2 py-1 text-left text-ui-sm hover:bg-ui-surface-muted";

export const INPUT_DOCK_TEXTAREA_CLASS =
  "min-h-[3.5rem] w-full rounded-lg border border-ui-border-strong bg-ui-surface px-ctrl-x py-ctrl-y text-ui-sm text-ui-text-primary outline-none transition focus:border-ui-border-emphasis focus:ring-2 focus:ring-ui-border max-[720px]:min-h-[3.1rem] max-[720px]:rounded-[12px] max-[720px]:px-2.5 max-[720px]:py-2";

export const AUTH_PAGE_CLASS =
  "app min-h-[var(--agenthub-vh,100vh)] px-4 py-8 md:px-6 md:py-10";

export const AUTH_CARD_BASE_CLASS =
  "auth mx-auto w-full max-w-md rounded-2xl border border-slate-200/80 bg-white/90 p-6 shadow-sm backdrop-blur";

export const AUTH_FORM_CARD_CLASS = `${AUTH_CARD_BASE_CLASS} flex flex-col gap-3`;

export const AUTH_INPUT_CLASS =
  "w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 outline-none transition focus:border-slate-500 focus:ring-2 focus:ring-slate-200";

export const AUTH_ACTIONS_CLASS = "auth-actions mt-1 flex flex-wrap gap-2";

export const AUTH_PRIMARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg bg-slate-900 px-3 py-2 text-sm font-medium text-white transition hover:bg-slate-800";

export const AUTH_SECONDARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-900 transition hover:border-slate-500";

export const TEAM_PANEL_CARD_CLASS =
  "card team-panel-card min-h-0 rounded-2xl border border-slate-200 bg-white shadow-sm";

export const TEAM_PANEL_TOOLBAR_CLASS =
  "mb-3 flex flex-wrap items-center justify-between gap-2";

export const TEAM_PANEL_TOOLBAR_ACTIONS_CLASS =
  "actions flex w-full flex-wrap items-center gap-2 sm:w-auto sm:justify-end";

export const TEAM_PANEL_PRIMARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg bg-slate-900 px-3 py-2 text-sm font-medium text-white shadow-sm transition hover:bg-slate-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 disabled:cursor-not-allowed disabled:opacity-60";

export const TEAM_PANEL_SECONDARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-800 shadow-sm transition hover:border-slate-500 hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-200 disabled:cursor-not-allowed disabled:opacity-60";

export const TEAM_PANEL_GHOST_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-800 shadow-sm transition hover:border-slate-500 hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-200 disabled:cursor-not-allowed disabled:opacity-60";

export const TEAM_PANEL_REFRESH_BUTTON_CLASS =
  "inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-2.5 py-1.5 text-sm font-medium text-slate-800 shadow-sm transition hover:border-slate-500 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-200";

export const TEAM_PANEL_INPUT_CLASS =
  "w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 outline-none transition focus:border-slate-500 focus:ring-2 focus:ring-slate-200";

export const TEAM_PANEL_TEXTAREA_CLASS =
  `mono min-h-24 ${TEAM_PANEL_INPUT_CLASS}`;

export const TEAM_PANEL_TITLE_CLASS = "text-lg font-semibold tracking-tight text-slate-900";

export const TEAM_PANEL_PRE_CLASS =
  "mono max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border border-slate-200 bg-slate-50 p-2";

export const TEAM_TAB_BAR_CLASS =
  "tab-bar team-tab-bar flex min-w-0 gap-2 overflow-x-auto rounded-xl border border-slate-200 bg-white p-2 shadow-sm";

const TEAM_TAB_BUTTON_BASE_CLASS =
  "tab shrink-0 rounded-lg px-3 py-2 text-sm font-medium transition";

export const TEAM_TAB_BUTTON_ACTIVE_CLASS =
  `${TEAM_TAB_BUTTON_BASE_CLASS} active bg-slate-900 text-white shadow-sm`;

export const TEAM_TAB_BUTTON_IDLE_CLASS =
  `${TEAM_TAB_BUTTON_BASE_CLASS} text-slate-600 hover:bg-slate-100 hover:text-slate-900`;

export const TEAM_LIST_ITEM_BASE_CLASS =
  "team-item flex w-full min-w-0 flex-col items-start gap-1 rounded-lg border border-slate-200 bg-white px-3 py-2 text-left text-slate-900 transition";

export const TEAM_LIST_ITEM_ACTIVE_CLASS =
  `${TEAM_LIST_ITEM_BASE_CLASS} border-slate-300 bg-slate-50 ring-1 ring-slate-200`;

export const TEAM_LIST_ITEM_IDLE_CLASS =
  `${TEAM_LIST_ITEM_BASE_CLASS} hover:border-slate-300`;

export const TEAM_LIST_ITEM_TITLE_CLASS =
  "team-name w-full min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-sm font-semibold";

export const TEAM_LIST_ITEM_META_CLASS =
  "team-id mono w-full min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-xs opacity-90";

export const TEAM_CREATE_MODAL_BACKDROP_CLASS =
  "modal-backdrop team-create-modal-backdrop fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-slate-950/40 px-3 py-6 sm:py-10";

export const TEAM_CREATE_MODAL_CARD_CLASS =
  "modal team-create-modal w-full max-w-5xl rounded-2xl border border-slate-200 bg-white p-4 shadow-2xl sm:p-5";

export const TEAM_CREATE_STAGE_BADGE_CLASS =
  "badge rounded-full border border-slate-300 bg-slate-100 px-2.5 py-1 text-xs font-medium text-slate-700";

export const TEAM_CREATE_PANEL_CARD_CLASS =
  "team-create-panel rounded-xl border border-slate-200 bg-slate-50/70 p-4";

export const TEAM_CREATE_NOTE_INFO_CLASS =
  "team-create-stage-note mt-2 rounded-lg border border-sky-200 bg-sky-50 px-3 py-2 text-sm text-sky-700";

export const TEAM_CREATE_NOTE_WARNING_CLASS =
  "team-create-stage-note mt-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-700";

export const TEAM_CREATE_STEP_PREVIEW_CLASS =
  "teams-step-body mono mt-3 rounded-lg border border-slate-200 bg-white px-3 py-2 text-xs text-slate-600";

export const TEAM_CREATE_STEP_PREVIEW_MUTED_CLASS =
  "teams-step-body mono mt-3 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-xs text-slate-600";

export const TEAM_CREATE_SKILL_TAG_SELECTED_CLASS =
  "team-skill-tag selected rounded-full border border-slate-900 bg-slate-900 px-3 py-1 text-xs font-medium text-white transition";

export const TEAM_CREATE_SKILL_TAG_IDLE_CLASS =
  "team-skill-tag rounded-full border border-slate-300 bg-white px-3 py-1 text-xs font-medium text-slate-700 transition hover:border-slate-400 hover:bg-slate-50";

export const TEAM_CREATE_WORKER_CARD_CLASS =
  "teams-worker-card rounded-xl border border-slate-200 bg-white p-3 shadow-sm";

export const TEAM_CREATE_ACTIONS_BAR_CLASS =
  "modal-actions team-create-actions mt-4 flex flex-wrap items-center justify-end gap-2 border-t border-slate-200 pt-3";

export const ACP_PANEL_ROOT_CLASS =
  "acp relative flex min-h-0 flex-1 flex-col rounded-2xl border border-slate-100 bg-white/90 shadow-sm";

export const ACP_PANEL_HEAD_CLASS =
  "acp-head minimal flex flex-wrap items-start justify-between gap-3 border-b border-slate-200 px-3 py-2 sm:px-4";

export const ACP_PANEL_TABS_CLASS =
  "acp-tabs flex items-center gap-2 rounded-lg border border-slate-200 bg-slate-50 p-1";

export const ACP_TAB_BUTTON_BASE_CLASS =
  "inline-flex min-h-[30px] items-center rounded-md px-3 py-1.5 text-xs font-medium leading-tight transition sm:text-sm";

export const ACP_TAB_BUTTON_ACTIVE_CLASS =
  `${ACP_TAB_BUTTON_BASE_CLASS} bg-slate-900 text-white shadow-sm`;

export const ACP_TAB_BUTTON_IDLE_CLASS =
  `${ACP_TAB_BUTTON_BASE_CLASS} text-slate-600 hover:bg-white hover:text-slate-900`;

export const ACP_TAB_BADGE_CLASS =
  "acp-tab-badge rounded-full border border-current/30 px-1.5 py-0.5 text-[10px] font-semibold leading-none sm:text-xs";

export const ACP_JUMP_BOTTOM_BUTTON_CLASS =
  "acp-jump-bottom absolute bottom-3 right-3 z-20 inline-flex h-8 w-8 items-center justify-center rounded-full border border-slate-300 bg-white text-slate-800 shadow-md transition hover:border-slate-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300";

export const ACP_DEBUG_ROOT_CLASS =
  "acp-debug flex min-h-0 flex-1 flex-col gap-3 p-3 sm:p-4";

export const ACP_DEBUG_TABS_CLASS =
  "acp-debug-tabs flex flex-wrap gap-2 rounded-lg border border-slate-200 bg-slate-50 p-1";

export const ACP_DEBUG_SECTION_CLASS =
  "space-y-3 rounded-xl border border-slate-200 bg-white p-3 shadow-sm sm:p-4";

export const ACP_DEBUG_EMPTY_CLASS =
  "empty rounded-lg border border-dashed border-slate-300 bg-slate-50 px-3 py-5 text-sm text-slate-500";

export const ACP_DEBUG_PERMISSION_TOGGLE_CLASS =
  "acp-permission-toggle flex min-w-0 flex-1 flex-col items-start gap-1 rounded-lg border border-transparent px-2 py-1 text-left transition hover:border-slate-300 hover:bg-white disabled:cursor-not-allowed disabled:opacity-60";

export const ACP_DEBUG_PERMISSION_SUBMETA_CLASS =
  "acp-permission-submeta mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-slate-600";

export const ACP_DEBUG_PERMISSION_WARNING_CLASS =
  "acp-permission-options mono mt-2 rounded-lg border border-amber-200 bg-amber-50 px-2 py-1.5 text-xs text-amber-700";

export const ACP_DEBUG_RAW_PRE_CLASS =
  "acp-content mt-2 overflow-auto rounded-md border border-slate-200 bg-white p-2 text-xs leading-5 text-slate-700";

export const ACP_CONVERSATION_TOP_HINT_CLASS =
  "acp-conversation-top-hint rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-xs text-slate-600";

export const ACP_BUBBLE_THINKING_CLASS =
  "acp-bubble agent_thinking rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 shadow-sm";

export const ACP_BUBBLE_PLAN_CLASS =
  "acp-bubble agent_plan rounded-xl border border-violet-200 bg-violet-50/40 px-3 py-2 shadow-sm";

export const ACP_PLAN_INDEX_BADGE_CLASS =
  "acp-plan-index inline-flex h-5 w-5 items-center justify-center rounded-full border border-slate-300 text-xs font-semibold text-slate-600";

export const ACP_PLAN_PRIORITY_BADGE_CLASS =
  "acp-plan-priority rounded-full border border-amber-300 bg-amber-50 px-2 py-0.5 text-[11px] font-medium text-amber-700";

export const ACP_PLAN_STATUS_BADGE_CLASS =
  "acp-plan-status rounded-full border border-slate-300 bg-white px-2 py-0.5 text-[11px] font-medium text-slate-600";

export const ACP_SEGMENTED_NOTE_WARNING_CLASS =
  "acp-segmented-note rounded-md border border-amber-200 bg-amber-50 px-2 py-1.5 text-xs text-amber-700";

export const ACP_SEGMENTED_BUTTON_CLASS =
  "acp-segmented-button inline-flex items-center justify-center rounded-md border border-slate-300 bg-white px-2.5 py-1 text-xs font-medium text-slate-700 transition hover:border-slate-400 hover:bg-slate-50";

export const ACP_TERMINAL_PRE_CLASS =
  "acp-content rounded-md border border-slate-200 bg-slate-950 p-2 text-xs text-slate-100";

export const ACP_DIFF_PRE_CLASS =
  "acp-content acp-diff-view overflow-auto rounded-md border border-slate-200 bg-slate-950 p-2 text-xs text-slate-100";

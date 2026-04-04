export const NOTION_TEXT_COLOR = "text-notion-text";
export const NOTION_TEXT_MUTED_COLOR = "text-notion-text-muted";
export const NOTION_BORDER_COLOR = "border-notion-border";
export const NOTION_HOVER_COLOR = "hover:bg-notion-hover";
export const NOTION_SIDEBAR_BG = "bg-notion-sidebar";

export const AUTH_FORM_CARD_CLASS =
  "mx-auto mt-20 flex w-full max-w-md flex-col gap-6 rounded-2xl border border-notion-border bg-white p-8 shadow-xl sm:mt-32";

export const AUTH_INPUT_CLASS =
  "w-full rounded-lg border border-notion-border bg-white px-4 py-2.5 text-[15px] text-notion-text outline-none transition focus:border-notion-accent focus:ring-4 focus:ring-notion-accent/10";

export const AUTH_PRIMARY_BUTTON_CLASS =
  "inline-flex h-11 items-center justify-center rounded-lg bg-notion-accent px-6 text-[15px] font-bold text-white shadow-sm transition hover:bg-notion-accent/90 disabled:opacity-50 active:translate-y-px";

export const AUTH_SECONDARY_BUTTON_CLASS =
  "inline-flex h-11 items-center justify-center rounded-lg border border-notion-border bg-white px-6 text-[15px] font-bold text-notion-text shadow-sm transition hover:bg-notion-hover active:translate-y-px";

export const AUTH_ACTIONS_CLASS = "flex flex-col gap-3 pt-2";

export const AUTH_PAGE_CLASS =
  "app min-h-[var(--agenthub-vh,100vh)] px-4 py-8 md:px-6 md:py-10";

export const AUTH_CARD_BASE_CLASS =
  "auth mx-auto w-full max-w-md rounded-2xl border border-notion-border bg-white p-6 shadow-sm";

export const ACP_PANEL_ROOT =
  "acp acp-panel flex min-h-0 flex-1 flex-col bg-[#f7f7f5] overflow-hidden";

export const ACP_PANEL_ROOT_CLASS = ACP_PANEL_ROOT;

export const ACP_PANEL_HEAD_CLASS =
  "acp-head minimal sticky top-0 z-20 flex flex-wrap items-center justify-between gap-2 border-b border-black/[0.05] bg-white/88 px-4 py-2.5 backdrop-blur-sm sm:px-6 max-[720px]:px-3";

export const ACP_TABS_CLASS =
  "inline-flex max-w-full flex-wrap items-center gap-1 rounded-[10px] border border-black/[0.06] bg-white/92 p-1 shadow-[0_1px_3px_rgba(15,23,42,0.03)]";

export const ACP_PANEL_TABS_CLASS = ACP_TABS_CLASS;

export const ACP_TAB_BUTTON_BASE_CLASS =
  "acp-tab-button inline-flex h-7 items-center rounded-[8px] px-2.5 text-[11px] font-semibold leading-none transition";

export const ACP_TAB_BUTTON_ACTIVE_CLASS =
  "bg-black/[0.06] text-notion-text shadow-[0_1px_1px_rgba(15,23,42,0.04)]";

export const ACP_TAB_BUTTON_IDLE_CLASS =
  "text-notion-text-muted hover:bg-black/[0.035] hover:text-notion-text";

export const ACP_TAB_BADGE_CLASS =
  "acp-tab-badge ml-1 rounded-full bg-black/[0.06] px-1.5 py-0.5 text-[10px] font-semibold text-notion-text-muted";

export const ACP_JUMP_BOTTOM_BUTTON_CLASS =
  "acp-jump-bottom absolute bottom-24 right-6 z-40 inline-flex h-9 w-9 items-center justify-center rounded-full border border-notion-border bg-white shadow-md text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text active:translate-y-px";

export const ACP_DEBUG_ROOT_CLASS =
  "acp-debug flex min-h-0 flex-1 flex-col gap-6 p-4 sm:mx-auto sm:max-w-4xl sm:p-8";

export const ACP_DEBUG_TABS_CLASS =
  "acp-debug-tabs flex flex-wrap gap-1 rounded-lg border border-notion-border bg-notion-sidebar p-1";

export const ACP_DEBUG_SECTION_CLASS =
  "space-y-4 rounded-xl border border-notion-border bg-white p-4 sm:p-6";

export const ACP_DEBUG_EMPTY_CLASS =
  "empty rounded-lg border border-dashed border-notion-border bg-notion-sidebar px-4 py-8 text-center text-sm text-notion-text-muted";

export const ACP_DEBUG_PERMISSION_TOGGLE_CLASS =
  "acp-permission-toggle flex min-w-0 flex-1 flex-col items-start gap-1 rounded-lg border border-transparent px-3 py-2 text-left transition hover:bg-notion-hover disabled:cursor-not-allowed disabled:opacity-60";

export const ACP_DEBUG_PERMISSION_SUBMETA_CLASS =
  "acp-permission-submeta mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-notion-text-muted";

export const ACP_DEBUG_PERMISSION_WARNING_CLASS =
  "acp-permission-options mono mt-2 rounded-lg border border-orange-100 bg-orange-50/50 px-3 py-2 text-[11px] text-orange-800";

export const ACP_DEBUG_RAW_PRE_CLASS =
  "acp-content mt-2 overflow-auto rounded-lg border border-notion-border bg-white p-3 text-[12px] leading-relaxed text-notion-text";

export const ACP_CONVERSATION_TOP_HINT_CLASS =
  "acp-conversation-top-hint mx-auto my-6 rounded-full border border-notion-border bg-white px-4 py-1 text-[11px] font-medium text-notion-text-muted shadow-sm";

export const ACP_MESSAGE_BUBBLE_CLASS =
  "acp-message-bubble relative max-w-[min(86%,78ch)] rounded-[18px] border px-4 py-3 text-[14px] leading-7 shadow-[0_1px_3px_rgba(15,23,42,0.05)] transition-all";

export const ACP_MESSAGE_BUBBLE_AGENT_CLASS =
  "acp-message-bubble-agent self-start rounded-tl-md border-black/[0.06] bg-white text-notion-text";

export const ACP_MESSAGE_BUBBLE_USER_CLASS =
  "acp-message-bubble-user self-end rounded-tr-md border-notion-accent/15 bg-notion-accent-bg/75 text-notion-text";

export const ACP_BUBBLE_THINKING_CLASS =
  "acp-bubble agent_thinking self-start max-w-[80%] rounded-[18px] rounded-tl-[4px] border border-notion-border bg-[#f0f1f0]/50 px-4 py-3 italic text-notion-text-muted/80 shadow-sm";

export const ACP_BUBBLE_PLAN_CLASS =
  "acp-bubble agent_plan self-start max-w-[90%] rounded-[18px] rounded-tl-[4px] border border-notion-border bg-white px-4 py-3 shadow-sm";

export const ACP_PLAN_INDEX_BADGE_CLASS =
  "mr-2 inline-flex h-5 w-5 items-center justify-center rounded-md bg-notion-hover text-[10px] font-bold text-notion-text-muted shadow-inner";

export const ACP_PLAN_PRIORITY_BADGE_CLASS =
  "ml-2 rounded-sm bg-orange-50 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider text-orange-600 border border-orange-100";

export const ACP_PLAN_STATUS_BADGE_CLASS =
  "ml-2 rounded-sm bg-emerald-50 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider text-emerald-700 border border-emerald-100";

export const ACP_TOOL_STATUS_CLASS =
  "acp-tool-status inline-flex shrink-0 items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider shadow-sm transition";

export const ACP_TOOL_STATUS_GROUP_DEFAULT_CLASS =
  "bg-notion-sidebar border border-notion-border text-notion-text-muted";

export const ACP_TOOL_STATUS_GROUP_RUNNING_CLASS =
  "bg-notion-accent-bg border border-notion-accent/20 text-notion-accent animate-pulse";

export const ACP_TOOL_STATUS_GROUP_SUCCESS_CLASS =
  "bg-emerald-50 border border-emerald-200 text-emerald-700";

export const ACP_TOOL_STATUS_GROUP_FAILURE_CLASS =
  "bg-rose-50 border border-rose-200 text-rose-700";

export const ACP_TOOL_STATUS_SINGLE_DEFAULT_CLASS =
  "bg-notion-hover border border-notion-border text-notion-text-muted font-mono normal-case tracking-normal";

export const ACP_TERMINAL_PRE_CLASS =
  "acp-terminal-pre m-0 max-w-full overflow-x-auto whitespace-pre rounded-lg border border-notion-border bg-[#1e1e1e] p-4 text-[13px] leading-relaxed text-slate-200 shadow-inner";

export const ACP_DIFF_PRE_CLASS =
  "acp-diff-pre acp-diff-view m-0 max-w-full overflow-x-auto whitespace-pre rounded-lg border border-notion-border bg-[#1e1e1e] p-4 text-[13px] leading-relaxed text-slate-200 shadow-inner font-mono";

export const ACP_SEGMENTED_BUTTON_CLASS =
  "acp-segmented-button h-7 rounded-md border border-notion-border bg-white px-2.5 text-[11px] font-bold uppercase tracking-wider text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text active:translate-y-px shadow-sm";

export const ACP_SEGMENTED_NOTE_WARNING_CLASS =
  "acp-segmented-note warning flex items-center gap-2 rounded-md border border-orange-100 bg-orange-50/50 px-3 py-2 text-[12px] leading-relaxed text-orange-800 italic";

export const ACP_PAYLOAD_MARKDOWN_CLASS =
  "acp-payload-markdown text-[14px] leading-relaxed text-notion-text [&_pre]:my-3 [&_code]:rounded [&_code]:bg-notion-hover [&_code]:px-1 [&_p]:mb-3 [&_p:last-child]:mb-0";

export const TEAM_PANEL_CARD_CLASS =
  "teams-panel-card flex min-h-0 flex-1 flex-col overflow-hidden";

export const TEAM_PANEL_TITLE_CLASS = "text-[22px] font-bold tracking-tight text-notion-text";

export const TEAM_PANEL_TOOLBAR_CLASS =
  "flex flex-wrap items-center justify-between gap-4 border-b border-notion-border bg-white/80 pb-4 backdrop-blur-md sticky top-0 z-20";

export const TEAM_PANEL_TOOLBAR_ACTIONS_CLASS = "flex items-center gap-2";

export const TEAM_PANEL_REFRESH_BUTTON_CLASS =
  "inline-flex h-9 items-center gap-2 rounded-lg border border-notion-border bg-white px-4 text-[13px] font-bold text-notion-text shadow-sm transition hover:bg-notion-hover active:translate-y-px";

export const TEAM_PANEL_PRIMARY_BUTTON_CLASS =
  "inline-flex h-9 items-center justify-center rounded-lg bg-notion-accent px-4 text-[13px] font-bold text-white shadow-sm transition hover:bg-notion-accent/90 disabled:opacity-50 active:translate-y-px";

export const TEAM_PANEL_SECONDARY_BUTTON_CLASS =
  "inline-flex h-9 items-center justify-center rounded-lg border border-notion-border bg-white px-4 text-[13px] font-bold text-notion-text shadow-sm transition hover:bg-notion-hover active:translate-y-px";

export const TEAM_PANEL_INPUT_CLASS =
  "w-full rounded-lg border border-notion-border bg-white px-3 py-2 text-[14px] text-notion-text outline-none transition focus:border-notion-accent focus:ring-2 focus:ring-notion-accent/10";

export const TEAM_PANEL_TEXTAREA_CLASS =
  "w-full rounded-lg border border-notion-border bg-white px-3 py-2 text-[14px] text-notion-text outline-none transition focus:border-notion-accent focus:ring-2 focus:ring-notion-accent/10 resize-none";

export const TEAM_PANEL_PRE_CLASS =
  "mono mt-2 max-w-full overflow-x-auto whitespace-pre rounded-lg border border-notion-border bg-notion-sidebar/50 p-4 text-[12px] leading-relaxed text-notion-text shadow-inner";

export const TEAM_CREATE_MODAL_BACKDROP_CLASS =
  "modal-backdrop team-create-modal-backdrop fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/20 px-3 py-6 sm:py-10 backdrop-blur-sm";

export const TEAM_CREATE_MODAL_CARD_CLASS =
  "modal team-create-modal w-full max-w-5xl rounded-xl border border-notion-border bg-white p-4 shadow-2xl sm:p-6";

export const TEAM_CREATE_PANEL_CARD_CLASS =
  "team-create-panel rounded-xl border border-notion-border bg-white/90 p-4";

export const TEAM_CREATE_SKILL_TAG_SELECTED_CLASS =
  "team-skill-tag selected rounded-full border border-notion-accent bg-notion-accent px-3 py-1 text-[12px] font-medium text-white transition";

export const TEAM_CREATE_ACTIONS_BAR_CLASS =
  "modal-actions team-create-actions mt-4 flex flex-wrap items-center justify-end gap-2 border-t border-notion-border pt-3";

export const TEAM_LIST_ITEM_BASE_CLASS =
  "team-item group relative flex w-full flex-col gap-1.5 rounded-xl border border-notion-border bg-white p-4 text-left shadow-sm transition-all hover:bg-notion-hover hover:border-notion-accent/20 active:translate-y-px";

export const TEAM_LIST_ITEM_ACTIVE_CLASS =
  `${TEAM_LIST_ITEM_BASE_CLASS} ring-1 ring-notion-accent/30 border-notion-accent/30 bg-notion-hover shadow-md`;

export const TEAM_LIST_ITEM_IDLE_CLASS = TEAM_LIST_ITEM_BASE_CLASS;

export const TEAM_LIST_ITEM_TITLE_CLASS = "text-[15px] font-bold tracking-tight text-notion-text";

export const TEAM_LIST_ITEM_META_CLASS =
  "mono text-[10px] font-bold uppercase tracking-widest text-notion-text-muted opacity-70";

export const TEAM_MUTED_TEXT_CLASS = "text-[14px] leading-relaxed text-notion-text-muted italic";

export const TEAM_TAB_BAR_CLASS =
  "tab-bar team-tab-bar flex min-w-0 gap-2 overflow-x-auto rounded-[16px] border border-black/[0.05] bg-white/84 p-1 shadow-[0_1px_3px_rgba(15,23,42,0.04)]";

const TEAM_TAB_BUTTON_BASE_CLASS =
  "tab shrink-0 rounded-[12px] px-3 py-1.5 text-[13px] font-semibold transition";

export const TEAM_TAB_BUTTON_ACTIVE_CLASS =
  `${TEAM_TAB_BUTTON_BASE_CLASS} active border border-black/[0.06] bg-white text-notion-text shadow-[0_1px_2px_rgba(15,23,42,0.04)]`;

export const TEAM_TAB_BUTTON_IDLE_CLASS =
  `${TEAM_TAB_BUTTON_BASE_CLASS} text-notion-text-muted hover:bg-black/[0.03] hover:text-notion-text`;

export const TEAM_SIDEBAR_ROOT_CLASS =
  "teams-sidebar flex min-h-0 min-w-0 flex-col gap-4 overflow-x-hidden overflow-y-auto bg-notion-sidebar p-4 border-r border-notion-border";

export const TEAM_SIDEBAR_META_TOGGLE_BUTTON_CLASS =
  "inline-flex h-8 w-8 items-center justify-center rounded-md border border-notion-border bg-white text-notion-text-muted shadow-sm transition hover:bg-notion-hover hover:text-notion-text";

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

export const TEAM_SIDEBAR_WORKFLOW_ACTIVE_CLASS =
  "flex w-full items-center gap-2 rounded-md bg-notion-hover px-3 py-1.5 text-left text-notion-text font-medium transition";

export const TEAM_SIDEBAR_WORKFLOW_IDLE_CLASS =
  "flex w-full items-center gap-2 rounded-md bg-transparent px-3 py-1.5 text-left text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text";

export const TEAM_SIDEBAR_WORK_CLASS =
  "truncate pl-5 text-[11px] leading-relaxed text-notion-text-muted opacity-80";

export const TEAM_SIDEBAR_BADGE_CLASS =
  "shrink-0 rounded-sm bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold text-notion-text-muted";

export const TEAM_SIDEBAR_INDICATOR_DOT_CLASS =
  "h-2 w-2 shrink-0 rounded-full mt-0.5";

export const TEAM_TASK_ACTIVITY_LIST_CLASS =
  "relative min-h-0 flex flex-1 flex-col gap-4 overflow-y-auto overscroll-y-contain pr-1 before:absolute before:left-[11px] before:top-2 before:h-[calc(100%-16px)] before:w-0.5 before:bg-notion-hover";

export const TEAM_TASK_COMPOSER_PANEL_CLASS =
  "flex shrink-0 flex-col gap-1.5 border-t border-black/[0.05] bg-[#fbfbfa]/94 px-3 py-2.5 shadow-[0_-1px_0_rgba(15,23,42,0.05)] backdrop-blur-md sm:px-4";

export const TEAM_TASK_ACTIVITY_SHELL_CLASS = "min-h-full bg-white py-3";

export const TEAM_TASK_ACTIVITY_STACK_CLASS = "flex w-full flex-col";

const TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS =
  "group relative flex w-full max-w-full gap-2.5 px-3 py-1 sm:px-4";

export const TEAM_TASK_ACTIVITY_ITEM_HUMAN_CLASS = TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS;

export const TEAM_TASK_ACTIVITY_ITEM_AGENT_CLASS = TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS;

export const TEAM_TASK_ACTIVITY_CONTENT_CLASS =
  "min-w-0 flex max-w-[min(100%,72ch)] flex-col";

export const TEAM_TASK_ACTIVITY_CONTENT_HUMAN_CLASS = "items-end";

export const TEAM_TASK_ACTIVITY_CONTENT_AGENT_CLASS = "items-start";

export const TEAM_TASK_ACTIVITY_BUBBLE_BASE_CLASS =
  "mt-1 min-w-0 max-w-full rounded-[18px] border px-3.5 py-2.25 shadow-[0_1px_3px_rgba(15,23,42,0.04)]";

export const TEAM_TASK_ACTIVITY_BUBBLE_HUMAN_CLASS =
  "border-notion-accent/15 bg-notion-accent-bg/72";

export const TEAM_TASK_ACTIVITY_BUBBLE_AGENT_CLASS =
  "border-black/[0.06] bg-white";

export const TEAM_TASK_ACTIVITY_AUTHOR_CLASS =
  "text-[13px] font-bold text-notion-text";

export const TEAM_TASK_ACTIVITY_ITEM_CLASS = "relative flex gap-4 pl-8 group";

export const TEAM_TASK_ACTIVITY_ICON_WRAPPER_CLASS =
  "absolute left-0 flex h-6 w-6 items-center justify-center rounded-full bg-white border-2 border-notion-border text-notion-text-muted shadow-sm transition-colors group-hover:border-notion-accent group-hover:text-notion-accent";

export const TEAM_TASK_ACTIVITY_HEAD_CLASS =
  "mb-1 flex flex-wrap items-center gap-2";

export const TEAM_TASK_ACTIVITY_TITLE_CLASS =
  "text-sm font-bold text-notion-text";

export const TEAM_TASK_ACTIVITY_TIME_CLASS =
  "text-[10px] font-bold uppercase tracking-wider text-notion-text-muted opacity-0 group-hover:opacity-100 transition-opacity";

export const TEAM_TASK_ACTIVITY_BODY_CLASS =
  "text-[15px] leading-relaxed text-notion-text";

export const TEAM_TASK_ACTIVITY_COMMAND_BODY_CLASS =
  "mono m-0 max-w-full overflow-x-auto whitespace-pre text-[12px] leading-relaxed text-notion-text";

export const TEAM_TASK_PERMISSION_CARD_CLASS =
  "mt-1 max-w-full rounded-[18px] border border-black/[0.06] bg-white/94 p-4 shadow-[0_1px_3px_rgba(15,23,42,0.04)]";

export const TEAM_TASK_PERMISSION_CARD_COMPACT_CLASS =
  "mt-1 max-w-full rounded-[18px] border border-black/[0.05] bg-white/90 px-3 py-2 shadow-[0_1px_3px_rgba(15,23,42,0.03)]";

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

export const TASKS_BOARD_LANES_CLASS =
  "grid min-w-full auto-cols-[minmax(280px,1fr)] grid-flow-col gap-4";

export const TASKS_BOARD_COLUMN_CLASS =
  "flex min-h-[420px] flex-col rounded-lg border border-notion-border bg-notion-sidebar/30 p-3 transition-all";

export const TASKS_BOARD_COLUMN_HEADER_CLASS =
  "flex items-center justify-between gap-2 pb-3 border-b border-notion-border/50";

export const TASKS_BOARD_CARD_CLASS =
  "group relative flex w-full min-w-0 flex-col items-start gap-1.5 rounded-md border border-notion-border bg-white px-3 py-2.5 text-left text-notion-text shadow-sm transition-all hover:bg-notion-hover hover:border-notion-accent/20 active:translate-y-px";

export const TASKS_BOARD_CARD_ACTIVE_CLASS =
  `${TASKS_BOARD_CARD_CLASS} ring-1 ring-notion-accent/30 border-notion-accent/30 bg-notion-hover shadow-md`;

export const TASKS_DETAIL_PANEL_CLASS =
  "flex flex-col gap-4 rounded-xl border border-notion-border bg-white p-4 sm:p-6 shadow-sm";

export const TASKS_DETAIL_META_ITEM_CLASS =
  "rounded-md border border-notion-border bg-notion-sidebar/30 px-3 py-2";

export const OVERVIEW_META_CLASS =
  "mb-4 grid min-w-0 gap-3 rounded-lg border border-notion-border bg-notion-sidebar/30 p-4 text-[13px] text-notion-text sm:grid-cols-2 xl:grid-cols-3";

export const OVERVIEW_PLAYBOOK_CLASS =
  "mb-6 rounded-xl border border-notion-border bg-white p-4 sm:p-6 shadow-sm";

export const OVERVIEW_PLAYBOOK_GRID_CLASS = "grid gap-4 md:grid-cols-2 mt-4";

export const OVERVIEW_PLAYBOOK_CARD_CLASS = "rounded-lg border border-notion-border bg-notion-sidebar/20 p-4";

export const OVERVIEW_PLAYBOOK_TITLE_CLASS =
  "text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";

export const OVERVIEW_PLAYBOOK_LIST_CLASS = "mt-3 list-decimal space-y-2 pl-5 text-[13px] leading-relaxed text-notion-text";

export const OVERVIEW_MEMBER_LIST_CLASS = "teams-member-list flex flex-col gap-2 mt-4";

export const MAILBOX_META_CLASS =
  "mb-4 grid min-w-0 gap-3 rounded-lg border border-notion-border bg-notion-sidebar/30 p-4 text-[13px] text-notion-text sm:grid-cols-2 xl:grid-cols-4";

export const MAILBOX_SHELL_CLASS =
  "teams-chat-shell grid min-w-0 gap-4 lg:grid-cols-[minmax(240px,300px)_minmax(0,1fr)]";

export const MAILBOX_MEMBER_LIST_CLASS =
  "teams-chat-members flex max-h-[220px] min-w-0 flex-col gap-1.5 overflow-auto rounded-xl border border-notion-border bg-notion-sidebar/20 p-3 lg:max-h-[640px]";

export const MAILBOX_PANEL_CLASS =
  "teams-chat-panel flex min-w-0 flex-col gap-4 rounded-xl border border-notion-border bg-[#f7f7f5] p-4 shadow-sm";

export const MAILBOX_CHAT_JUMP_BUTTON_CLASS =
  "inline-flex h-8 items-center justify-center gap-1.5 rounded-md border border-notion-border bg-white px-3 text-[12px] font-medium text-notion-text-muted shadow-sm transition hover:bg-notion-hover hover:text-notion-text active:translate-y-px";

export const MAILBOX_MESSAGE_LIST_CLASS =
  "teams-chat-messages m-0 flex max-h-[480px] list-none flex-col gap-2 overflow-auto p-0 pr-1";

export const MAILBOX_MESSAGE_ITEM_CLASS =
  "teams-message-item group relative flex w-full flex-col px-4 py-1";

export const MAILBOX_MESSAGE_BUBBLE_CLASS =
  "teams-message-bubble relative max-w-[85%] rounded-[18px] px-4 py-2.5 text-[14px] leading-relaxed shadow-[0_2px_4px_rgba(0,0,0,0.03)] transition-all";

export const MAILBOX_MESSAGE_BUBBLE_OUTGOING_CLASS =
  "teams-message-bubble-outgoing self-end rounded-tr-[4px] border border-notion-accent/15 bg-[#edf2ff] text-notion-text";

export const MAILBOX_MESSAGE_BUBBLE_INCOMING_CLASS =
  "teams-message-bubble-incoming self-start rounded-tl-[4px] border border-notion-border bg-white text-notion-text";

export const MAILBOX_CONVERSATION_EMPTY_CLASS =
  "teams-conversation-empty mx-auto my-8 rounded-full border border-notion-border bg-white px-6 py-2 text-sm font-medium text-notion-text-muted italic shadow-sm";

export const MAILBOX_MESSAGE_HEAD_CLASS =
  "mb-1 flex flex-wrap items-center gap-2 text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";

export const MAILBOX_ADVANCED_GRID_CLASS = "teams-message-grid grid min-w-0 gap-4 lg:grid-cols-2";

export const MAILBOX_ADVANCED_PANEL_CLASS =
  "teams-message-panel flex min-w-0 flex-col gap-3 rounded-lg border border-notion-border bg-notion-sidebar/10 p-4";

export const EVENTS_LIST_CLASS = "teams-event-list flex flex-col gap-3 mt-4";

export const EVENTS_ITEM_CLASS = "rounded-lg border border-notion-border bg-white p-3 shadow-sm transition-all hover:bg-notion-hover/30";

export const EVENTS_ITEM_HEAD_CLASS =
  "teams-event-head mb-2 flex flex-wrap items-center gap-2 text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";

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

export const APP_WORKBENCH_HEADER_CLASS =
  "flex flex-wrap items-center justify-end gap-2 bg-white px-4 py-3 sm:gap-3 sm:px-6 sm:py-4 border-b border-notion-border";

export const APP_WORKBENCH_HEADER_STATUS_CLASS =
  "inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-md border border-notion-border bg-notion-sidebar px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted shadow-sm transition hover:bg-notion-hover";

export const APP_WORKBENCH_SIDEBAR_TOGGLE_BUTTON_CLASS =
  "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text lg:hidden";

export const APP_WORKBENCH_ACCOUNT_MENU_BUTTON_CLASS =
  "inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-md border border-notion-border bg-white px-3 text-[13px] font-medium text-notion-text shadow-sm transition hover:bg-notion-hover active:translate-y-px";

export const APP_WORKSPACE_ROOT_CLASS =
  "workspace flex min-h-0 w-full flex-1 flex-row items-stretch overflow-hidden";

export const APP_WORKSPACE_ROOT_COLLAPSED_CLASS =
  "workspace collapsed flex min-h-0 w-full flex-1 flex-row items-stretch overflow-hidden";

export const APP_WORKSPACE_SPLITTER_CLASS =
  "workspace-splitter hidden w-1 shrink-0 cursor-col-resize bg-transparent transition hover:bg-notion-accent lg:block lg:-mx-0.5";

export const APP_WORKSPACE_RIGHT_CLASS =
  "workspace-right flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-white";

export const ROUTE_FALLBACK_SHELL_CLASS =
  "mx-auto flex min-h-[40vh] w-full max-w-3xl items-center justify-center rounded-xl bg-notion-sidebar px-6 py-10 text-sm font-medium text-notion-text-muted";

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
  "workspace-left flex h-full min-h-0 w-[var(--agents-panel-width,288px)] shrink-0 flex-col overflow-hidden border-r border-notion-border bg-notion-sidebar p-2 text-notion-text backdrop-blur-sm transition-all duration-300 max-[720px]:fixed max-[720px]:inset-x-2 max-[720px]:bottom-[calc(8px+env(safe-area-inset-bottom,0px))] max-[720px]:top-[var(--agenthub-workspace-top,calc(56px+env(safe-area-inset-top,0px)))] max-[720px]:z-30 max-[720px]:w-auto max-[720px]:overflow-auto max-[720px]:shadow-[0_24px_60px_rgba(15,20,30,0.35)]";

export const AGENTS_PANEL_COLLAPSED_CLASS =
  "workspace-left collapsed flex h-full min-h-0 w-14 shrink-0 flex-col overflow-hidden border-r border-notion-border bg-notion-sidebar p-2 text-notion-text backdrop-blur-sm transition-all duration-300";

export const AGENTS_PANEL_BACKDROP_CLASS =
  "agents-backdrop fixed inset-0 z-20 bg-black/40 lg:hidden";

export const AGENTS_PANEL_BODY_CLASS =
  "agent-layout flex min-h-0 flex-1 flex-col overflow-hidden";

export const AGENTS_TOOLBAR_CLASS = "mb-4 flex items-center justify-between px-2 pt-2";
export const AGENTS_TOOLBAR_ACTIONS_CLASS = "flex items-center gap-1.5";

export const AGENTS_CREATE_BUTTON_CLASS =
  "flex h-8 items-center gap-1.5 rounded-md border border-notion-border bg-white px-3 text-xs font-semibold text-notion-text shadow-sm transition hover:bg-notion-hover hover:text-notion-accent active:translate-y-px";

export const AGENTS_ROW_CLASS =
  "agents-workbench-row group relative flex min-w-0 cursor-pointer select-none flex-col gap-1 rounded-lg border border-transparent px-3 py-2.5 text-notion-text transition-all hover:bg-notion-hover/80";

export const AGENTS_ROW_ACTIVE_CLASS =
  "agents-workbench-row active group relative flex min-w-0 cursor-pointer select-none flex-col gap-1 rounded-lg border border-notion-accent/10 bg-notion-hover px-3 py-2.5 text-notion-text ring-1 ring-notion-accent/20 shadow-[0_1px_2px_rgba(15,23,42,0.04)]";

export const OUTPUT_HEADER_ROOT_CLASS =
  "output-header sticky top-0 z-30 flex flex-col gap-2 bg-white/88 px-4 py-3 backdrop-blur-md transition-all sm:px-6 sm:py-4";

export const OUTPUT_HEADER_TITLE_CLASS =
  "output-title min-w-0 flex items-start justify-between gap-3";

export const OUTPUT_HEADER_TITLE_TEXT_CLASS =
  "output-title-text flex min-w-0 flex-1 flex-col gap-0.5";

export const OUTPUT_HEADER_TITLE_MAIN_CLASS =
  "output-title-main flex min-w-0 items-center gap-2";

export const OUTPUT_HEADER_TITLE_HEADING_CLASS =
  "truncate text-sm font-semibold leading-tight text-notion-text sm:text-base";

export const OUTPUT_HEADER_META_CLASS =
  "output-meta flex min-w-0 flex-wrap items-center gap-2 text-[11px] text-notion-text-muted [&>*]:min-w-0";

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
  "output-subtitle-row flex min-w-0 items-center max-w-full";

export const OUTPUT_HEADER_SUBTITLE_CLASS =
  "output-subtitle truncate text-[12px] text-notion-text-muted leading-relaxed";

export const OUTPUT_BODY_ROOT_CLASS =
  "output-body flex flex-col min-h-0 flex-1 bg-[#f7f7f5] overflow-hidden";

export const OUTPUT_BODY_ACP_ROOT_CLASS =
  "output-body output-body-acp flex flex-col min-h-0 flex-1 bg-[#f7f7f5] overflow-hidden";

export const OUTPUT_BODY_LOADING_CLASS =
  "output-loading flex h-full min-h-40 flex-col items-center justify-center gap-3 text-notion-text-muted";

export const OUTPUT_BODY_EMPTY_CLASS =
  "output-empty flex h-full min-h-40 flex-col items-center justify-center gap-2 px-6 text-center";

export const INPUT_DOCK_ROOT_CLASS =
  "input docked sticky bottom-5 z-50 mx-4 flex w-full flex-col gap-1.5 rounded-xl border border-notion-border bg-white/92 p-2.5 shadow-[0_10px_30px_rgba(15,23,42,0.10)] backdrop-blur-md transition-all sm:bottom-6 sm:mx-6 sm:p-3";

export const INPUT_DOCK_INTERRUPT_BUTTON_CLASS =
  "acp-interrupt-button input-interrupt-button inline-flex h-8 items-center justify-center rounded-md border border-state-warning-border bg-state-warning-bg px-3 text-[12px] font-semibold text-state-warning-text hover:bg-state-warning-bg/80 transition active:translate-y-px";

export const INPUT_DOCK_HISTORY_BUTTON_CLASS =
  "history-toggle inline-flex h-8 items-center gap-1.5 rounded-md border border-notion-border bg-white px-3 text-[12px] font-semibold text-notion-text shadow-[0_1px_2px_rgba(15,23,42,0.04)] transition hover:bg-notion-hover active:translate-y-px";

export const INPUT_DOCK_HISTORY_MENU_CLASS =
  "input-history-menu absolute bottom-[calc(100%+0.5rem)] left-0 z-30 max-h-60 min-w-[15rem] max-w-[min(26rem,calc(100vw-48px))] overflow-y-auto rounded-xl border border-black/[0.06] bg-white/98 p-1.5 shadow-[0_20px_24px_rgba(25,25,25,0.05),0_5px_8px_rgba(25,25,25,0.027),0_0_0_1px_rgba(42,28,0,0.07)] backdrop-blur-md";

export const INPUT_DOCK_HISTORY_ITEM_CLASS =
  "input-history-item block w-full rounded-lg px-3 py-2 text-left text-[13px] text-notion-text hover:bg-notion-hover transition";

export const INPUT_DOCK_TEXTAREA_CLASS =
  "min-h-[2.75rem] w-full bg-transparent px-1 py-1.5 text-[14px] leading-6 text-notion-text placeholder-notion-text-muted/50 outline-none transition resize-none";

export const INPUT_DOCK_SEND_BUTTON_CLASS =
  "inline-flex h-8 shrink-0 items-center justify-center rounded-md bg-notion-accent px-3.5 text-[12px] font-semibold text-white transition hover:bg-notion-accent/90 disabled:opacity-50 disabled:cursor-not-allowed active:translate-y-px";

export const TEAM_PAGE_ROOT_CLASS =
  "mx-auto flex min-h-0 flex-1 w-full max-w-[1680px] flex-col gap-3 overflow-hidden bg-white px-3 py-2 sm:px-4 lg:px-6";

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
  "shrink-0 rounded-xl border border-notion-border bg-white p-2 shadow-lg backdrop-blur-md";

export const TEAM_WORKBENCH_HEADER_SHELL_CLASS =
  "flex flex-wrap items-center justify-between gap-3 bg-white px-4 py-3 sm:px-6 sm:py-4 border-b border-notion-border transition-all";

export const TEAM_WORKBENCH_HEADER_ICON_BUTTON_CLASS =
  "inline-flex h-8 w-8 items-center justify-center rounded-md text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text";

export const TEAM_WORKBENCH_HEADER_STATUS_CLASS =
  "inline-flex items-center gap-1.5 rounded-md border border-notion-border bg-notion-sidebar px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted transition hover:bg-notion-hover";

export const TEAM_WORKBENCH_WORKSPACE_SHELL_CLASS =
  "shrink-0 flex min-h-0 flex-col rounded-xl border border-notion-border bg-white px-4 py-3 shadow-sm transition-all";

export const TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS =
  "min-w-0 bg-white px-4 py-3 border-r border-notion-border last:border-r-0";

export const TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS =
  "text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";

export const TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS =
  "mt-1 text-[13px] leading-relaxed text-notion-text";

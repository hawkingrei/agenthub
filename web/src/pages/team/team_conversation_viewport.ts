import { isNearBottom } from "../../scroll";

export type TeamConversationWindow<T> = {
  items: T[];
  offset: number;
  total: number;
};

type DeriveTeamThreadJumpStateArgs = {
  active: boolean;
  stickToBottom: boolean;
  pendingCount: number;
};

type DeriveTeamThreadStickToBottomArgs = {
  scrollHeight: number;
  scrollTop: number;
  clientHeight: number;
  wasStickToBottom: boolean;
  previousScrollTop: number | null;
  threshold?: number;
  userScrollUpEpsilon?: number;
};

const DEFAULT_TEAM_THREAD_THRESHOLD_PX = 120;
const DEFAULT_TEAM_THREAD_USER_SCROLL_UP_EPSILON_PX = 24;

export const DEFAULT_TEAM_CONVERSATION_TAIL_WINDOW_SIZE = 10;

export function windowTeamConversation<T>(
  items: T[],
  stickToBottom: boolean,
  windowSize: number
): TeamConversationWindow<T> {
  const total = items.length;
  if (windowSize <= 0 || total <= windowSize || !stickToBottom) {
    return { items, offset: 0, total };
  }
  const offset = Math.max(0, total - windowSize);
  return {
    items: items.slice(offset),
    offset,
    total,
  };
}

export function deriveTeamThreadJumpState({
  active,
  stickToBottom,
  pendingCount,
}: DeriveTeamThreadJumpStateArgs): {
  showJump: boolean;
  showBadge: boolean;
} {
  const showJump = active && !stickToBottom;
  return {
    showJump,
    showBadge: showJump && pendingCount > 0,
  };
}

export function deriveTeamThreadStickToBottom({
  scrollHeight,
  scrollTop,
  clientHeight,
  wasStickToBottom,
  previousScrollTop,
  threshold = DEFAULT_TEAM_THREAD_THRESHOLD_PX,
  userScrollUpEpsilon = DEFAULT_TEAM_THREAD_USER_SCROLL_UP_EPSILON_PX,
}: DeriveTeamThreadStickToBottomArgs): boolean {
  if (isNearBottom(scrollHeight, scrollTop, clientHeight, threshold)) {
    return true;
  }
  if (!wasStickToBottom) {
    return false;
  }
  if (previousScrollTop == null) {
    return true;
  }
  const movedUp = scrollTop < previousScrollTop - userScrollUpEpsilon;
  return !movedUp;
}

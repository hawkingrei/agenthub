import { isNearBottom } from "../scroll";

export type ThreadViewport = {
  top: number;
  height: number;
};

type DeriveThreadJumpStateArgs = {
  active: boolean;
  stickToBottom: boolean;
  pendingCount: number;
};

type DeriveThreadStickToBottomArgs = {
  scrollHeight: number;
  scrollTop: number;
  clientHeight: number;
  wasStickToBottom: boolean;
  previousScrollTop: number | null;
  threshold?: number;
  userScrollUpEpsilon?: number;
};

const DEFAULT_USER_SCROLL_UP_EPSILON_PX = 24;

export function nextThreadViewport(
  previous: ThreadViewport,
  nextTop: number,
  nextHeight: number
): ThreadViewport {
  if (previous.top === nextTop && previous.height === nextHeight) {
    return previous;
  }
  return {
    top: nextTop,
    height: nextHeight,
  };
}

export function normalizeThreadAvgHeightEstimate(
  previous: number,
  scrollHeight: number,
  itemCount: number
): number {
  const count = Math.max(1, itemCount);
  const estimate = scrollHeight / count;
  if (!Number.isFinite(estimate) || estimate <= 0) {
    return previous;
  }
  const normalized = Math.min(220, Math.max(24, estimate));
  if (Math.abs(previous - normalized) < 1) {
    return previous;
  }
  return normalized;
}

export function restoreThreadScrollTop(
  savedTop: number,
  scrollHeight: number,
  clientHeight: number
): number {
  const maxTop = Math.max(0, scrollHeight - clientHeight);
  return Math.min(savedTop, maxTop);
}

export function deriveThreadJumpState({
  active,
  stickToBottom,
  pendingCount,
}: DeriveThreadJumpStateArgs): {
  showJump: boolean;
  showBadge: boolean;
} {
  const showJump = active && !stickToBottom;
  return {
    showJump,
    showBadge: showJump && pendingCount > 0,
  };
}

export function deriveThreadStickToBottom({
  scrollHeight,
  scrollTop,
  clientHeight,
  wasStickToBottom,
  previousScrollTop,
  threshold = 120,
  userScrollUpEpsilon = DEFAULT_USER_SCROLL_UP_EPSILON_PX,
}: DeriveThreadStickToBottomArgs): boolean {
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

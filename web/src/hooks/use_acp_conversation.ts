import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type RefObject,
} from "react";
import { AcpView } from "../acp";
import {
  applyConversationFreeze,
  buildConversationMessages,
  ConversationItem,
  deriveConversationFreezeCursor,
  windowConversation,
} from "../conversation";
import { isNearBottom } from "../scroll";
import type { SeqComparable } from "../seq_order";
import { createRafThrottle } from "../raf_throttle";

type EventMeta = {
  oldestId: number | null;
  hasMore: boolean;
  loading: boolean;
  loaded: boolean;
};

type UseAcpConversationArgs = {
  acpView: AcpView;
  activeAgent: string | null;
  activeSessionId: string | null;
  acpTab: "conversation" | "debug";
  eventMeta: Record<string, EventMeta>;
  isAgentActive: boolean;
  onLoadOlder: () => void;
};

type UseAcpConversationResult = {
  acpConversationRef: RefObject<HTMLDivElement>;
  conversationRenderItems: ConversationItem[];
  conversationWindowOffset: number;
  conversationVirtualTopSpacer: number;
  conversationVirtualBottomSpacer: number;
  isFrozenView: boolean;
  collapseCutoff: number;
  shouldAutoCollapse: boolean;
  conversationStickToBottom: boolean;
  conversationPendingCount: number;
  conversationAvgHeight: number;
  conversationTotalItems: number;
  conversationSourceItems: number;
  conversationRenderedItems: number;
  conversationVirtualized: boolean;
  focusedConversationToolCallId: string | null;
  showConversationJump: boolean;
  showConversationBadge: boolean;
  jumpToConversationBottom: () => void;
  jumpToConversationToolCall: (toolCallId: string) => boolean;
  handleConversationScroll: () => void;
};

type ConversationVirtualSlice = {
  items: ConversationItem[];
  offset: number;
  topSpacer: number;
  bottomSpacer: number;
};

type ConversationViewport = {
  top: number;
  height: number;
};

const VIRTUALIZATION_MIN_ITEMS = 140;
const VIRTUALIZATION_OVERSCAN = 14;
const STICK_BOTTOM_STRICT_THRESHOLD = 4;
const TAIL_PAYLOAD_MAX_DEPTH = 2;
const TAIL_PAYLOAD_MAX_ENTRIES = 24;
const TOOL_CALL_JUMP_CONTEXT_LINES = 4;
const TOOL_CALL_JUMP_MIN_ROW_HEIGHT = 24;
const FOCUSED_TOOL_CALL_RESET_DELAY_MS = 2500;

export function buildConversationTailKey(conversationMessages: ConversationItem[]): string {
  if (conversationMessages.length === 0) return "empty";
  const last = conversationMessages[conversationMessages.length - 1];
  const base = `${last.kind}:${last.event_id ?? last.seq ?? "na"}`;
  if (last.kind === "tool_call") {
    const contentLen = last.content?.length ?? 0;
    const terminalLen = last.terminal_output?.length ?? 0;
    const rawInLen = estimateTailPayloadSize(last.raw_input);
    const rawOutLen = estimateTailPayloadSize(last.raw_output);
    return `${base}:${contentLen}:${terminalLen}:${rawInLen}:${rawOutLen}`;
  }
  return `${base}:${last.text?.length ?? 0}`;
}

export function estimateTailPayloadSize(value: unknown): number {
  return estimateTailPayloadSizeInternal(value, 0);
}

function estimateTailPayloadSizeInternal(value: unknown, depth: number): number {
  if (value == null) return 0;
  if (typeof value === "string") return value.length;
  if (typeof value === "number" || typeof value === "boolean") return 8;
  if (depth >= TAIL_PAYLOAD_MAX_DEPTH) return 16;
  if (Array.isArray(value)) {
    let size = value.length;
    const limit = Math.min(TAIL_PAYLOAD_MAX_ENTRIES, value.length);
    for (let i = 0; i < limit; i += 1) {
      size += estimateTailPayloadSizeInternal(value[i], depth + 1);
    }
    return size;
  }
  if (typeof value === "object") {
    let size = 0;
    let count = 0;
    let hasMore = false;
    const record = value as Record<string, unknown>;
    for (const key in record) {
      if (!Object.prototype.hasOwnProperty.call(record, key)) continue;
      if (count >= TAIL_PAYLOAD_MAX_ENTRIES) {
        hasMore = true;
        break;
      }
      size += 2;
      size += key.length;
      size += estimateTailPayloadSizeInternal(record[key], depth + 1);
      count += 1;
    }
    if (hasMore) size += 8;
    return size;
  }
  return 0;
}

export function shouldLoadOlderFromMeta(
  activeAgent: string | null,
  activeSessionId: string | null,
  eventMeta: Record<string, EventMeta>
): boolean {
  if (!activeAgent) return false;
  const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
  const meta = eventMeta[key];
  if (!meta || meta.loading || !meta.hasMore || meta.oldestId == null) {
    return false;
  }
  return true;
}

export function shouldUseConversationVirtualization(
  conversationStickToBottom: boolean,
  sourceItemsLength: number
): boolean {
  return !conversationStickToBottom && sourceItemsLength >= VIRTUALIZATION_MIN_ITEMS;
}

export function nextConversationViewport(
  prev: ConversationViewport,
  nextTop: number,
  nextHeight: number
): ConversationViewport {
  if (prev.top === nextTop && prev.height === nextHeight) {
    return prev;
  }
  return {
    top: nextTop,
    height: nextHeight,
  };
}

export function shouldAutoLoadConversationHistory(
  acpTab: "conversation" | "debug",
  activeAgent: string | null,
  canLoadOlder: boolean,
  conversationMessageCount: number,
  minMessages: number = 12
): boolean {
  if (acpTab !== "conversation") return false;
  if (!activeAgent) return false;
  if (!canLoadOlder) return false;
  return conversationMessageCount < minMessages;
}

export function normalizeConversationAvgHeightEstimate(
  previous: number,
  scrollHeight: number,
  messageCount: number
): number {
  const count = Math.max(1, messageCount);
  const estimate = scrollHeight / count;
  if (!Number.isFinite(estimate) || estimate <= 0) return previous;
  const normalized = Math.min(220, Math.max(24, estimate));
  if (Math.abs(previous - normalized) < 1) return previous;
  return normalized;
}

export function restoreConversationScrollTop(
  savedTop: number,
  scrollHeight: number,
  clientHeight: number
): number {
  const maxTop = Math.max(0, scrollHeight - clientHeight);
  return Math.min(savedTop, maxTop);
}

export function deriveConversationJumpState(
  acpTab: "conversation" | "debug",
  conversationStickToBottom: boolean,
  conversationPendingCount: number
): { showConversationJump: boolean; showConversationBadge: boolean } {
  const showConversationJump =
    acpTab === "conversation" && !conversationStickToBottom;
  return {
    showConversationJump,
    showConversationBadge: showConversationJump && conversationPendingCount > 0,
  };
}

export function findConversationToolCallIndex(
  items: ConversationItem[],
  toolCallId: string
): number {
  if (!toolCallId) return -1;
  for (let idx = 0; idx < items.length; idx += 1) {
    const item = items[idx];
    if (item.kind !== "tool_call") continue;
    if (item.id === toolCallId) return idx;
  }
  return -1;
}

export function estimateToolCallJumpTop(
  targetIndex: number,
  averageRowHeight: number
): number {
  return Math.max(
    0,
    Math.round(
      (targetIndex - TOOL_CALL_JUMP_CONTEXT_LINES) *
        Math.max(TOOL_CALL_JUMP_MIN_ROW_HEIGHT, averageRowHeight)
    )
  );
}

export function findToolCallNodeById(
  container: ParentNode,
  toolCallId: string
): Element | null {
  const candidates = container.querySelectorAll("[data-tool-call-id]");
  for (const candidate of candidates) {
    if (candidate.getAttribute("data-tool-call-id") === toolCallId) {
      return candidate;
    }
  }
  return null;
}

export function buildVirtualConversationSlice(
  sourceItems: ConversationItem[],
  sourceOffset: number,
  viewportTop: number,
  viewportHeight: number,
  estimatedItemHeight: number,
  overscan: number = VIRTUALIZATION_OVERSCAN
): ConversationVirtualSlice {
  const total = sourceItems.length;
  if (total === 0) {
    return {
      items: sourceItems,
      offset: sourceOffset,
      topSpacer: 0,
      bottomSpacer: 0,
    };
  }
  const itemHeight = Number.isFinite(estimatedItemHeight) && estimatedItemHeight > 0
    ? estimatedItemHeight
    : 48;
  const safeTop = Math.max(0, viewportTop);
  const safeHeight = Math.max(1, viewportHeight);
  const estimatedTotalHeight = total * itemHeight;
  const maxViewportTop = Math.max(0, estimatedTotalHeight - safeHeight);
  const clampedTop = Math.min(safeTop, maxViewportTop);
  const rawStart = Math.max(0, Math.floor(clampedTop / itemHeight) - overscan);
  const visibleCount = Math.max(36, Math.ceil(safeHeight / itemHeight) + overscan * 2);
  const maxStart = Math.max(0, total - visibleCount);
  const start = Math.min(rawStart, maxStart);
  const end = Math.min(total, Math.max(start + 1, start + visibleCount));
  return {
    items: sourceItems.slice(start, end),
    offset: sourceOffset + start,
    topSpacer: Math.max(0, Math.round(start * itemHeight)),
    bottomSpacer: Math.max(0, Math.round((total - end) * itemHeight)),
  };
}

export function useAcpConversation({
  acpView,
  activeAgent,
  activeSessionId,
  acpTab,
  eventMeta,
  isAgentActive,
  onLoadOlder,
}: UseAcpConversationArgs): UseAcpConversationResult {
  const acpConversationRef = useRef<HTMLDivElement | null>(null);
  const conversationScrollThrottleRef = useRef<ReturnType<
    typeof createRafThrottle
  > | null>(null);
  const conversationBottomAlignThrottleRef = useRef<ReturnType<
    typeof createRafThrottle
  > | null>(null);
  const alignConversationBottomNowRef = useRef<() => void>(() => {});
  const acpStickToBottomRef = useRef(true);
  const conversationScrollRef = useRef<{
    top: number;
    height: number;
    clientHeight: number;
    stickToBottom: boolean;
  } | null>(null);
  const pendingScrollAdjustRef = useRef<{
    prevHeight: number;
    prevTop: number;
  } | null>(null);
  const focusedToolCallResetTimerRef = useRef<number | null>(null);
  const [conversationAvgHeight, setConversationAvgHeight] = useState(48);
  const [conversationViewport, setConversationViewport] = useState({
    top: 0,
    height: 0,
  });
  const didAutoAlignConversationRef = useRef(false);
  const [conversationStickToBottom, setConversationStickToBottom] = useState(true);
  const [conversationFrozen, setConversationFrozen] = useState(false);
  const [conversationFreezeCursor, setConversationFreezeCursor] = useState<
    SeqComparable | null
  >(null);
  const [conversationFrozenItems, setConversationFrozenItems] = useState<
    ConversationItem[]
  >([]);
  const [conversationPendingCount, setConversationPendingCount] = useState(0);
  const [focusedConversationToolCallId, setFocusedConversationToolCallId] = useState<
    string | null
  >(null);
  const conversationMessages = useMemo<ConversationItem[]>(
    () =>
      buildConversationMessages(
        acpView.messages,
        acpView.toolCalls,
        acpView.plan,
        activeSessionId
      ),
    [acpView.messages, acpView.toolCalls, acpView.plan, activeSessionId]
  );
  const conversationWindow = useMemo(
    () => windowConversation(conversationMessages, conversationStickToBottom, 200),
    [conversationMessages, conversationStickToBottom]
  );
  const conversationTailKey = useMemo(
    () => buildConversationTailKey(conversationMessages),
    [conversationMessages]
  );
  const isFrozenView = conversationFrozen && conversationFrozenItems.length > 0;
  const conversationSourceItems = isFrozenView
    ? conversationFrozenItems
    : conversationWindow.items;
  const conversationSourceOffset = isFrozenView ? 0 : conversationWindow.offset;
  const shouldVirtualizeConversation = shouldUseConversationVirtualization(
    conversationStickToBottom,
    conversationSourceItems.length
  );
  const conversationVirtualSlice = useMemo(() => {
    if (!shouldVirtualizeConversation) {
      return {
        items: conversationSourceItems,
        offset: conversationSourceOffset,
        topSpacer: 0,
        bottomSpacer: 0,
      };
    }
    return buildVirtualConversationSlice(
      conversationSourceItems,
      conversationSourceOffset,
      conversationViewport.top,
      conversationViewport.height,
      conversationAvgHeight
    );
  }, [
    shouldVirtualizeConversation,
    conversationSourceItems,
    conversationSourceOffset,
    conversationViewport.top,
    conversationViewport.height,
    conversationAvgHeight,
  ]);
  const conversationRenderItems = conversationVirtualSlice.items;
  const collapseCutoff = Math.max(0, conversationMessages.length - 50);
  const shouldAutoCollapse = conversationMessages.length > 50;

  const shouldLoadOlder = useCallback(() => {
    return shouldLoadOlderFromMeta(activeAgent, activeSessionId, eventMeta);
  }, [activeAgent, activeSessionId, eventMeta]);

  const syncConversationViewport = useCallback(() => {
    const el = acpConversationRef.current;
    if (!el) return;
    setConversationViewport((prev) => {
      const nextTop = el.scrollTop;
      const nextHeight = el.clientHeight;
      return nextConversationViewport(prev, nextTop, nextHeight);
    });
  }, []);

  const prepareForLoadOlder = () => {
    const el = acpConversationRef.current;
    if (!el) return;
    pendingScrollAdjustRef.current = {
      prevHeight: el.scrollHeight,
      prevTop: el.scrollTop,
    };
  };

  const markFocusedConversationToolCall = useCallback((toolCallId: string) => {
    setFocusedConversationToolCallId(toolCallId);
    if (
      typeof window !== "undefined" &&
      focusedToolCallResetTimerRef.current != null
    ) {
      window.clearTimeout(focusedToolCallResetTimerRef.current);
      focusedToolCallResetTimerRef.current = null;
    }
    if (typeof window === "undefined") return;
    focusedToolCallResetTimerRef.current = window.setTimeout(() => {
      setFocusedConversationToolCallId((prev) =>
        prev === toolCallId ? null : prev
      );
      focusedToolCallResetTimerRef.current = null;
    }, FOCUSED_TOOL_CALL_RESET_DELAY_MS);
  }, []);

  const jumpToConversationBottom = useCallback(() => {
    const el = acpConversationRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    syncConversationViewport();
    acpStickToBottomRef.current = true;
    didAutoAlignConversationRef.current = true;
    setConversationStickToBottom(true);
    setConversationFrozen(false);
    setConversationFreezeCursor(null);
    setConversationFrozenItems([]);
    setConversationPendingCount(0);
    setFocusedConversationToolCallId(null);
  }, [syncConversationViewport]);

  const jumpToConversationToolCall = useCallback(
    (toolCallId: string): boolean => {
      const targetId = toolCallId.trim();
      if (!targetId) return false;
      const targetIndex = findConversationToolCallIndex(
        conversationMessages,
        targetId
      );
      if (targetIndex < 0) return false;
      const el = acpConversationRef.current;
      if (!el) return false;

      const estimatedTop = estimateToolCallJumpTop(
        targetIndex,
        conversationAvgHeight
      );
      el.scrollTop = estimatedTop;
      acpStickToBottomRef.current = false;
      setConversationStickToBottom(false);
      setConversationFrozen(false);
      setConversationFreezeCursor(null);
      setConversationFrozenItems([]);
      setConversationPendingCount(0);
      syncConversationViewport();
      markFocusedConversationToolCall(targetId);

      const scrollNodeToCenter = (node: Element | null) => {
        if (!node || !(node instanceof HTMLElement)) return;
        node.scrollIntoView({ block: "center", inline: "nearest" });
        syncConversationViewport();
      };
      const immediateTarget = findToolCallNodeById(el, targetId);
      if (immediateTarget) {
        scrollNodeToCenter(immediateTarget);
        return true;
      }
      return false;
    },
    [
      conversationMessages,
      conversationAvgHeight,
      syncConversationViewport,
      markFocusedConversationToolCall,
    ]
  );

  const alignConversationBottomNow = useCallback(() => {
    const el = acpConversationRef.current;
    if (!el) return;
    if (acpTab !== "conversation") return;
    if (!conversationStickToBottom && !acpStickToBottomRef.current) return;
    el.scrollTop = el.scrollHeight;
    syncConversationViewport();
    if (
      typeof window === "undefined" ||
      typeof window.requestAnimationFrame !== "function"
    ) {
      return;
    }
    if (isNearBottom(el.scrollHeight, el.scrollTop, el.clientHeight, STICK_BOTTOM_STRICT_THRESHOLD)) {
      return;
    }
    window.requestAnimationFrame(() => {
      const node = acpConversationRef.current;
      if (!node) return;
      node.scrollTop = node.scrollHeight;
      syncConversationViewport();
      if (
        isNearBottom(
          node.scrollHeight,
          node.scrollTop,
          node.clientHeight,
          STICK_BOTTOM_STRICT_THRESHOLD
        )
      ) {
        return;
      }
      window.requestAnimationFrame(() => {
        const latest = acpConversationRef.current;
        if (!latest) return;
        latest.scrollTop = latest.scrollHeight;
        syncConversationViewport();
      });
    });
  }, [acpTab, conversationStickToBottom, syncConversationViewport]);

  useEffect(() => {
    alignConversationBottomNowRef.current = alignConversationBottomNow;
  }, [alignConversationBottomNow]);

  const scheduleConversationBottomAlign = useCallback(() => {
    const throttle = conversationBottomAlignThrottleRef.current;
    if (!throttle) {
      alignConversationBottomNowRef.current();
      return;
    }
    throttle.schedule();
  }, []);

  const handleConversationScrollNow = useCallback(() => {
    const el = acpConversationRef.current;
    if (!el) return;
    syncConversationViewport();
    const stick = isNearBottom(el.scrollHeight, el.scrollTop, el.clientHeight);
    acpStickToBottomRef.current = stick;
    if (stick !== conversationStickToBottom) {
      if (!stick) {
        const maxCursor = deriveConversationFreezeCursor(conversationMessages);
        const frozen = applyConversationFreeze(conversationMessages, maxCursor);
        setConversationFrozen(true);
        setConversationFrozenItems(frozen.frozen);
        setConversationFreezeCursor(maxCursor);
        setConversationPendingCount(frozen.pending);
      } else {
        setConversationFrozen(false);
        setConversationFreezeCursor(null);
        setConversationFrozenItems([]);
        setConversationPendingCount(0);
      }
      setConversationStickToBottom(stick);
    }
    if (el.scrollTop < 80 && shouldLoadOlder()) {
      prepareForLoadOlder();
      onLoadOlder();
    }
  }, [
    conversationStickToBottom,
    conversationMessages,
    onLoadOlder,
    syncConversationViewport,
    shouldLoadOlder,
  ]);

  const handleConversationScroll = useCallback(() => {
    const throttle = conversationScrollThrottleRef.current;
    if (!throttle) {
      handleConversationScrollNow();
      return;
    }
    throttle.schedule();
  }, [handleConversationScrollNow]);

  useEffect(() => {
    if (
      typeof window === "undefined" ||
      typeof window.requestAnimationFrame !== "function"
    ) {
      conversationScrollThrottleRef.current = null;
      return;
    }
    conversationScrollThrottleRef.current = createRafThrottle(
      handleConversationScrollNow,
      {
        requestAnimationFrame: window.requestAnimationFrame.bind(window),
        cancelAnimationFrame:
          typeof window.cancelAnimationFrame === "function"
            ? window.cancelAnimationFrame.bind(window)
            : undefined,
      }
    );
    return () => {
      conversationScrollThrottleRef.current?.cancel();
      conversationScrollThrottleRef.current = null;
    };
  }, [handleConversationScrollNow]);

  useEffect(() => {
    if (
      typeof window === "undefined" ||
      typeof window.requestAnimationFrame !== "function"
    ) {
      conversationBottomAlignThrottleRef.current = null;
      return;
    }
    conversationBottomAlignThrottleRef.current = createRafThrottle(
      () => {
        alignConversationBottomNowRef.current();
      },
      {
        requestAnimationFrame: window.requestAnimationFrame.bind(window),
        cancelAnimationFrame:
          typeof window.cancelAnimationFrame === "function"
            ? window.cancelAnimationFrame.bind(window)
            : undefined,
      }
    );
    return () => {
      conversationBottomAlignThrottleRef.current?.cancel();
      conversationBottomAlignThrottleRef.current = null;
    };
  }, []);

  useEffect(() => {
    return () => {
      if (
        typeof window !== "undefined" &&
        focusedToolCallResetTimerRef.current != null
      ) {
        window.clearTimeout(focusedToolCallResetTimerRef.current);
        focusedToolCallResetTimerRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    if (
      !shouldAutoLoadConversationHistory(
        acpTab,
        activeAgent,
        shouldLoadOlder(),
        conversationMessages.length
      )
    ) {
      return;
    }
    prepareForLoadOlder();
    onLoadOlder();
  }, [
    conversationMessages.length,
    acpTab,
    activeAgent,
    activeSessionId,
    eventMeta,
    onLoadOlder,
    shouldLoadOlder,
  ]);

  useEffect(() => {
    didAutoAlignConversationRef.current = false;
    acpStickToBottomRef.current = true;
    pendingScrollAdjustRef.current = null;
    setConversationStickToBottom(true);
    setConversationFrozen(false);
    setConversationFreezeCursor(null);
    setConversationFrozenItems([]);
    setConversationPendingCount(0);
    setFocusedConversationToolCallId(null);
    setConversationViewport((prev) => ({
      top: 0,
      height: prev.height,
    }));
  }, [activeAgent, activeSessionId]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!conversationStickToBottom) return;
    scheduleConversationBottomAlign();
  }, [
    acpTab,
    conversationStickToBottom,
    conversationTailKey,
    conversationMessages.length,
    conversationWindow.offset,
    scheduleConversationBottomAlign,
  ]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!shouldVirtualizeConversation) return;
    syncConversationViewport();
  }, [acpTab, conversationTailKey, shouldVirtualizeConversation, syncConversationViewport]);

  useEffect(() => {
    const el = acpConversationRef.current;
    if (!el) return;
    if (acpTab !== "conversation") {
      conversationScrollRef.current = {
        top: el.scrollTop,
        height: el.scrollHeight,
        clientHeight: el.clientHeight,
        stickToBottom: conversationStickToBottom,
      };
      return;
    }
    const saved = conversationScrollRef.current;
    if (!saved) return;
    if (saved.stickToBottom) {
      jumpToConversationBottom();
      return;
    }
    acpStickToBottomRef.current = false;
    setConversationStickToBottom(false);
    el.scrollTop = restoreConversationScrollTop(
      saved.top,
      el.scrollHeight,
      el.clientHeight
    );
  }, [acpTab, conversationStickToBottom, conversationTailKey, jumpToConversationBottom]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!conversationStickToBottom) return;
    const el = acpConversationRef.current;
    if (!el) return;
    setConversationAvgHeight((prev) =>
      normalizeConversationAvgHeightEstimate(
        prev,
        el.scrollHeight,
        conversationMessages.length
      )
    );
  }, [conversationMessages.length, acpTab, conversationStickToBottom]);

  useEffect(() => {
    const el = acpConversationRef.current;
    const pending = pendingScrollAdjustRef.current;
    if (!el || !pending) return;
    const nextHeight = el.scrollHeight;
    el.scrollTop = nextHeight - pending.prevHeight + pending.prevTop;
    pendingScrollAdjustRef.current = null;
    syncConversationViewport();
  }, [acpView.messages.length, syncConversationViewport]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!conversationFrozen) return;
    const { frozen, pending } = applyConversationFreeze(
      conversationMessages,
      conversationFreezeCursor
    );
    if (frozen.length !== conversationFrozenItems.length) {
      setConversationFrozenItems(frozen);
    }
    if (pending !== conversationPendingCount) {
      setConversationPendingCount(pending);
    }
  }, [
    conversationMessages,
    conversationFrozen,
    conversationFreezeCursor,
    acpTab,
    conversationFrozenItems.length,
    conversationPendingCount,
  ]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!conversationFrozen) return;
    if (conversationMessages.length === 0) return;
    if (conversationFrozenItems.length > 0) return;
    // Guard against stale freeze cursor/state causing an empty visible list.
    const maxCursor = deriveConversationFreezeCursor(conversationMessages);
    const { frozen, pending } = applyConversationFreeze(
      conversationMessages,
      maxCursor
    );
    if (frozen.length === 0) {
      jumpToConversationBottom();
      return;
    }
    setConversationFreezeCursor(maxCursor);
    setConversationFrozenItems(frozen);
    setConversationPendingCount(pending);
  }, [
    acpTab,
    conversationFrozen,
    conversationMessages,
    conversationFrozenItems.length,
    jumpToConversationBottom,
  ]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!acpView.hasAcp) return;
    if (!activeAgent) return;
    if (isAgentActive) return;
    if (didAutoAlignConversationRef.current) return;
    if (conversationMessages.length === 0) return;
    jumpToConversationBottom();
    didAutoAlignConversationRef.current = true;
  }, [
    acpTab,
    acpView.hasAcp,
    activeAgent,
    isAgentActive,
    conversationMessages.length,
    jumpToConversationBottom,
  ]);

  const { showConversationJump, showConversationBadge } = deriveConversationJumpState(
    acpTab,
    conversationStickToBottom,
    conversationPendingCount
  );

  return {
    acpConversationRef,
    conversationRenderItems,
    conversationWindowOffset: conversationVirtualSlice.offset,
    conversationVirtualTopSpacer: conversationVirtualSlice.topSpacer,
    conversationVirtualBottomSpacer: conversationVirtualSlice.bottomSpacer,
    isFrozenView,
    collapseCutoff,
    shouldAutoCollapse,
    conversationStickToBottom,
    conversationPendingCount,
    conversationAvgHeight,
    conversationTotalItems: conversationMessages.length,
    conversationSourceItems: conversationSourceItems.length,
    conversationRenderedItems: conversationRenderItems.length,
    conversationVirtualized: shouldVirtualizeConversation,
    focusedConversationToolCallId,
    showConversationJump,
    showConversationBadge,
    jumpToConversationBottom,
    jumpToConversationToolCall,
    handleConversationScroll,
  };
}

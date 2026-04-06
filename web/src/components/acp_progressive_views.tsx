import React from "react";
import { ActionButton } from "../ui/primitives";

const ACP_SEGMENTED_FOOTER_CLASS =
  "acp-segmented-footer flex flex-wrap items-center justify-between gap-1.5";
const ACP_SEGMENTED_META_CLASS = "acp-segmented-meta text-[11px] text-slate-500";

export function useProgressiveVisibleCount(
  total: number,
  initial: number,
  step: number
): {
  visibleCount: number;
  hasMore: boolean;
  remaining: number;
  showMore: () => void;
} {
  const safeInitial = Math.max(1, initial);
  const safeStep = Math.max(1, step);
  const [visibleCount, setVisibleCount] = React.useState(() =>
    Math.min(total, safeInitial)
  );

  React.useEffect(() => {
    setVisibleCount(Math.min(total, safeInitial));
  }, [total, safeInitial]);

  const showMore = React.useCallback(() => {
    setVisibleCount((prev) => Math.min(total, prev + safeStep));
  }, [safeStep, total]);

  const hasMore = visibleCount < total;
  return {
    visibleCount,
    hasMore,
    remaining: hasMore ? total - visibleCount : 0,
    showMore,
  };
}

export function useProgressiveTailWindow(
  total: number,
  initial: number,
  step: number
): {
  startIndex: number;
  endIndex: number;
  hasMore: boolean;
  remaining: number;
  showMore: () => void;
} {
  const safeInitial = Math.max(1, initial);
  const safeStep = Math.max(1, step);
  const baseline = Math.min(total, safeInitial);
  const [visibleCount, setVisibleCount] = React.useState(() => baseline);

  React.useEffect(() => {
    setVisibleCount((prev) => {
      const clampedPrev = Math.min(total, Math.max(prev, 0));
      if (clampedPrev === 0) return baseline;
      return Math.max(baseline, clampedPrev);
    });
  }, [baseline, total]);

  const showMore = React.useCallback(() => {
    setVisibleCount((prev) => Math.min(total, prev + safeStep));
  }, [safeStep, total]);

  const hasMore = visibleCount < total;
  const remaining = hasMore ? total - visibleCount : 0;
  const startIndex = Math.max(0, total - visibleCount);
  return {
    startIndex,
    endIndex: total,
    hasMore,
    remaining,
    showMore,
  };
}

export function SegmentedMoreFooter({
  remaining,
  unitLabel,
  onShowMore,
}: {
  remaining: number;
  unitLabel: string;
  onShowMore: () => void;
}) {
  return (
    <div className={ACP_SEGMENTED_FOOTER_CLASS}>
      <span className={ACP_SEGMENTED_META_CLASS}>
        {remaining} more {unitLabel}
      </span>
      <ActionButton
        tone="secondary"
        size="sm"
        className="h-7 px-2.5 text-[11px] font-bold uppercase tracking-wider"
        onClick={onShowMore}
        aria-label={`Show ${remaining} more ${unitLabel}`}
      >
        Show more
      </ActionButton>
    </div>
  );
}

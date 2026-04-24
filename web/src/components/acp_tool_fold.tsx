import React from "react";
import { isToolCallLive } from "../conversation";

const TOOL_VISIBILITY_COLLAPSE_THRESHOLD = 0;
const ACP_SUBFOLD_CLASS = "acp-subfold mt-1";

export function deriveToolCallOpenState(
  prevOpen: boolean,
  wasLive: boolean,
  isLive: boolean
): boolean {
  if (isLive) return true;
  if (wasLive) return false;
  return prevOpen;
}

export function shouldCollapseToolFoldWhenOutOfView(
  isIntersecting: boolean,
  intersectionRatio?: number | null
): boolean {
  if (isIntersecting) return false;
  if (intersectionRatio != null && Number.isFinite(intersectionRatio)) {
    return intersectionRatio <= TOOL_VISIBILITY_COLLAPSE_THRESHOLD;
  }
  return true;
}

export function useAutoCollapseToolFoldWhenOutOfView({
  detailsRef,
  enabled,
  rootElement,
  onCollapse,
}: {
  detailsRef: React.RefObject<HTMLDetailsElement | null>;
  enabled: boolean;
  rootElement?: HTMLElement | null;
  onCollapse: () => void;
}): void {
  React.useEffect(() => {
    if (!enabled) return;
    if (typeof window === "undefined") return;
    if (typeof window.IntersectionObserver !== "function") return;
    const node = detailsRef.current;
    if (!node) return;
    const root = rootElement ?? node.closest(".acp-conversation");
    if (!(root instanceof HTMLElement)) return;
    const observer = new window.IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.target !== node) continue;
          if (
            shouldCollapseToolFoldWhenOutOfView(
              entry.isIntersecting,
              entry.intersectionRatio
            )
          ) {
            onCollapse();
          }
        }
      },
      {
        root,
        threshold: [0, 0.05],
      }
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [detailsRef, enabled, onCollapse]);
}

export function isToolCallEffectivelyLive(
  status?: string,
  runStatus?: string | null
): boolean {
  if (!isToolCallLive(status)) return false;
  if (!isRunTerminalStatus(runStatus)) return true;
  return false;
}

export function isRunTerminalStatus(status?: string | null): boolean {
  if (!status) return false;
  const normalized = status.trim().toLowerCase().replace(/[\s-]+/g, "_");
  return (
    normalized === "completed" ||
    normalized === "failed" ||
    normalized === "cancelled" ||
    normalized === "canceled" ||
    normalized === "stopped" ||
    normalized === "interrupted"
  );
}

type FoldSectionProps = {
  label: string;
  preview: string;
  defaultOpen: boolean;
  parentOpen?: boolean;
  lazyRender?: boolean;
  children: React.ReactNode;
};

export function FoldSection({
  label,
  preview,
  defaultOpen,
  parentOpen = true,
  lazyRender = false,
  children,
}: FoldSectionProps) {
  const [open, setOpen] = React.useState(defaultOpen);

  React.useEffect(() => {
    setOpen(defaultOpen);
  }, [defaultOpen]);

  React.useEffect(() => {
    if (!parentOpen) {
      setOpen(false);
    }
  }, [parentOpen]);

  const shouldRenderBody = !lazyRender || open || typeof window === "undefined";
  return (
    <div className={ACP_SUBFOLD_CLASS}>
      <details
        open={open}
        onToggle={(event) => {
          setOpen(event.currentTarget.open);
        }}
      >
        <summary className="flex cursor-pointer list-none items-center gap-2 rounded-md bg-slate-50/80 px-2.5 py-1.5 text-[11px] font-semibold tracking-[0.02em] text-notion-text-muted">
          <i
            className={`bi ${open ? "bi-chevron-down" : "bi-chevron-right"}`}
            aria-hidden="true"
          />
          <span>{label}</span>
          {preview && !open ? (
            <span className="ml-1 max-w-[240px] truncate font-normal normal-case opacity-65">
              · {preview}
            </span>
          ) : null}
        </summary>
        {shouldRenderBody ? (
          <div className="mt-2 border-l border-black/[0.06] pl-2.5">{children}</div>
        ) : null}
      </details>
    </div>
  );
}

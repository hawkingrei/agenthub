import { UnstyledButton } from "@mantine/core";
import React from "react";
import type { AcpDebugProps } from "./acp_debug";
import { loadAcpDebugModule } from "./acp_debug_loader";

function reportDebugSlotError(error: unknown) {
  if ("reportError" in globalThis && typeof globalThis.reportError === "function") {
    globalThis.reportError(error);
  }
}

export function AcpDebugSlot(props: AcpDebugProps) {
  const [DebugView, setDebugView] = React.useState<React.ComponentType<AcpDebugProps> | null>(null);
  const [loadFailed, setLoadFailed] = React.useState(false);
  const [retryNonce, setRetryNonce] = React.useState(0);

  React.useEffect(() => {
    let cancelled = false;
    setDebugView(null);
    setLoadFailed(false);
    void loadAcpDebugModule()
      .then((module) => {
        if (!cancelled) {
          setDebugView(() => module.AcpDebug);
        }
      })
      .catch((error) => {
        reportDebugSlotError(error);
        if (!cancelled) {
          setLoadFailed(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [retryNonce]);

  if (loadFailed) {
    return (
      <div className="flex items-center justify-between gap-3 px-3 py-2 text-sm text-notion-text-muted">
        <div>Inspect panel failed to load. Try reloading this view.</div>
        <UnstyledButton
          type="button"
          className="rounded-md border border-notion-border bg-white px-2 py-1 text-[12px] font-medium text-notion-text transition hover:bg-notion-hover"
          onClick={() => setRetryNonce((prev) => prev + 1)}
        >
          Retry
        </UnstyledButton>
      </div>
    );
  }

  if (DebugView == null) {
    return <div className="px-3 py-2 text-sm text-notion-text-muted">Loading debug...</div>;
  }

  return <DebugView {...props} />;
}

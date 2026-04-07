import { useState, useRef, useEffect } from "react";
import { 
  loadAgentsPanelWidthPreference, 
  persistAgentsPanelWidthPreference, 
  clampAgentsPanelWidth, 
  resolveAgentsPanelMaxWidth 
} from "./app_agents_helpers";
import { setupLayoutAnchorVarSync, setupRuntimeViewportVarSync } from "./app_viewport";

const AGENTS_DESKTOP_BREAKPOINT_PX = 1024;

export function useAppLayout(auth: unknown, error: string | null, agentsCollapsed: boolean) {
  const [agentsPanelWidth, setAgentsPanelWidth] = useState(() => loadAgentsPanelWidthPreference());
  const agentsPanelWidthRef = useRef(agentsPanelWidth);
  const appRootRef = useRef<HTMLDivElement | null>(null);
  const appHeaderRef = useRef<HTMLElement | null>(null);
  const workspaceRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (typeof window === "undefined" || typeof document === "undefined") return;
    return setupRuntimeViewportVarSync(window, document.documentElement.style);
  }, []);

  useEffect(() => {
    if (typeof window === "undefined" || typeof document === "undefined") return;
    return setupLayoutAnchorVarSync(
      window,
      document.documentElement.style,
      {
        appRoot: appRootRef.current,
        appHeader: appHeaderRef.current,
        workspace: workspaceRef.current,
      },
      typeof ResizeObserver === "undefined" ? undefined : ResizeObserver
    );
  }, [auth, error, agentsCollapsed]);

  useEffect(() => {
    agentsPanelWidthRef.current = agentsPanelWidth;
    persistAgentsPanelWidthPreference(agentsPanelWidth);
  }, [agentsPanelWidth]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const syncAgentsPanelWidth = () => {
      if (window.innerWidth <= AGENTS_DESKTOP_BREAKPOINT_PX) return;
      const workspaceWidth = workspaceRef.current?.getBoundingClientRect().width ?? 0;
      if (workspaceWidth <= 0) return;
      const nextMaxWidth = resolveAgentsPanelMaxWidth(workspaceWidth);
      setAgentsPanelWidth((current) => clampAgentsPanelWidth(current, nextMaxWidth));
    };
    syncAgentsPanelWidth();
    window.addEventListener("resize", syncAgentsPanelWidth);
    return () => window.removeEventListener("resize", syncAgentsPanelWidth);
  }, [agentsCollapsed]);

  return {
    agentsPanelWidth,
    setAgentsPanelWidth,
    appRootRef,
    appHeaderRef,
    workspaceRef,
  };
}

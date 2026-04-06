type RuntimeViewportSize = {
  height: number;
  width: number;
};

type RuntimeWindowLike = {
  innerHeight: number;
  innerWidth: number;
  visualViewport?: VisualViewport | null;
  addEventListener: (type: string, listener: () => void) => void;
  removeEventListener: (type: string, listener: () => void) => void;
  requestAnimationFrame?: (cb: (timestamp: number) => void) => number;
  cancelAnimationFrame?: (id: number) => void;
};

type StyleVarTarget = {
  setProperty: (name: string, value: string) => void;
};

type LayoutAnchorNodeLike = {
  getBoundingClientRect: () => { height: number; top: number };
};

type LayoutAnchorNodes = {
  appRoot: LayoutAnchorNodeLike | null;
  appHeader: LayoutAnchorNodeLike | null;
  workspace: LayoutAnchorNodeLike | null;
};

type ResizeObserverLike = {
  observe: (target: object) => void;
  disconnect: () => void;
};

type ResizeObserverCtorLike = new (callback: () => void) => ResizeObserverLike;

const MIN_RELIABLE_VIEWPORT_AXIS_PX = 48;

export function resolveRuntimeViewportAxis(
  axis: number | null | undefined,
  fallback: number
): number {
  const fallbackRounded = Math.max(1, Math.round(fallback));
  if (typeof axis !== "number" || !Number.isFinite(axis)) {
    return fallbackRounded;
  }
  const rounded = Math.max(1, Math.round(axis));
  const minReliable = Math.min(MIN_RELIABLE_VIEWPORT_AXIS_PX, fallbackRounded);
  if (rounded < minReliable) {
    return fallbackRounded;
  }
  return rounded;
}

export function resolveRuntimeViewportSize(
  viewport: Pick<VisualViewport, "height" | "width" | "offsetTop"> | null | undefined,
  innerHeight: number,
  innerWidth: number
): RuntimeViewportSize {
  const toSafeViewportDimension = (
    viewportValue: number | undefined,
    fallback: number
  ): number => {
    const safeFallback =
      typeof fallback === "number" && Number.isFinite(fallback) && fallback > 0
        ? fallback
        : 1;
    if (
      typeof viewportValue !== "number" ||
      !Number.isFinite(viewportValue) ||
      viewportValue <= 1
    ) {
      return safeFallback;
    }
    return viewportValue;
  };
  const viewportOffsetTop =
    typeof viewport?.offsetTop === "number" && Number.isFinite(viewport.offsetTop)
      ? Math.max(0, viewport.offsetTop)
      : 0;
  return {
    height: resolveRuntimeViewportAxis(
      toSafeViewportDimension(viewport?.height, innerHeight) + viewportOffsetTop,
      innerHeight
    ),
    width: resolveRuntimeViewportAxis(
      toSafeViewportDimension(viewport?.width, innerWidth),
      innerWidth
    ),
  };
}

export function shouldSyncRuntimeViewportSize(
  previous: RuntimeViewportSize | null,
  next: RuntimeViewportSize
): boolean {
  if (!previous) return true;
  return previous.height !== next.height || previous.width !== next.width;
}

export function resolveRuntimeKeyboardInset(
  viewport: Pick<VisualViewport, "height" | "offsetTop"> | null | undefined,
  innerHeight: number
): number {
  const safeInnerHeight =
    typeof innerHeight === "number" && Number.isFinite(innerHeight) && innerHeight > 0
      ? innerHeight
      : 1;
  const viewportHeight = resolveRuntimeViewportAxis(viewport?.height, safeInnerHeight);
  const viewportOffsetTop =
    typeof viewport?.offsetTop === "number" && Number.isFinite(viewport.offsetTop)
      ? Math.max(0, Math.round(viewport.offsetTop))
      : 0;
  const inset = safeInnerHeight - viewportHeight - viewportOffsetTop;
  if (!Number.isFinite(inset)) return 0;
  return inset > 0 ? inset : 0;
}

export function toNonNegativeRoundedPx(value: number | null | undefined): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  return Math.max(0, Math.round(value));
}

export function setupRuntimeViewportVarSync(
  runtimeWindow: RuntimeWindowLike,
  styleTarget: StyleVarTarget
): () => void {
  const viewport = runtimeWindow.visualViewport;
  let rafId: number | null = null;
  let previousSize: RuntimeViewportSize | null = null;
  let previousKeyboardInset: number | null = null;
  const syncViewportSizeNow = () => {
    const rawNextSize = resolveRuntimeViewportSize(
      viewport,
      runtimeWindow.innerHeight,
      runtimeWindow.innerWidth
    );
    const nextKeyboardInset = resolveRuntimeKeyboardInset(
      viewport,
      runtimeWindow.innerHeight
    );
    const nextSize =
      nextKeyboardInset > 0 &&
      previousSize &&
      rawNextSize.width === previousSize.width
        ? previousSize
        : rawNextSize;
    if (shouldSyncRuntimeViewportSize(previousSize, nextSize)) {
      previousSize = nextSize;
      styleTarget.setProperty("--agenthub-vh", `${nextSize.height}px`);
      styleTarget.setProperty("--agenthub-vw", `${nextSize.width}px`);
    }
    if (previousKeyboardInset === nextKeyboardInset) {
      return;
    }
    previousKeyboardInset = nextKeyboardInset;
    styleTarget.setProperty("--agenthub-keyboard-inset", `${nextKeyboardInset}px`);
  };
  const scheduleSyncViewportSize = () => {
    if (
      typeof runtimeWindow.requestAnimationFrame !== "function" ||
      typeof runtimeWindow.cancelAnimationFrame !== "function"
    ) {
      syncViewportSizeNow();
      return;
    }
    if (rafId != null) return;
    rafId = runtimeWindow.requestAnimationFrame(() => {
      rafId = null;
      syncViewportSizeNow();
    });
  };
  syncViewportSizeNow();
  runtimeWindow.addEventListener("resize", scheduleSyncViewportSize);
  runtimeWindow.addEventListener("orientationchange", scheduleSyncViewportSize);
  viewport?.addEventListener("resize", scheduleSyncViewportSize);
  viewport?.addEventListener("scroll", scheduleSyncViewportSize);
  return () => {
    if (
      rafId != null &&
      typeof runtimeWindow.cancelAnimationFrame === "function"
    ) {
      runtimeWindow.cancelAnimationFrame(rafId);
    }
    runtimeWindow.removeEventListener("resize", scheduleSyncViewportSize);
    runtimeWindow.removeEventListener("orientationchange", scheduleSyncViewportSize);
    viewport?.removeEventListener("resize", scheduleSyncViewportSize);
    viewport?.removeEventListener("scroll", scheduleSyncViewportSize);
  };
}

export function setupLayoutAnchorVarSync(
  runtimeWindow: RuntimeWindowLike,
  styleTarget: StyleVarTarget,
  nodes: LayoutAnchorNodes,
  resizeObserverCtor?: ResizeObserverCtorLike
): () => void {
  const syncLayoutAnchors = () => {
    const headerHeight = toNonNegativeRoundedPx(
      nodes.appHeader?.getBoundingClientRect().height
    );
    if (headerHeight != null) {
      styleTarget.setProperty("--agenthub-header-height", `${headerHeight}px`);
    }
    const workspaceTop = toNonNegativeRoundedPx(
      nodes.workspace?.getBoundingClientRect().top
    );
    if (workspaceTop != null) {
      styleTarget.setProperty("--agenthub-workspace-top", `${workspaceTop}px`);
    }
  };
  let rafId: number | null = null;
  const scheduleSync = () => {
    if (
      typeof runtimeWindow.requestAnimationFrame !== "function" ||
      typeof runtimeWindow.cancelAnimationFrame !== "function"
    ) {
      syncLayoutAnchors();
      return;
    }
    if (rafId != null) {
      runtimeWindow.cancelAnimationFrame(rafId);
    }
    rafId = runtimeWindow.requestAnimationFrame(() => {
      rafId = null;
      syncLayoutAnchors();
    });
  };

  syncLayoutAnchors();
  runtimeWindow.addEventListener("resize", scheduleSync);
  runtimeWindow.addEventListener("orientationchange", scheduleSync);
  const viewport = runtimeWindow.visualViewport;
  viewport?.addEventListener("resize", scheduleSync);
  viewport?.addEventListener("scroll", scheduleSync);
  let observer: ResizeObserverLike | null = null;
  if (resizeObserverCtor) {
    observer = new resizeObserverCtor(() => scheduleSync());
    if (nodes.appRoot) observer.observe(nodes.appRoot);
    if (nodes.appHeader) observer.observe(nodes.appHeader);
    if (nodes.workspace) observer.observe(nodes.workspace);
  }
  return () => {
    if (
      rafId != null &&
      typeof runtimeWindow.cancelAnimationFrame === "function"
    ) {
      runtimeWindow.cancelAnimationFrame(rafId);
    }
    runtimeWindow.removeEventListener("resize", scheduleSync);
    runtimeWindow.removeEventListener("orientationchange", scheduleSync);
    viewport?.removeEventListener("resize", scheduleSync);
    viewport?.removeEventListener("scroll", scheduleSync);
    observer?.disconnect();
  };
}

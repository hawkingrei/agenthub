import { cx } from "../ui/primitives";
import { WORKSPACE_PANEL_LOADING_CLASS } from "../ui/tailwind_classes";

type WorkspacePanelLoadingFallbackProps = {
  className?: string;
  title?: string;
  body?: string;
};

export function WorkspacePanelLoadingFallback({
  className = "",
  title = "Loading workspace panel...",
  body = "AgentHub is loading this workspace surface.",
}: WorkspacePanelLoadingFallbackProps) {
  return (
    <div
      className={cx(WORKSPACE_PANEL_LOADING_CLASS, className)}
      data-workspace-panel-loading="true"
      role="status"
      aria-live="polite"
      aria-busy="true"
    >
      <p className="font-medium text-notion-text">{title}</p>
      <p className="mt-1 text-ui-text-muted">{body}</p>
    </div>
  );
}

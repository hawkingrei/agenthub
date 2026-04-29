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
      className={`rounded-2xl border border-notion-border bg-white/88 px-4 py-6 text-sm text-ui-text-muted shadow-sm ${className}`.trim()}
      data-workspace-panel-loading="true"
    >
      <p className="font-medium text-notion-text">{title}</p>
      <p className="mt-1 text-ui-text-muted">{body}</p>
    </div>
  );
}

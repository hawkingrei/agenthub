import { WorkspacePanelLoadingFallback } from "../components/workspace_panel_loading_fallback";

export function RouteFallback({ label }: { label: string }) {
  return (
    <div className="mx-auto flex min-h-[40vh] w-full max-w-3xl items-center justify-center px-6 py-10">
      <WorkspacePanelLoadingFallback
        title={label}
        body="AgentHub is loading this workspace route."
        className="w-full max-w-md"
      />
    </div>
  );
}

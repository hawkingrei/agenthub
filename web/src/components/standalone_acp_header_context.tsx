import { readAgentHubMetaKind, type AcpView } from "../acp";
import type { AgentRecord } from "../api";
import { isToolCallLive } from "../conversation";

function formatRuntimeLabel(value: string): string {
  return value
    .trim()
    .replace(/[_-]+/g, " ")
    .replace(/\b\w/g, (character) => character.toUpperCase());
}

function StatusPill({
  icon,
  label,
  tone = "neutral",
}: {
  icon: string;
  label: string;
  tone?: "neutral" | "active" | "warning";
}) {
  const toneClass =
    tone === "active"
      ? "border-emerald-200 bg-emerald-50 text-emerald-700"
      : tone === "warning"
        ? "border-amber-200 bg-amber-50 text-amber-700"
        : "border-notion-border bg-white text-notion-text-muted";
  return (
    <span
      className={`inline-flex min-w-0 items-center gap-1 rounded-full border px-2 py-1 text-[11px] font-medium ${toneClass}`}
    >
      <i className={`bi ${icon} shrink-0 text-[10px]`} aria-hidden="true" />
      <span className="max-w-44 truncate">{label}</span>
    </span>
  );
}

export function StandaloneAcpHeaderContext({
  acpView,
  agent,
}: {
  acpView: AcpView;
  agent: AgentRecord | null;
}) {
  const subagents = acpView.toolCalls.filter(
    (call) => readAgentHubMetaKind(call.meta) === "codex_subagent"
  );
  const activeSubagents = subagents.filter((call) => isToolCallLive(call.status)).length;
  const configuredModel = acpView.configOptions.find((option) => option.id === "model");
  const configuredReasoning = acpView.configOptions.find(
    (option) => option.id === "reasoning_effort"
  );
  const model = configuredModel?.currentValueId ?? agent?.runtime_model ?? null;
  const reasoning = configuredReasoning?.currentValueId ?? agent?.thinking_level ?? null;
  const mode = acpView.currentMode ?? agent?.codex_acp_default_mode ?? null;
  const runStatus = acpView.runStatus?.status ?? agent?.status ?? "idle";
  const normalizedStatus = runStatus.trim().toLowerCase().replace(/[\s-]+/g, "_");
  const statusTone = ["running", "starting", "in_progress", "waiting_permission"].includes(
    normalizedStatus
  )
    ? "active"
    : normalizedStatus === "failed" || normalizedStatus === "error"
      ? "warning"
      : "neutral";

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-1.5" data-standalone-acp-context="true">
      <StatusPill icon="bi-activity" label={formatRuntimeLabel(runStatus)} tone={statusTone} />
      {model ? <StatusPill icon="bi-cpu" label={model} /> : null}
      {reasoning ? (
        <StatusPill icon="bi-lightbulb" label={formatRuntimeLabel(reasoning)} />
      ) : null}
      {mode ? (
        <StatusPill icon="bi-shield-check" label={formatRuntimeLabel(mode)} />
      ) : null}
      {subagents.length > 0 ? (
        <StatusPill
          icon="bi-diagram-3"
          label={`${activeSubagents} active / ${subagents.length} total`}
          tone={activeSubagents > 0 ? "active" : "neutral"}
        />
      ) : null}
    </div>
  );
}

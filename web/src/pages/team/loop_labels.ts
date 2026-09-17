import type {
  LoopActivation,
  LoopConfiguration,
  LoopRegistration,
} from "../../loop_types";

export const loopLabel = (value: string) => value.replace(/_/g, " ");
export const loopTime = (value: number | null) =>
  value == null ? "Not recorded" : new Date(value * 1000).toLocaleString();

const blockers: Record<string, string> = {
  team_loop_mode_required: "Choose durable execution for this Team first.",
  executor_guardian_unavailable:
    "This server needs a supported Linux executor.",
  remote_loop_unsupported: "Choose a local runtime for this member.",
  member_role_required: "Assign a coordinator or worker role.",
  role_prompt_invalid: "Review this member's role prompt.",
  unique_team_membership_required:
    "Use a member identity owned by this Team only.",
  local_acp_provider_required: "Choose a supported local ACP provider.",
  actor_control_unavailable:
    "The server's actor control service is unavailable.",
  workspace_unavailable: "Choose an available workspace.",
  worktree_repository_unavailable: "Choose an available worktree repository.",
  workspace_policy_invalid: "Review this member's workspace configuration.",
  provider_binary_unavailable:
    "Install or configure the selected provider executable.",
  runtime_profile_unsupported:
    "This provider does not support the selected model profile.",
  mem_binding_unavailable: "Configure the required memory scope binding.",
  mem_proxy_unavailable: "The required memory connection is unavailable.",
  required_capability_unavailable:
    "A required member capability is unavailable.",
  required_capabilities_invalid:
    "Review the required capabilities in this member's profile.",
  unfenced_executor_retained:
    "The previous execution still requires verified cleanup.",
  legacy_executor_must_stop:
    "Stop the existing process before enabling durable execution.",
  resume_capability_is_negotiated_before_entry:
    "Resume support is checked when the provider starts.",
};
export function loopPreflightMessages(
  configuration: LoopConfiguration,
): string[] {
  return [
    ...configuration.preflight.blockers,
    ...configuration.preflight.warnings,
  ].map((code) => blockers[code] ?? loopLabel(code));
}

export function loopOutcomeLabel(activation: LoopActivation): string {
  if (!activation.outcome) return "No outcome recorded";
  const { kind, wait_reason } = activation.outcome;
  if (kind === "completion_proposed")
    return "Completion proposed; task review remains separate";
  if (kind === "waiting" && wait_reason)
    return `Waiting for ${loopLabel(wait_reason)}`;
  return loopLabel(kind);
}

export function loopScheduleLabel(registration: LoopRegistration): string {
  const schedule = registration.input.schedule;
  if (schedule.kind === "task_status")
    return `Task condition: ${schedule.statuses.map(loopLabel).join(", ")}`;
  if (schedule.kind === "thread_reply") return "Waiting for a new thread reply";
  return `${schedule.kind === "recurring" ? "Recurring wake" : "Scheduled wake"}: ${loopTime(registration.next_due_at)}`;
}

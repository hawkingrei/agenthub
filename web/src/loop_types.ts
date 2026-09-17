// Release API projections. Process state and canonical task state remain separate.
export type LoopPolicyState = "disabled" | "enabled" | "suspended";
export type LoopSessionPolicy = "fresh" | "resume";
export type LoopActivationState =
  | "pending"
  | "starting"
  | "running"
  | "finalizing"
  | "finished"
  | "interrupted"
  | "canceled";
export type LoopOutcomeKind =
  | "progress"
  | "handoff"
  | "waiting"
  | "no_actionable_work"
  | "completion_proposed";
export type LoopWaitReason =
  | "input"
  | "dependency"
  | "permission"
  | "knowledge"
  | "external_event"
  | "due_time";

export type LoopLimits = {
  pending_per_actor: number;
  pending_per_team: number;
  sources_per_activation: number;
  lease_seconds: number;
  renewal_seconds: number;
  startup_attempts: number;
  retry_initial_seconds: number;
  retry_max_seconds: number;
  consecutive_no_progress: number;
  window_seconds: number;
  activations_per_actor: number;
  activations_per_team: number;
  standing_per_actor: number;
  standing_per_team: number;
};

export type LoopPolicy = {
  actor_id: string;
  team_id: string;
  state: LoopPolicyState;
  session_policy: LoopSessionPolicy;
  revision: number;
  mailbox_run_id: string | null;
  limits: LoopLimits;
  generation: number;
  no_progress_count: number;
  created_at: number;
  updated_at: number;
};

export type LoopConfiguration = {
  policy: LoopPolicy | null;
  preflight: {
    ready: boolean;
    provider_id: string | null;
    capabilities: string[];
    blockers: string[];
    warnings: string[];
  };
};

export type LoopConfigurationUpdate = {
  expected_revision: number;
  state: LoopPolicyState;
  session_policy: LoopSessionPolicy;
  limits: LoopLimits;
};

export type LoopActivation = {
  id: string;
  actor_id: string;
  team_id: string;
  state: LoopActivationState;
  due_at: number;
  next_admission_at: number;
  policy_revision: number;
  generation: number;
  attempt_count: number;
  mailbox_run_id: string | null;
  session_id: string | null;
  created_at: number;
  updated_at: number;
  finished_at: number | null;
  outcome: {
    kind: LoopOutcomeKind;
    wait_reason: LoopWaitReason | null;
    task_note_id: number | null;
    continuation: { due_at: number; task_id: string | null } | null;
  } | null;
  launch: {
    version: number;
    provider_id: string;
    configuration_digest: string;
    entry_prompt_version: string;
    session_policy: LoopSessionPolicy;
    workspace: string;
    model: string | null;
    thinking_level: string | null;
  } | null;
};

export type LoopTriggerReceipt = {
  trigger_id: string;
  activation_id: string;
  duplicate: boolean;
};

export type LoopHistoryPage = {
  activations: LoopActivation[];
  next_cursor: string | null;
};
export type LoopSourceSummary = {
  id: string;
  kind:
    | "operator"
    | "message"
    | "assignment"
    | "continuation"
    | "dependency"
    | "scheduled"
    | "member_request"
    | "app_event";
  references: {
    task_id: string | null;
    mailbox_message_id: number | null;
    conversation_message_id: number | null;
    thread_id: number | null;
    scheduling_actor_id: string | null;
    scheduling_activation_id: string | null;
    scheduling_user_id: string | null;
    app_id: string | null;
  };
  due_at: number | null;
  created_at: number;
  revoked: boolean;
};
export type LoopSourcePage = {
  sources: LoopSourceSummary[];
  next_cursor: string | null;
};
export type LoopEvent = {
  id: number;
  activation_id: string;
  kind: string;
  generation: number;
  trigger_id: string | null;
  reason: string | null;
  exit_reason: string | null;
  created_at: number;
};
export type LoopEventPage = { events: LoopEvent[]; next_cursor: number | null };
export type LoopToolSummary = {
  id: number;
  activation_id: string;
  generation: number;
  surface: "control_rpc" | "mcp_tool";
  tool_name: string;
  target_ref: string | null;
  operation_id: string | null;
  attempt_number: number | null;
  status:
    | "started"
    | "succeeded"
    | "failed"
    | "outcome_unknown"
    | "input_required"
    | "task_accepted";
  started_at: number;
  completed_at: number | null;
  duration_ms: number | null;
};
export type LoopToolPage = {
  tools: LoopToolSummary[];
  next_cursor: number | null;
};

export type LoopSchedule =
  | { kind: "due"; due_at: number }
  | { kind: "recurring"; first_at: number; interval_seconds: number }
  | {
      kind: "task_status";
      task_id: string;
      statuses: string[];
      repeat: boolean;
    }
  | {
      kind: "thread_reply";
      root_message_id: number;
      after_message_id: number;
      repeat: boolean;
    };
export type LoopRegistration = {
  id: string;
  input: {
    actor_id: string;
    team_id: string;
    source_key: string;
    schedule: LoopSchedule;
    work_task_id: string | null;
    references: LoopSourceSummary["references"];
  };
  state: "active" | "completed" | "revoked";
  next_due_at: number | null;
  observed_cursor: number;
  pending_cursor: number | null;
  pending_due_at: number | null;
  next_check_at: number;
  created_at: number;
  updated_at: number;
};
export type LoopRegistrationPage = {
  registrations: LoopRegistration[];
  next_cursor: string | null;
};

type LoopDurationMetrics = {
  samples: number;
  total_seconds: number;
  maximum_seconds: number | null;
  clock_regressions: number;
};
export type LoopMetrics = {
  observed_at: number;
  window_start: number;
  policy_state: LoopPolicyState | null;
  pending: {
    count: number;
    oldest_age_seconds: number | null;
    due_count: number;
    oldest_due_age_seconds: number | null;
  };
  admission_latency: LoopDurationMetrics;
  running_duration: LoopDurationMetrics;
  unsettled_run_count: number;
  oldest_unsettled_run_age_seconds: number | null;
  outcomes: Array<{ kind: LoopOutcomeKind; count: number }>;
  exits: Array<{ kind: string; count: number }>;
  exits_without_reason: number;
  startup_failures: number;
  retries: number;
  duplicates: {
    suppressed_total: number;
    sources_with_unknown_baseline: number;
  };
  progress: {
    finalized_activations: number;
    no_progress_activations: number;
    current_no_progress_streak: number | null;
  };
  waits: Array<{
    kind: "due" | "recurring" | "task_status" | "thread_reply";
    count: number;
    oldest_age_seconds: number | null;
  }>;
  current_wait: {
    reason: LoopWaitReason;
    recorded_at: number;
    age_seconds: number | null;
  } | null;
  mem: {
    latest: { kind: string; observed_at: number } | null;
    observations: Array<{ kind: string; count: number }>;
  };
};

import { AgentRecord, TeamRunSnapshotRecord, TeamRuntimeMemberRecord } from "../../api";
import { isAgentActiveStatus } from "../../agent_ws";

export const DEFAULT_TEAM_LEADER_PROMPT = [
  "You are the Team Leader in AgentHub.",
  "Role policy:",
  "- You are an architect/reviewer/efficiency owner. Do not implement feature code directly.",
  "- You own technical research and option comparison before delegation, including assumptions, trade-offs, and risks.",
  "- Your direct edits are limited to coordination artifacts (for example `AGENTS.md`) and review notes.",
  "- Treat `AGENTS.md` as index/routing artifact; keep detailed procedures in skill files.",
  "- Start from an empty workspace. First create or refresh `AGENTS.md` with run goals, task split, and decision log.",
  "- Leader usually works in an empty coordination workspace and normally does not need `.agenthubmemory/`.",
  "- For code review, either use GitHub CLI (`gh pr view` / `gh api`) or clone target repos for inspection.",
  "- You are responsible for direct human-facing planning communication. Do not redirect human questions to workers.",
  "- In shared human/team conversation, provide the first response by default.",
  "- When replying in team conversation, use @member_id for directed recipients; no @ means broadcast to all members.",
  "- If workers reply first due to urgent correction/new evidence, acknowledge and integrate their update quickly.",
  "Planning quality gate:",
  "- Decision Complete: every delegated step must be executable without extra implementation judgment calls.",
  "- Explore Before Asking: discoverable repo/system facts must be explored before asking human questions.",
  "- Two kinds of unknowns: discoverable facts are resolved by exploration; preference/tradeoff unknowns are asked explicitly.",
  "- Clearance checklist before delegation: objective, scope IN/OUT, approach, acceptance criteria, test strategy, and risk/rollback notes must be explicit.",
  "- If checklist is incomplete, continue exploration or ask focused clarification before dispatching worker steps.",
  "Coordination contract:",
  "- Use stable `spec.members[].member_id` as teammate routing keys in mailbox coordination.",
  "- Treat `spec.members[].description` as A2A identity card source for each member.",
  "- Keep `/api/agents/:id/.well-known/agent-card` description aligned with team member role identity.",
  "- Record discovery-card identity policy and update checkpoints in `AGENTS.md`.",
  "- Keep TODO/task statuses aligned with mailbox evidence and compact stale duplicate entries.",
  "- Load `team-task-lifecycle` whenever you are creating canonical Team tasks or advancing them through review.",
  "- Canonical Team task states are `open`, `in_progress`, `in_review`, `completed`, and `canceled`.",
  "- Successful worker execution should usually move a task to `in_review`; reserve `completed` for explicit review/acceptance.",
  "- If your own role description/prompt/skill profile drifts, send a `profile_patch_proposal` for your member record; use `target=\"team\"` for durable identity updates and `target=\"run\"` for temporary run-scoped adjustments.",
  "- Use `agent_time_trigger_set` / `agent_time_trigger_list` / `agent_time_trigger_cancel` for deferred follow-ups or timed reminders that should come back as ACP messages later.",
  "- `agent_loop` is an operator-controlled idle watchdog: it is disabled by default, enabled externally per agent, and only injects a configured ACP reminder after silence. Treat loop prompts as follow-up nudges, not as new human intent.",
  "- Do not assume you may enable or retune `agent_loop` yourself unless a human/operator explicitly asks for it.",
  "- Team ACP permission review path is operator-facing: worker-originated permission requests should route to leader first.",
  "- Use `acp_permission_review_respond` only when you are leader or the current worker explicitly delegated by leader for that permission request.",
  "- If leader-side agent review is unavailable or times out, the system will post a human-review request into `Channel` (`all`); human review remains valid and does not block normal team progress.",
  "- Finalization by mode: persistent teams stay running; one-shot/non-interactive runs request graceful worker shutdown before final response.",
  "Team workflow phases:",
  "1. Team formation",
  "2. Task analysis",
  "3. Role assignment",
  "4. Communication and collaboration",
  "5. Consensus formation",
  "6. Result integration",
  "Cold start policy:",
  "1. Before mailbox work, scan TODO sources (`TODO.md`).",
  "2. If unfinished planning tasks exist, resume them and publish a concise continuity update.",
  "3. If no planning tasks exist, treat as zero-start and align mission/scope with human actor.",
  "4. Refresh `AGENTS.md` sections: Agent Profile, Objective, Active Assignment, Active Skills, Role Skill Profile, Routing Contract, TODO And Context Pointers, Progress Log.",
  "Workflow:",
  "1. Read run input, perform targeted technical research, and produce a concise ordered execution plan.",
  "2. Delegate concrete, testable tasks to workers via actor mailbox.",
  "3. Run periodic sync checkpoints with workers and align assumptions/conflicts.",
  "4. Pull inbox regularly and acknowledge consumed messages.",
  "5. Merge worker outputs, review quality, resolve conflicts, and synthesize final deliverable.",
  "6. If blocked by missing facts, send clarification_request and move step to input_required.",
  "Structured payload contracts:",
  "- leader_task_assignment: {\"type\":\"leader_task_assignment\",\"task\":\"...\",\"acceptance\":\"...\",\"deadline\":\"...\"}",
  "- clarification_request: {\"type\":\"clarification_request\",\"question\":\"...\",\"choices\":[\"...\"],\"blocking_scope\":\"run|step\",\"context\":{}}",
  "- profile_patch_proposal: {\"type\":\"profile_patch_proposal\",\"target\":\"run|team\",\"prompt_append\":\"...\",\"description\":\"...\",\"skills_add\":[\"...\"]}",
].join("\n");

export const DEFAULT_TEAM_WORKER_PROMPT = [
  "You are a Worker in an AgentHub team.",
  "Your job is to execute assignments from the team leader and report results.",
  "Workspace policy:",
  "- Leader owns canonical Team task creation and task lifecycle management; you advance assigned tasks instead of inventing parallel task records.",
  "- In a concrete project workspace, keep durable worker memory under `.agenthubmemory/` (`TODO.md`, `journal/`, `note/`).",
  "- `.cache/context/` remains runtime continuity state and is not the durable worker TODO source.",
  "- Work in your own git worktree only. Never share the same worktree with other workers.",
  "- Create a random branch at start (for example `worker-<id>-<random>`), then implement on that branch.",
  "- Periodically sync from `main` (`fetch` + `rebase` or equivalent) and report conflicts immediately.",
  "- Keep your identity in `spec.members[].description`; this text is exposed by `/api/agents/:id/.well-known/agent-card`.",
  "- If your own description/prompt/skill profile is stale, send `profile_patch_proposal` yourself instead of waiting for a human/operator to edit the card manually.",
  "- Use `agent_time_trigger_set` / `agent_time_trigger_list` / `agent_time_trigger_cancel` for timed rechecks, reminders, or follow-ups that should wake you up later through ACP.",
  "- `agent_loop` is an operator-controlled idle watchdog: it is disabled by default, enabled externally per agent, and only injects a configured ACP reminder after silence. Treat loop prompts as follow-up nudges, not as new human intent.",
  "- Do not assume you may enable or retune `agent_loop` yourself unless a human/operator explicitly asks for it.",
  "- Team ACP permission requests that you trigger are routed to leader first.",
  "- Only leader or the current leader-delegated reviewer should use `acp_permission_review_respond`; other workers must wait for delegation or human review.",
  "- Do not review your own Team ACP permission request; wait for leader review or human review in `Channel` (`all`).",
  "- If leader-side agent review is unavailable or times out, the system may post a human-review request into `Channel` (`all`) without blocking your current run.",
  "- Load `team-task-lifecycle` whenever you need canonical Team task state guidance.",
  "- Treat `in_review` as the handoff state after implementation evidence is ready; do not treat worker completion as canonical Team task `completed`.",
  "- If cross-worker dependency exists, coordinate quickly with the related worker and send a summary back to leader.",
  "- Treat `AGENTS.md` as objective/phase/skill index; execute detailed procedures from skill files.",
  "- In shared human/team conversation, leader has first-response priority.",
  "- In team conversation replies, use @member_id when targeting specific recipients; no @ means broadcast to all members.",
  "- Do not speak before leader unless one of these is true: (a) leader statement is incorrect and needs correction, (b) you can add critical missing context, (c) you discovered new evidence, (d) you are explicitly mentioned.",
  "Team workflow phases:",
  "1. Team formation",
  "2. Task analysis",
  "3. Role assignment",
  "4. Communication and collaboration",
  "5. Consensus formation",
  "6. Result integration",
  "Cold start policy:",
  "1. Before mailbox work, scan TODO sources (`TODO.md`, and `.agenthubmemory/TODO.md` when this is a concrete project workspace).",
  "2. Continue unfinished worker TODO items first, then process inbox tasks.",
  "3. If no TODO and no inbox assignment, report idle state and request next task from leader.",
  "Workflow:",
  "1. Pull inbox and find the latest task from leader.",
  "2. Acknowledge messages after reading.",
  "3. Execute the task with minimal and auditable changes.",
  "4. Send result with evidence back to leader via actor mailbox.",
  "5. If blocked, send blocker details and a concrete next action.",
  "Use worker_status payload contract:",
  "{\"type\":\"worker_status\",\"status\":\"done|blocked\",\"result\":\"...\",\"evidence\":[\"...\"],\"next_action\":\"...\"}",
].join("\n");

export const DEFAULT_TEAM_LEADER_SKILLS = [
  "agenthub-actor-runtime",
  "team-leader-orchestrator",
];

export const DEFAULT_TEAM_WORKER_SKILLS = [
  "agenthub-actor-runtime",
  "team-worker-executor",
];

export const REQUIRED_TEAM_LEADER_SKILLS = [
  "agenthub-actor-runtime",
  "team-leader-orchestrator",
];

export const REQUIRED_TEAM_WORKER_SKILLS = [
  "agenthub-actor-runtime",
  "team-worker-executor",
];

const MANDATORY_TEAM_SKILLS = ["agenthub-actor-runtime"];
const OPTIONAL_TEAM_SKILLS = ["team-deliberation-rules"];

export const TEAM_SKILL_OPTIONS = [
  ...new Set([
    ...DEFAULT_TEAM_LEADER_SKILLS,
    ...DEFAULT_TEAM_WORKER_SKILLS,
    ...OPTIONAL_TEAM_SKILLS,
  ]),
];

export type WorkerDraft = {
  member_id: string;
  description: string;
  model: string;
  prompt: string;
  skills: string[];
  custom_skills: string;
};

export type TeamSpecMember = {
  member_id: string;
  role: string;
};

export type TeamMemberAgentStatus = {
  member_id: string;
  role: string;
  agent_name?: string;
  status: string;
  missing_agent: boolean;
};

export type TeamMemberAgentStatusSummary = {
  active: number;
  inactive: number;
  missing: number;
  total: number;
};

export type TeamMemberLiveState = {
  member_id: string;
  role: string;
  agent_name?: string;
  lifecycle_status: string;
  lifecycle_tone: "active" | "inactive" | "missing";
  run_status: string;
  step_status: string;
  pending_inbox_count: number | null;
  current_work: string;
};

export type TeamCreateDraftState = {
  leaderMemberId: string;
  leaderModel: string;
  leaderPrompt: string;
  leaderSkills: string[];
  leaderCustomSkills: string;
  workers: WorkerDraft[];
  useSpecOverride: boolean;
  newTeamSpec: string;
  teamForgeAgentIds: string[];
};

export function selectTeamForgeAgents(
  agents: AgentRecord[],
  teamForgeAgentIds: string[]
): AgentRecord[] {
  if (teamForgeAgentIds.length === 0) {
    return [];
  }
  const byId = new Map(agents.map((agent) => [agent.id, agent]));
  return teamForgeAgentIds
    .map((agentId) => byId.get(agentId))
    .filter((agent): agent is AgentRecord => Boolean(agent));
}

export function createInitialTeamDraftState(): TeamCreateDraftState {
  return {
    leaderMemberId: "",
    leaderModel: "",
    leaderPrompt: DEFAULT_TEAM_LEADER_PROMPT,
    leaderSkills: [...DEFAULT_TEAM_LEADER_SKILLS],
    leaderCustomSkills: "",
    workers: [],
    useSpecOverride: false,
    newTeamSpec: "{}",
    teamForgeAgentIds: [],
  };
}

function parseCsvList(raw: string): string[] {
  return raw
    .split(",")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function ensureMandatorySkills(skills: string[], requiredSkills: string[]): string[] {
  const deduped = [...new Set(skills.map((item) => item.trim()).filter(Boolean))];
  const required = [...new Set(requiredSkills.map((item) => item.trim()).filter(Boolean))];
  const mandatory = required.filter((item) => !deduped.includes(item));
  if (mandatory.length === 0) {
    return deduped;
  }
  return [...mandatory, ...deduped];
}

export function normalizeSkillSelection(
  selected: string[],
  customRaw: string,
  fallback: string[],
  requiredSkills: string[] = MANDATORY_TEAM_SKILLS
): string[] {
  const allowed = new Set(TEAM_SKILL_OPTIONS);
  const selectedSkills = [...new Set(selected.map((item) => item.trim()).filter(Boolean))].filter(
    (item) => allowed.has(item)
  );
  const customSkills = parseCsvList(customRaw);
  const merged = [...new Set([...selectedSkills, ...customSkills])];
  if (merged.length > 0) {
    return ensureMandatorySkills(merged, requiredSkills);
  }
  return ensureMandatorySkills(fallback, requiredSkills);
}

export function toggleSkillSelection(
  selected: string[],
  skill: string,
  requiredSkills: string[] = MANDATORY_TEAM_SKILLS
): string[] {
  const normalized = skill.trim();
  if (!normalized || !TEAM_SKILL_OPTIONS.includes(normalized)) {
    return selected;
  }
  const required = new Set(
    requiredSkills.map((item) => item.trim()).filter((item) => item.length > 0)
  );
  const normalizedSelected = ensureMandatorySkills(selected, [...required]);
  if (normalizedSelected.includes(normalized)) {
    if (required.has(normalized)) {
      return normalizedSelected;
    }
    return normalizedSelected.filter((item) => item !== normalized);
  }
  return ensureMandatorySkills([...normalizedSelected, normalized], [...required]);
}

function asObjectRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function normalizeLifecycleStatus(status: string | null | undefined): string {
  const normalized = status?.trim().toLowerCase();
  return normalized && normalized.length > 0 ? normalized : "unknown";
}

export function parseTeamSpecMembers(spec: unknown): TeamSpecMember[] {
  const specRecord = asObjectRecord(spec);
  if (!specRecord) return [];
  const membersRaw = specRecord.members;
  if (!Array.isArray(membersRaw)) return [];

  const deduped = new Map<string, TeamSpecMember>();
  for (const item of membersRaw) {
    const memberRecord = asObjectRecord(item);
    if (!memberRecord) continue;
    const memberIdRaw = memberRecord.member_id;
    if (typeof memberIdRaw !== "string") continue;
    const memberId = memberIdRaw.trim();
    if (!memberId) continue;
    if (deduped.has(memberId)) continue;
    const roleRaw = memberRecord.role;
    const role =
      typeof roleRaw === "string" && roleRaw.trim().length > 0
        ? roleRaw.trim()
        : "member";
    deduped.set(memberId, { member_id: memberId, role });
  }
  return [...deduped.values()];
}

export function resolveTeamMemberAgentStatuses(
  spec: unknown,
  agents: AgentRecord[],
  fallbackAgentsById?: Record<string, AgentRecord | null>,
  runtimeMembers?: TeamRuntimeMemberRecord[] | null
): TeamMemberAgentStatus[] {
  const byId = new Map(agents.map((agent) => [agent.id, agent]));
  const runtimeById = new Map(
    (runtimeMembers ?? []).map((member) => [member.member_id, member])
  );
  if (fallbackAgentsById) {
    for (const [memberId, agent] of Object.entries(fallbackAgentsById)) {
      if (agent && !byId.has(memberId)) {
        byId.set(memberId, agent);
      }
    }
  }
  return parseTeamSpecMembers(spec).map((member) => {
    const agent = byId.get(member.member_id);
    const runtimeMember = runtimeById.get(member.member_id);
    const runtimeStatus =
      runtimeMember?.session_status?.trim() || runtimeMember?.agent_status?.trim() || "";
    if (!agent && !runtimeMember) {
      return {
        member_id: member.member_id,
        role: member.role,
        status: "missing",
        missing_agent: true,
      };
    }
    return {
      member_id: member.member_id,
      role: member.role,
      agent_name: agent?.name ?? runtimeMember?.display_name,
      status: normalizeLifecycleStatus(runtimeStatus || agent?.status || "missing"),
      missing_agent: false,
    };
  });
}

export function summarizeTeamMemberAgentStatuses(
  members: TeamMemberAgentStatus[]
): TeamMemberAgentStatusSummary {
  let active = 0;
  let missing = 0;
  for (const member of members) {
    if (member.missing_agent) {
      missing += 1;
      continue;
    }
    if (isAgentActiveStatus(member.status)) {
      active += 1;
    }
  }
  const total = members.length;
  const inactive = total - active - missing;
  return {
    active,
    inactive,
    missing,
    total,
  };
}

function resolveTeamRoleWeight(role: string): number {
  const normalized = role.trim().toLowerCase();
  if (normalized === "leader") return 0;
  if (normalized === "worker") return 1;
  return 2;
}

function toCompactWorkPreview(value: unknown, maxLength = 72): string {
  if (value == null) {
    return "";
  }
  const raw =
    typeof value === "string"
      ? value
      : (() => {
          try {
            return JSON.stringify(value);
          } catch {
            return String(value);
          }
        })();
  const normalized = raw.replace(/\s+/g, " ").trim();
  if (!normalized) {
    return "";
  }
  if (normalized.length <= maxLength) {
    return normalized;
  }
  return `${normalized.slice(0, maxLength - 3)}...`;
}

function resolveTeamMemberCurrentWork(
  snapshotMember?: TeamRunSnapshotRecord["members"][number]
): string {
  if (!snapshotMember) {
    return "No active run context.";
  }
  const step = snapshotMember.latest_step;
  if (!step) {
    return `run_status=${snapshotMember.status}`;
  }
  const stepLabel = step.step_key || step.id;
  const payloadPreview =
    toCompactWorkPreview(step.input) ||
    toCompactWorkPreview(step.output) ||
    toCompactWorkPreview(step.error_text);
  if (!payloadPreview) {
    return `${stepLabel} (${step.status})`;
  }
  return `${stepLabel}: ${payloadPreview}`;
}

export function resolveTeamMemberLifecycleTone(
  member: TeamMemberAgentStatus
): "active" | "inactive" | "missing" {
  if (member.missing_agent) {
    return "missing";
  }
  return isAgentActiveStatus(member.status) ? "active" : "inactive";
}

export function buildTeamMemberLiveStates(
  members: TeamMemberAgentStatus[],
  snapshotMembers?: TeamRunSnapshotRecord["members"]
): TeamMemberLiveState[] {
  const snapshotByMemberId = new Map(
    (snapshotMembers ?? []).map((member) => [member.member_id, member])
  );
  return [...members]
    .map((member) => {
      const snapshotMember = snapshotByMemberId.get(member.member_id);
      return {
        member_id: member.member_id,
        role: member.role,
        agent_name: member.agent_name,
        lifecycle_status: member.status,
        lifecycle_tone: resolveTeamMemberLifecycleTone(member),
        run_status: snapshotMember?.status ?? "-",
        step_status: snapshotMember?.latest_step?.status ?? "-",
        pending_inbox_count: snapshotMember?.pending_inbox_count ?? null,
        current_work: resolveTeamMemberCurrentWork(snapshotMember),
      };
    })
    .sort((a, b) => {
      const roleGap = resolveTeamRoleWeight(a.role) - resolveTeamRoleWeight(b.role);
      if (roleGap !== 0) return roleGap;
      return a.member_id.localeCompare(b.member_id);
    });
}

export function buildDefaultWorkerDraft(memberId: string): WorkerDraft {
  return {
    member_id: memberId,
    description: "",
    model: "",
    prompt: DEFAULT_TEAM_WORKER_PROMPT,
    skills: [...DEFAULT_TEAM_WORKER_SKILLS],
    custom_skills: "",
  };
}

export function assignCreatedWorkerToDraft(
  workers: WorkerDraft[],
  createdMemberId: string
): WorkerDraft[] {
  const memberId = createdMemberId.trim();
  if (!memberId) {
    return workers;
  }
  if (workers.some((worker) => worker.member_id.trim() === memberId)) {
    return workers;
  }
  const firstUnassigned = workers.findIndex(
    (worker) => worker.member_id.trim().length === 0
  );
  if (firstUnassigned >= 0) {
    return workers.map((worker, index) =>
      index === firstUnassigned ? { ...worker, member_id: memberId } : worker
    );
  }
  return [...workers, buildDefaultWorkerDraft(memberId)];
}

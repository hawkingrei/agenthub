import {
  getLocalStorageItemSafe,
  removeLocalStorageItemSafe,
  setLocalStorageItemSafe,
} from "../../storage/safe_storage";
import {
  createInitialTeamDraftState,
  type WorkerDraft,
} from "./member_helpers";
import { clampCreateTeamStage } from "./create_helpers";
import type { CreateTeamStage, TeamCreateState } from "./state";

export type TeamCreateEntryMode = "wizard" | "manual_spec";
export type TeamCreateDraftLoadResult = {
  draft: Partial<TeamCreateState> | null;
  error: string | null;
};

const TEAM_CREATE_DRAFT_STORAGE_KEY = "agenthub_team_create_draft_v1";
const TEAM_CREATE_DRAFT_SCHEMA_VERSION = 1;

type PersistedTeamCreateDraft = {
  schema_version: number;
  status: "creating";
  entry_mode: TeamCreateEntryMode;
  updated_at: number;
  draft: {
    newTeamName: string;
    newTeamDescription: string;
    newTeamSpec: string;
    createTeamStage: number;
    coordinatorMemberId: string;
    coordinatorModel: string;
    coordinatorPrompt: string;
    coordinatorSkills: string[];
    coordinatorCustomSkills: string;
    workers: WorkerDraft[];
    teamForgeAgentIds: string[];
  };
};

function draftField(
  draft: Record<string, unknown>,
  primaryKey: string,
  legacyKeys: string[] = []
): unknown {
  if (primaryKey in draft) {
    return draft[primaryKey];
  }
  for (const legacyKey of legacyKeys) {
    if (legacyKey in draft) {
      return draft[legacyKey];
    }
  }
  return undefined;
}

function asString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map((item) => asString(item).trim()).filter((item) => item.length > 0);
}

function normalizeWorkerDrafts(value: unknown): WorkerDraft[] {
  if (!Array.isArray(value)) return [];
  return value
    .map((item): WorkerDraft | null => {
      if (!item || typeof item !== "object" || Array.isArray(item)) {
        return null;
      }
      const candidate = item as Record<string, unknown>;
      return {
        member_id: asString(candidate.member_id),
        description: asString(candidate.description),
        model: asString(candidate.model),
        prompt: asString(candidate.prompt),
        skills: asStringArray(candidate.skills),
        custom_skills: asString(candidate.custom_skills),
      };
    })
    .filter((item): item is WorkerDraft => Boolean(item));
}

function normalizeCreateStage(value: unknown): CreateTeamStage {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return 0;
  }
  if (value <= 0) return 0;
  if (value >= 3) return 3;
  return Math.floor(value) as CreateTeamStage;
}

function resolveEntryMode(value: unknown): TeamCreateEntryMode {
  return value === "manual_spec" ? "manual_spec" : "wizard";
}

type ParsePersistedDraftResult = {
  value: PersistedTeamCreateDraft | null;
  error: string | null;
};

function parsePersistedDraft(raw: string): ParsePersistedDraftResult {
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {
        value: null,
        error: "Team create draft is invalid and has been ignored.",
      };
    }
    const candidate = parsed as Record<string, unknown>;
    if (candidate.schema_version !== TEAM_CREATE_DRAFT_SCHEMA_VERSION) {
      return { value: null, error: null };
    }
    if (candidate.status !== "creating") {
      return {
        value: null,
        error: "Team create draft has unknown status and has been ignored.",
      };
    }
    const entryMode = resolveEntryMode(candidate.entry_mode);
    const draft =
      candidate.draft && typeof candidate.draft === "object" && !Array.isArray(candidate.draft)
        ? (candidate.draft as Record<string, unknown>)
        : null;
    if (!draft) {
      return {
        value: null,
        error: "Team create draft is incomplete and has been ignored.",
      };
    }
    return {
      value: {
        schema_version: TEAM_CREATE_DRAFT_SCHEMA_VERSION,
        status: "creating",
        entry_mode: entryMode,
        updated_at:
          typeof candidate.updated_at === "number" && Number.isFinite(candidate.updated_at)
            ? Math.floor(candidate.updated_at)
            : Date.now(),
        draft: {
          newTeamName: asString(draftField(draft, "newTeamName")),
          newTeamDescription: asString(draftField(draft, "newTeamDescription")),
          newTeamSpec: asString(draftField(draft, "newTeamSpec")),
          createTeamStage: normalizeCreateStage(draftField(draft, "createTeamStage")),
          coordinatorMemberId: asString(
            draftField(draft, "coordinatorMemberId", ["leaderMemberId"])
          ),
          coordinatorModel: asString(
            draftField(draft, "coordinatorModel", ["leaderModel"])
          ),
          coordinatorPrompt: asString(
            draftField(draft, "coordinatorPrompt", ["leaderPrompt"])
          ),
          coordinatorSkills: asStringArray(
            draftField(draft, "coordinatorSkills", ["leaderSkills"])
          ),
          coordinatorCustomSkills: asString(
            draftField(draft, "coordinatorCustomSkills", ["leaderCustomSkills"])
          ),
          workers: normalizeWorkerDrafts(draftField(draft, "workers")),
          teamForgeAgentIds: asStringArray(draftField(draft, "teamForgeAgentIds")),
        },
      },
      error: null,
    };
  } catch {
    return {
      value: null,
      error: "Team create draft is corrupted and has been reset.",
    };
  }
}

export function loadTeamCreateDraft(
  entryMode: TeamCreateEntryMode
): TeamCreateDraftLoadResult {
  const raw = getLocalStorageItemSafe(TEAM_CREATE_DRAFT_STORAGE_KEY);
  if (!raw) return { draft: null, error: null };
  const parsed = parsePersistedDraft(raw);
  if (parsed.error) {
    clearTeamCreateDraft();
    return { draft: null, error: parsed.error };
  }
  if (!parsed.value || parsed.value.entry_mode !== entryMode) {
    return { draft: null, error: null };
  }
  const initial = createInitialTeamDraftState();
  return {
    draft: {
      newTeamName: parsed.value.draft.newTeamName,
      newTeamDescription: parsed.value.draft.newTeamDescription,
      useSpecOverride: parsed.value.entry_mode === "manual_spec",
      newTeamSpec: parsed.value.draft.newTeamSpec || initial.newTeamSpec,
      createTeamStage: clampCreateTeamStage(parsed.value.draft.createTeamStage),
      coordinatorMemberId: parsed.value.draft.coordinatorMemberId,
      coordinatorModel: parsed.value.draft.coordinatorModel,
      coordinatorPrompt: parsed.value.draft.coordinatorPrompt || initial.coordinatorPrompt,
      coordinatorSkills:
        parsed.value.draft.coordinatorSkills.length > 0
          ? parsed.value.draft.coordinatorSkills
          : [...initial.coordinatorSkills],
      coordinatorCustomSkills: parsed.value.draft.coordinatorCustomSkills,
      workers: parsed.value.draft.workers,
      teamForgeAgentIds: parsed.value.draft.teamForgeAgentIds,
    },
    error: null,
  };
}

export function persistTeamCreateDraft(state: TeamCreateState): string | null {
  if (!state.showCreateTeamModal) return null;
  const entryMode: TeamCreateEntryMode = state.useSpecOverride ? "manual_spec" : "wizard";
  const payload: PersistedTeamCreateDraft = {
    schema_version: TEAM_CREATE_DRAFT_SCHEMA_VERSION,
    status: "creating",
    entry_mode: entryMode,
    updated_at: Date.now(),
    draft: {
      newTeamName: state.newTeamName,
      newTeamDescription: state.newTeamDescription,
      newTeamSpec: state.newTeamSpec,
      createTeamStage: state.createTeamStage,
      coordinatorMemberId: state.coordinatorMemberId,
      coordinatorModel: state.coordinatorModel,
      coordinatorPrompt: state.coordinatorPrompt,
      coordinatorSkills: state.coordinatorSkills,
      coordinatorCustomSkills: state.coordinatorCustomSkills,
      workers: state.workers,
      teamForgeAgentIds: state.teamForgeAgentIds,
    },
  };
  const saved = setLocalStorageItemSafe(
    TEAM_CREATE_DRAFT_STORAGE_KEY,
    JSON.stringify(payload)
  );
  if (saved) {
    return null;
  }
  return "Failed to save Team create draft locally.";
}

export function clearTeamCreateDraft(): void {
  removeLocalStorageItemSafe(TEAM_CREATE_DRAFT_STORAGE_KEY);
}

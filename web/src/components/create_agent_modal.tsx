import React from "react";
import {
  DEFAULT_AGENT_PRESET_ID,
  formatAgentCommand,
  getAgentPreset,
  isAgentPresetId,
  listAgentPresets,
  type AgentPresetId,
} from "../agent_presets";

type CreateAgentModalProps = {
  agentName: string;
  setAgentName: (value: string) => void;
  agentWorkdir: string;
  setAgentWorkdir: (value: string) => void;
  agentPresetId: AgentPresetId;
  setAgentPresetId: (value: AgentPresetId) => void;
  worktreeMode: "use_existing" | "create_worktree" | "reuse_worktree";
  setWorktreeMode: (
    value: "use_existing" | "create_worktree" | "reuse_worktree"
  ) => void;
  worktreeRepo: string;
  setWorktreeRepo: (value: string) => void;
  worktreeRef: string;
  setWorktreeRef: (value: string) => void;
  codeMode: boolean;
  setCodeMode: (value: boolean) => void;
  worktreeError: string | null;
  onCreateAgent: () => void;
  onClose: () => void;
};

export function CreateAgentModal({
  agentName,
  setAgentName,
  agentWorkdir,
  setAgentWorkdir,
  agentPresetId,
  setAgentPresetId,
  worktreeMode,
  setWorktreeMode,
  worktreeRepo,
  setWorktreeRepo,
  worktreeRef,
  setWorktreeRef,
  codeMode,
  setCodeMode,
  worktreeError,
  onCreateAgent,
  onClose,
}: CreateAgentModalProps) {
  const presets = listAgentPresets();
  const preset = getAgentPreset(agentPresetId);
  const commandSummary = formatAgentCommand(preset);
  return (
    <div className="modal-backdrop">
      <div className="modal">
        <div className="modal-head">
          <h3>Create Agent</h3>
          <button className="ghost" onClick={onClose}>
            Close
          </button>
        </div>
        <div className="modal-body">
          <div className="form-grid">
            <input
              placeholder="Agent name"
              value={agentName}
              onChange={(e) => setAgentName(e.target.value)}
            />
            <input
              placeholder="Workdir"
              value={agentWorkdir}
              onChange={(e) => setAgentWorkdir(e.target.value)}
            />
            <select
              value={worktreeMode}
              onChange={(e) =>
                setWorktreeMode(
                  e.target.value as
                    | "use_existing"
                    | "create_worktree"
                    | "reuse_worktree"
                )
              }
            >
              <option value="use_existing">Use existing workdir</option>
              <option value="create_worktree">Create git worktree</option>
              <option value="reuse_worktree">Reuse git worktree</option>
            </select>
            {(worktreeMode === "create_worktree" ||
              worktreeMode === "reuse_worktree") && (
              <input
                placeholder="Worktree repo path"
                value={worktreeRepo}
                onChange={(e) => setWorktreeRepo(e.target.value)}
              />
            )}
            {worktreeMode === "create_worktree" && (
              <input
                placeholder="Worktree ref (branch or commit)"
                value={worktreeRef}
                onChange={(e) => setWorktreeRef(e.target.value)}
              />
            )}
            <select
              value={agentPresetId}
              onChange={(e) => {
                const next = e.target.value;
                if (isAgentPresetId(next)) {
                  setAgentPresetId(next);
                } else {
                  setAgentPresetId(DEFAULT_AGENT_PRESET_ID);
                }
              }}
            >
              {presets.map((preset) => (
                <option key={preset.id} value={preset.id}>
                  {preset.label}
                </option>
              ))}
            </select>
          </div>
          {commandSummary && (
            <div className="notice">Command: {commandSummary}</div>
          )}
          <div className="checkbox-row">
            <label className="checkbox">
              <input
                type="checkbox"
                checked={codeMode}
                onChange={(e) => setCodeMode(e.target.checked)}
              />
              <span>Code mode</span>
            </label>
          </div>
          {worktreeError && (
            <div className="worktree-error">
              <h4>Worktree Setup Failed</h4>
              <p>{worktreeError}</p>
              <ul>
                <li>Check Safe Paths for the workdir and repo path.</li>
                <li>Ensure the workdir is empty when creating a worktree.</li>
                <li>Verify the git repo exists and the ref is valid.</li>
              </ul>
            </div>
          )}
        </div>
        <div className="modal-actions">
          <button onClick={onCreateAgent}>Create Agent</button>
          <button className="ghost" onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

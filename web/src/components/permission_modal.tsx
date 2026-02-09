import React from "react";
import { AcpPermissionRecord } from "../api";

type PermissionModalProps = {
  permissions: AcpPermissionRecord[];
  permissionBusy: string | null;
  onRespond: (agentId: string, permissionId: string, optionId?: string) => void;
};

export function PermissionModal({
  permissions,
  permissionBusy,
  onRespond,
}: PermissionModalProps) {
  return (
    <div className="modal-backdrop">
      <div className="modal">
        <div className="modal-head">
          <h3>Permission Requests</h3>
          <span className="badge">{permissions.length}</span>
        </div>
        <div className="modal-body">
          {permissions.map((perm) => {
            const toolCall = perm.tool_call as {
              title?: string;
              tool_call_id?: string;
            } | null;
            const title =
              toolCall?.title ?? perm.tool_call_id ?? "Permission Request";
            return (
              <div key={perm.id} className="acp-permission">
                <div className="head">
                  <div className="title">{title}</div>
                  <div className="meta">{perm.status}</div>
                </div>
                <div className="options">
                  {perm.options.map((opt, idx) => {
                    const optionId = opt.option_id ?? opt.optionId ?? "";
                    return (
                      <button
                        key={optionId || `${perm.id}-${idx}`}
                        disabled={permissionBusy === perm.id || !optionId}
                        onClick={() =>
                          onRespond(perm.agent_id, perm.id, optionId)
                        }
                      >
                        {opt.name}
                      </button>
                    );
                  })}
                  <button
                    disabled={permissionBusy === perm.id}
                    onClick={() => onRespond(perm.agent_id, perm.id)}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

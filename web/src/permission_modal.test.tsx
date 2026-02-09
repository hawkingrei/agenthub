import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PermissionModal } from "./components/permission_modal";
import { AcpPermissionRecord } from "./api";

const basePermission: AcpPermissionRecord = {
  id: "perm-1",
  agent_id: "agent-1",
  session_id: "session-1",
  options: [],
  status: "pending",
  created_at: 1,
};

const renderModal = (permissions: AcpPermissionRecord[]) =>
  renderToStaticMarkup(
    <PermissionModal
      permissions={permissions}
      permissionBusy={null}
      onRespond={() => {}}
    />
  );

const hasEnabledButton = (html: string, label: string) =>
  new RegExp(`<button(?![^>]*disabled)[^>]*>${label}</button>`).test(html);

const hasDisabledButton = (html: string, label: string) =>
  new RegExp(`<button[^>]*disabled[^>]*>${label}</button>`).test(html);

describe("PermissionModal option id handling", () => {
  it("renders enabled buttons when option_id is present", () => {
    const html = renderModal([
      {
        ...basePermission,
        options: [{ option_id: "allow_once", name: "Allow once", kind: "allow_once" }],
      },
    ]);
    expect(hasEnabledButton(html, "Allow once")).toBe(true);
  });

  it("disables options with empty option IDs", () => {
    const html = renderModal([
      {
        ...basePermission,
        options: [{ option_id: "", name: "Allow once", kind: "allow_once" }],
      },
    ]);
    expect(hasDisabledButton(html, "Allow once")).toBe(true);
  });
});

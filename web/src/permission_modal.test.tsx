import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";
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
    <MantineProvider>
      <PermissionModal
        permissions={permissions}
        permissionBusy={null}
        onRespond={() => {}}
        withinPortal={false}
      />
    </MantineProvider>
  );

const findButtonHtml = (html: string, label: string) => {
  const labelIndex = html.indexOf(label);
  if (labelIndex < 0) return null;
  const start = html.lastIndexOf("<button", labelIndex);
  if (start < 0) return null;
  const end = html.indexOf("</button>", labelIndex);
  if (end < 0) return null;
  return html.slice(start, end + "</button>".length);
};

const isDisabledButton = (buttonHtml: string) =>
  /(?:\s|^)disabled(?:\s|=|>)/.test(buttonHtml) ||
  /aria-disabled=["']true["']/.test(buttonHtml) ||
  /data-disabled=["']true["']/.test(buttonHtml);

const hasEnabledButton = (html: string, label: string) => {
  const button = findButtonHtml(html, label);
  return button !== null && !isDisabledButton(button);
};

const hasDisabledButton = (html: string, label: string) => {
  const button = findButtonHtml(html, label);
  return button !== null && isDisabledButton(button);
};

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

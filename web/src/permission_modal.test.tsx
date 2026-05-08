// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";
import { PermissionModal } from "./components/permission_modal";
import { AcpPermissionRecord } from "./api";
import { installReactDomTestGlobals, required } from "./test_utils/react_test_helpers";

installReactDomTestGlobals();

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

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

const renderInteractiveModal = (
  permissions: AcpPermissionRecord[],
  onRespond: (agentId: string, permissionId: string, optionId?: string) => void
) => {
  act(() => {
    root.render(
      <MantineProvider env="test">
        <PermissionModal
          permissions={permissions}
          permissionBusy={null}
          onRespond={onRespond}
          withinPortal={false}
        />
      </MantineProvider>
    );
  });
};

describe("PermissionModal option id handling", () => {
  it("renders Codex-aligned option labels when option_id is present", () => {
    const html = renderModal([
      {
        ...basePermission,
        options: [
          { option_id: "allow_once", name: "Allow once", kind: "allow_once" },
          { option_id: "allow_always", name: "Always allow", kind: "allow_always" },
          { option_id: "deny", name: "Reject", kind: "reject_once" },
        ],
      },
    ]);
    expect(hasEnabledButton(html, "Allow")).toBe(true);
    expect(hasEnabledButton(html, "ask again")).toBe(true);
    expect(hasEnabledButton(html, "Deny")).toBe(true);
  });

  it("disables options with empty option IDs", () => {
    const html = renderModal([
      {
        ...basePermission,
        options: [{ option_id: "", name: "Allow once", kind: "allow_once" }],
      },
    ]);
    expect(hasDisabledButton(html, "Allow")).toBe(true);
  });

  it("renders Deny fallback only when ACP does not provide a reject option", () => {
    const withoutReject = renderModal([
      {
        ...basePermission,
        options: [{ option_id: "allow_once", name: "Allow once", kind: "allow_once" }],
      },
    ]);
    expect(hasEnabledButton(withoutReject, "Deny")).toBe(true);

    const withReject = renderModal([
      {
        ...basePermission,
        options: [{ option_id: "reject", name: "Reject", kind: "reject_once" }],
      },
    ]);
    expect(findButtonHtml(withReject, "Deny")).not.toBeNull();
    expect(withReject.match(/Deny/g)).toHaveLength(1);
  });

  it("submits explicit options and fallback deny decisions", () => {
    const onRespond = vi.fn();
    renderInteractiveModal(
      [
        {
          ...basePermission,
          options: [{ option_id: "allow_once", name: "Allow once", kind: "allow_once" }],
        },
      ],
      onRespond
    );

    act(() => {
      required(container.querySelector("button"), "allow button missing").click();
    });
    expect(onRespond).toHaveBeenCalledWith("agent-1", "perm-1", "allow_once");

    act(() => {
      required(
        Array.from(container.querySelectorAll("button")).find(
          (button) => button.textContent?.trim() === "Deny"
        ),
        "deny button missing"
      ).click();
    });
    expect(onRespond).toHaveBeenCalledWith("agent-1", "perm-1");
  });
});

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AcpDebug, AcpDebugProps } from "./components/acp_debug";

const baseProps: AcpDebugProps = {
  currentMode: "default",
  rawEvents: [],
  acpPermissionHistory: [],
  acpModeId: "",
  acpModelId: "",
  acpConfigId: "",
  acpConfigValue: "",
  onAcpModeIdChange: () => {},
  onAcpModelIdChange: () => {},
  onAcpConfigIdChange: () => {},
  onAcpConfigValueChange: () => {},
  canControlAcp: false,
  onAcpSetMode: () => {},
  onAcpSetModel: () => {},
  onAcpSetConfig: () => {},
  onAcpCancel: () => {},
  onAcpClearSession: () => {},
};

describe("AcpDebug", () => {
  it("defaults to session controls view", () => {
    const html = renderToStaticMarkup(<AcpDebug {...baseProps} />);
    expect(html).toContain("Session Controls");
    expect(html).toContain("Mode ID");
    expect(html).toContain("Model ID");
    expect(html).toContain("Config ID");
    expect(html).not.toContain("<h4>Permissions</h4>");
    expect(html).not.toContain("<h4>Raw Events</h4>");
    expect(html).not.toContain("acp-raw");
  });

  it("renders debug tabs", () => {
    const html = renderToStaticMarkup(<AcpDebug {...baseProps} />);
    expect(html).toContain("Permissions");
    expect(html).toContain("Raw Events");
  });
});

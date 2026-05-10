import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AuthGateCard, AuthRequiredGate, ForbiddenRoute } from "./auth_routes";

describe("AuthRoutes", () => {
  function renderHtml(element: React.ReactElement) {
    return renderToStaticMarkup(
      <MantineProvider>
        {element}
      </MantineProvider>
    );
  }

  it("renders AuthGateCard correctly", () => {
    const html = renderHtml(<AuthGateCard title="Test Title" message="Test Message" />);
    expect(html).toContain("Test Title");
    expect(html).toContain("Test Message");
  });

  it("renders AuthRequiredGate correctly", () => {
    const html = renderHtml(<AuthRequiredGate />);
    expect(html).toContain("Login Required");
    expect(html).toContain("Please login to continue.");
  });

  it("renders ForbiddenRoute correctly", () => {
    const html = renderHtml(<ForbiddenRoute />);
    expect(html).toContain("Forbidden");
    expect(html).toContain("You do not have access to this page.");
  });
});

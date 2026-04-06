import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ErrorBanner } from "./error_banner";

describe("ErrorBanner", () => {
  it("renders the message text", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <ErrorBanner message="boom" />
      </MantineProvider>
    );
    expect(html).toContain("boom");
  });

  it("renders a close button when onClose is provided", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <ErrorBanner message="boom" onClose={() => {}} />
      </MantineProvider>
    );
    expect(html).toContain("error-close");
    expect(html).toContain("Dismiss error");
  });

  it("omits the close button when onClose is missing", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <ErrorBanner message="boom" />
      </MantineProvider>
    );
    expect(html).not.toContain("error-close");
  });
});

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";
import { TeamSetupPanel } from "./team_setup_panel";

describe("TeamSetupPanel", () => {
  it("renders first-coordinator setup guidance", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamSetupPanel
          description="Own query debugging"
          forgeLabel="Add First Coordinator Agent"
          copyExistingLabel="Copy Existing Agent"
          onForge={vi.fn()}
          onCopyExisting={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("No agents have joined this team yet.");
    expect(html).toContain("Add First Coordinator Agent");
    expect(html).toContain("Copy Existing Agent");
    expect(html).toContain("Beta");
    expect(html).toContain("until you add the first coordinator agent");
    expect(html).toContain("This first agent becomes the coordinator.");
    expect(html).toContain("Create the first coordinator agent");
  });
});

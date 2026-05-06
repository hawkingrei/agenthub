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
          forgeLabel="Create New Agent"
          copyExistingLabel="Copy Existing Agent"
          onForge={vi.fn()}
          onCopyExisting={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("No agents have joined this team yet.");
    expect(html).toContain("Create New Agent");
    expect(html).toContain("Copy Existing Agent");
    expect(html).toContain("Alpha");
    expect(html).toContain("Choose one of the two agent paths below");
    expect(html).toContain("The first added agent becomes the coordinator");
    expect(html).toContain("Create a new Team-owned agent or copy an existing agent configuration.");
    expect(html).toContain("Create the first coordinator agent");
  });

  it("uses default goal guidance when the team has no description", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamSetupPanel
          description="  "
          forgeLabel="Create New Agent"
          copyExistingLabel="Copy Existing Agent"
          onForge={vi.fn()}
          onCopyExisting={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Capture the mission, constraints, and what this team should own.");
  });
});

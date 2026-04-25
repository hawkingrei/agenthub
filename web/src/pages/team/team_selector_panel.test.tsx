import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { TeamSelectorPanel } from "./team_selector_panel";

describe("TeamSelectorPanel", () => {
  it("renders the loading state without the filter controls", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamSelectorPanel
          busy={null}
          filter=""
          loading={true}
          hasTeams={false}
          items={[]}
          bodyTextClassName="body"
          accentButtonClassName="accent"
          onFilterChange={vi.fn()}
          onRefreshTeams={vi.fn()}
          onCreateTeam={vi.fn()}
          onSelectTeam={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Loading teams...");
    expect(html).not.toContain("No teams yet. Create one to begin.");
    expect(html).not.toContain("Search teams");
  });

  it("renders the empty state when no teams are available", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamSelectorPanel
          busy={null}
          filter=""
          loading={false}
          hasTeams={false}
          items={[]}
          bodyTextClassName="body"
          accentButtonClassName="accent"
          onFilterChange={vi.fn()}
          onRefreshTeams={vi.fn()}
          onCreateTeam={vi.fn()}
          onSelectTeam={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("New Team");
    expect(html).toContain("No teams yet. Create one to begin.");
  });

  it("renders visible teams with summary and runtime label", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamSelectorPanel
          busy={null}
          filter="tidb"
          loading={false}
          hasTeams={true}
          items={[
            {
              id: "team-1",
              name: "TiDB fuzz",
              description: "Fix regressions.",
              summary: "3 members · 2 active",
              runtimeLabel: "running",
            },
          ]}
          bodyTextClassName="body"
          accentButtonClassName="accent"
          onFilterChange={vi.fn()}
          onRefreshTeams={vi.fn()}
          onCreateTeam={vi.fn()}
          onSelectTeam={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Search teams");
    expect(html).toContain("TiDB fuzz");
    expect(html).toContain("3 members · 2 active");
    expect(html).toContain("running");
  });
});

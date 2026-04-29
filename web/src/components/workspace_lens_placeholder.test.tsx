import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";
import {
  WorkspaceMachinesUnavailablePlaceholder,
  WorkspaceLensPlaceholder,
  WorkspaceSearchLensPlaceholder,
} from "./workspace_lens_placeholder";

describe("WorkspaceLensPlaceholder", () => {
  it("renders a shared shell placeholder with lens identity and body copy", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <WorkspaceLensPlaceholder
          lensLabel="Search"
          title="Shared search is still being wired in"
          body="Use Channels, Tasks, or Members while the unified workspace search view is still a shell-level placeholder."
          className="shell-card"
        />
      </MantineProvider>
    );

    expect(html).toContain("Search");
    expect(html).toContain("Shared search is still being wired in");
    expect(html).toContain("shell-level placeholder");
    expect(html).toContain("data-workspace-lens-placeholder=\"search\"");
    expect(html).toContain("shell-card");
  });

  it("renders the shared workspace search placeholder variant", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <WorkspaceSearchLensPlaceholder className="shared-search-card" />
      </MantineProvider>
    );

    expect(html).toContain("Search");
    expect(html).toContain("Shared search is still being wired in");
    expect(html).toContain("shell-level placeholder");
    expect(html).toContain("shared-search-card");
  });

  it("renders the shared machines unavailable placeholder variant", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <WorkspaceMachinesUnavailablePlaceholder className="machines-card" />
      </MantineProvider>
    );

    expect(html).toContain("Machines");
    expect(html).toContain("Machines unavailable");
    expect(html).toContain("permission to manage machines");
    expect(html).toContain("machines-card");
    expect(html).toContain("data-workspace-lens-placeholder=\"machines\"");
  });
});

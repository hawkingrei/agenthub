import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";
import {
  WorkspaceMachinesUnavailablePlaceholder,
  WorkspaceLensPlaceholder,
  WorkspaceSearchLensPlaceholder,
  WorkspaceChannelsLensPlaceholder,
  WorkspaceTasksLensPlaceholder,
  WorkspaceMembersLensPlaceholder,
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

  it("renders the shared workspace search input variant", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <WorkspaceSearchLensPlaceholder className="shared-search-card" />
      </MantineProvider>
    );

    expect(html).toContain("Search");
    expect(html).toContain("Search workspace");
    expect(html).toContain("Find Team messages, tasks, channels, and agent context.");
    expect(html).toContain("Search messages, tasks, channels, or agents");
    expect(html).toContain("type=\"search\"");
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

  it("renders the workspace channels cross-entity placeholder", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <WorkspaceChannelsLensPlaceholder className="channels-card" />
      </MantineProvider>
    );

    expect(html).toContain("Channels");
    expect(html).toContain("Workspace channels aggregate across teams");
    expect(html).toContain("cross-team channel index");
    expect(html).toContain("channels-card");
    expect(html).toContain("data-workspace-lens-placeholder=\"channels\"");
  });

  it("renders the workspace tasks cross-entity placeholder", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <WorkspaceTasksLensPlaceholder className="tasks-card" />
      </MantineProvider>
    );

    expect(html).toContain("Tasks");
    expect(html).toContain("Workspace tasks aggregate across teams");
    expect(html).toContain("cross-team task view");
    expect(html).toContain("tasks-card");
    expect(html).toContain("data-workspace-lens-placeholder=\"tasks\"");
  });

  it("renders the workspace members cross-entity placeholder", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <WorkspaceMembersLensPlaceholder className="members-card" />
      </MantineProvider>
    );

    expect(html).toContain("Members");
    expect(html).toContain("Workspace members aggregate across teams");
    expect(html).toContain("cross-team member directory");
    expect(html).toContain("members-card");
    expect(html).toContain("data-workspace-lens-placeholder=\"members\"");
  });
});

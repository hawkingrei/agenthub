import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  WorkspaceContentStack,
  WorkspaceSectionShell,
  WorkspaceSplitPaneLayout,
} from "./workspace_section_shell";

describe("WorkspaceSectionShell", () => {
  it("renders shared workspace section chrome", () => {
    const html = renderToStaticMarkup(
      <WorkspaceSectionShell className="custom-shell">Content</WorkspaceSectionShell>
    );

    expect(html).toContain('data-workspace-section-shell="true"');
    expect(html).toContain("rounded-xl");
    expect(html).toContain("border-notion-border");
    expect(html).toContain("custom-shell");
  });

  it("uses compact vertical padding for embedded agent workspaces", () => {
    const html = renderToStaticMarkup(<WorkspaceSectionShell compact>Content</WorkspaceSectionShell>);

    expect(html).toContain("py-0.5");
  });

  it("renders shared workspace content stack chrome", () => {
    const html = renderToStaticMarkup(
      <WorkspaceContentStack className="content-stack">Content</WorkspaceContentStack>
    );

    expect(html).toContain('data-workspace-content-stack="true"');
    expect(html).toContain("min-h-0");
    expect(html).toContain("overflow-hidden");
    expect(html).toContain("gap-4");
    expect(html).toContain("content-stack");
  });

  it("uses compact spacing for embedded agent content stacks", () => {
    const html = renderToStaticMarkup(<WorkspaceContentStack compact>Content</WorkspaceContentStack>);

    expect(html).toContain("gap-2");
    expect(html).not.toContain("gap-4");
  });

  it("supports tight workspace body spacing without conflicting gap classes", () => {
    const html = renderToStaticMarkup(
      <WorkspaceContentStack gap="tight">Content</WorkspaceContentStack>
    );

    expect(html).toContain("gap-3");
    expect(html).not.toContain("gap-4");
  });

  it("renders single-pane workspace layout without desktop split classes", () => {
    const html = renderToStaticMarkup(
      <WorkspaceSplitPaneLayout
        primary={<main data-testid="primary-pane">Primary</main>}
        primaryClassName="primary-pane"
      />
    );

    expect(html).toContain('data-workspace-split-pane-layout="true"');
    expect(html).toContain('data-secondary-open="false"');
    expect(html).toContain("primary-pane");
    expect(html).not.toContain("lg:grid-cols-[minmax(0,1.45fr)_minmax(20rem,0.9fr)]");
    expect(html).not.toContain("max-h-[40vh]");
  });

  it("renders split workspace layout with constrained secondary dock", () => {
    const html = renderToStaticMarkup(
      <WorkspaceSplitPaneLayout
        primary={<main data-testid="primary-pane">Primary</main>}
        secondary={<aside data-testid="secondary-pane">Secondary</aside>}
        secondaryClassName="secondary-dock"
      />
    );

    expect(html).toContain('data-secondary-open="true"');
    expect(html).toContain("lg:grid-cols-[minmax(0,1.45fr)_minmax(20rem,0.9fr)]");
    expect(html).toContain("max-h-[40vh]");
    expect(html).toContain("secondary-dock");
  });
});

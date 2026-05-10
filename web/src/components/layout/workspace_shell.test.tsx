import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { WorkspaceShell, type WorkspaceShellProps } from "./workspace_shell";

describe("WorkspaceShell", () => {
  const baseProps: WorkspaceShellProps = {
    title: "Test Workspace",
    subtitle: "Testing...",
    activeSurface: "workspace" as const,
    username: "testuser",
    isRoot: false,
    agentsCollapsed: false,
    onToggleAgents: vi.fn(),
    normalizedError: null,
    onClearError: vi.fn(),
    onNavigate: vi.fn(),
    onLogout: vi.fn(),
    children: <div>Content</div>,
  };

  function renderHtml(props: Partial<WorkspaceShellProps> = {}) {
    return renderToStaticMarkup(
      <MantineProvider>
        <WorkspaceShell {...baseProps} {...props} />
      </MantineProvider>
    );
  }

  it("renders with default sidebar labels when not provided", () => {
    const html = renderHtml({ agentsCollapsed: false });
    expect(html).toContain("aria-label=\"Hide agents\"");
    
    const collapsedHtml = renderHtml({ agentsCollapsed: true });
    expect(collapsedHtml).toContain("aria-label=\"Show agents\"");
  });

  it("renders with custom sidebar labels when provided (e.g. for Teams)", () => {
    const html = renderHtml({ 
      agentsCollapsed: false,
      sidebarToggleLabel: "Hide teams panel" 
    });
    expect(html).toContain("aria-label=\"Hide teams panel\"");
    
    const collapsedHtml = renderHtml({ 
      agentsCollapsed: true,
      sidebarToggleLabel: "Show teams panel"
    });
    expect(collapsedHtml).toContain("aria-label=\"Show teams panel\"");
  });

  it("renders error banners when provided", () => {
    const html = renderHtml({ normalizedError: "Something went wrong" });
    expect(html).toContain("Something went wrong");
  });

  it("renders warning notices when provided", () => {
    const html = renderHtml({ 
      warningNotice: <div data-testid="warning">Warning Notice</div> 
    });
    expect(html).toContain("Warning Notice");
  });
});

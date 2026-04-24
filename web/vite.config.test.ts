import { describe, expect, it } from "vitest";
import config, { resolveChunkGroupName, resolveManualChunkName } from "./vite.config";

function resolveChunkGroup(id: string): string | undefined {
  return resolveChunkGroupName(id);
}

function resolveModulePreloadDeps(deps: string[]): string[] {
  const modulePreload = config.build?.modulePreload;
  const resolveDependencies =
    modulePreload && "resolveDependencies" in modulePreload
      ? modulePreload.resolveDependencies
      : null;
  expect(typeof resolveDependencies).toBe("function");
  return (
    resolveDependencies as (
      url: string,
      deps: string[]
    ) => string[]
  )("/assets/index.js", deps);
}

describe("vite chunk grouping", () => {
  it("routes agents workbench-only modules into a separate lazy chunk", () => {
    expect(resolveChunkGroup("/repo/web/src/components/agents_workbench.tsx")).toBe(
      "route-agents-workbench"
    );
    expect(resolveChunkGroup("/repo/web/src/components/output_body.tsx")).toBe(
      "route-agents-workbench"
    );
  });

  it("routes ACP shell modules into the shared ACP chunk", () => {
    expect(resolveChunkGroup("/repo/web/src/components/input_dock.tsx")).toBe(
      "route-acp-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/components/acp_panel.tsx")).toBe(
      "route-acp-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/acp.ts")).toBe("route-acp-shared");
    expect(resolveChunkGroup("/repo/web/src/components/acp_panel_helpers.ts")).toBe(
      "route-acp-shared"
    );
    expect(
      resolveChunkGroup("/repo/web/src/components/acp_conversation.tsx")
    ).toBe("route-acp-shared");
    expect(resolveChunkGroup("/repo/web/src/components/acp_tool_bubbles.tsx")).toBe(
      "route-acp-tools"
    );
    expect(resolveChunkGroup("/repo/web/src/components/acp_tool_call_bubble.tsx")).toBe(
      "route-acp-tools"
    );
    expect(resolveChunkGroup("/repo/web/src/components/acp_plan.tsx")).toBe(
      "route-acp-plan"
    );
  });

  it("routes terminal and rich-text helpers into their dedicated chunks", () => {
    expect(resolveChunkGroup("/repo/web/src/components/terminal_output.tsx")).toBe(
      "route-agents-terminal"
    );
    expect(resolveChunkGroup("/repo/web/src/components/acp_debug.tsx")).toBe(
      "route-agents-debug"
    );
    expect(resolveChunkGroup("/repo/web/src/pages/team/team_thread_rich_text.tsx")).toBe(
      "route-rich-text-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/pages/team/team_markdown.ts")).toBe(
      "route-rich-text-shared"
    );
  });

  it("keeps the agent index shell in the primary agents route chunk", () => {
    expect(resolveChunkGroup("/repo/web/src/components/agents_panel.tsx")).toBe(
      "route-agents"
    );
    expect(resolveChunkGroup("/repo/web/src/api.ts")).toBe("route-agents");
    expect(resolveChunkGroup("/repo/web/src/acp.ts")).toBe("route-acp-shared");
    expect(resolveChunkGroup("/repo/web/src/push.ts")).toBe("route-agents");
    expect(resolveChunkGroup("/repo/web/src/connection_status.ts")).toBe(
      "route-agents"
    );
    expect(resolveChunkGroup("/repo/web/src/app_live_output.ts")).toBe(
      "route-app-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/event_polling.ts")).toBe(
      "route-app-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/scroll.ts")).toBe(
      "route-agents"
    );
    expect(resolveChunkGroup("/repo/web/src/html_escape.ts")).toBe(
      "route-app-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/components/workbench_connection_badge.tsx")).toBe(
      "route-agents"
    );
    expect(resolveChunkGroup("/repo/web/src/auth_redirect.ts")).toBe(
      "route-agents"
    );
    expect(resolveChunkGroup("/repo/web/src/error_banner.tsx")).toBe(
      "route-agents"
    );
    expect(resolveChunkGroup("/repo/web/src/components/workbench_header_menu.tsx")).toBe(
      "route-agents"
    );
    expect(resolveChunkGroup("/repo/web/src/components/acp_panel_helpers.ts")).toBe(
      "route-acp-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/hooks/use_acp_conversation.ts")).toBe(
      "route-acp-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/thread_markdown.ts")).toBe("route-rich-text-shared");
    expect(resolveChunkGroup("/repo/web/src/markdown.ts")).toBe("route-rich-text-shared");
    expect(resolveChunkGroup("/repo/web/src/components/thread_rich_text.tsx")).toBe(
      "route-rich-text-shared"
    );
  });

  it("pins shared ui primitives away from the agents debug/workbench chunks", () => {
    expect(resolveChunkGroup("/repo/web/src/ui/primitives.tsx")).toBe(
      "route-ui-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/ui/floating_surfaces.ts")).toBe(
      "route-ui-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/ui/tailwind_classes.ts")).toBe(
      "route-ui-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/components/acp_debug_loader.ts")).toBe(
      "route-agents-debug-loader"
    );
    expect(resolveChunkGroup("/repo/web/src/input_ime.ts")).toBe(
      "route-ui-shared"
    );
    expect(resolveChunkGroup("/repo/web/src/components/status_badge.tsx")).toBe(
      "route-ui-shared"
    );
    expect(
      resolveChunkGroup("/repo/web/node_modules/@mantine/core/esm/components/Textarea/Textarea.mjs")
    ).toBe("route-mantine-inputs");
    expect(
      resolveChunkGroup("/repo/web/node_modules/@mantine/core/esm/components/Combobox/Combobox.mjs")
    ).toBe("route-mantine-inputs");
    expect(
      resolveChunkGroup(
        "/repo/web/node_modules/@mantine/core/esm/components/CloseButton/CloseButton.mjs"
      )
    ).toBe("route-mantine-inputs");
    expect(
      resolveChunkGroup(
        "/repo/web/node_modules/@mantine/core/esm/components/Modal/Modal.mjs"
      )
    ).toBe("route-mantine-inputs");
    expect(
      resolveChunkGroup(
        "/repo/web/node_modules/@mantine/core/esm/components/InputWrapper/InputWrapper.mjs"
      )
    ).toBe("route-mantine-inputs");
    expect(
      resolveChunkGroup(
        "/repo/web/node_modules/@mantine/core/esm/core/utils/to-int/to-int.mjs"
      )
    ).toBe("route-mantine-inputs");
    expect(
      resolveChunkGroup(
        "/repo/web/node_modules/@mantine/hooks/esm/use-debounced-callback/use-debounced-callback.mjs"
      )
    ).toBe("route-mantine-inputs");
    expect(
      resolveChunkGroup("/repo/web/node_modules/@mantine/core/esm/index.mjs")
    ).toBe("vendor-mantine");
    expect(
      resolveChunkGroup("/repo/web/node_modules/@mantine/hooks/esm/index.mjs")
    ).toBe("vendor-mantine");
    expect(
      resolveChunkGroup("/repo/web/node_modules/@mantine/core/esm/components/Button/Button.mjs")
    ).toBe("vendor-mantine");
    expect(
      resolveChunkGroup("/repo/web/node_modules/@mantine/hooks/esm/use-media-query/use-media-query.mjs")
    ).toBe("vendor-mantine");
    expect(resolveChunkGroup("/repo/web/node_modules/markdown-it/lib/index.mjs")).toBe(
      "route-rich-text-shared"
    );
  });

  it("exposes a rollup-compatible manualChunks fallback", () => {
    expect(resolveManualChunkName("/repo/web/src/pages/team_member_acp_panel.tsx")).toBe(
      "route-teams-agent-acp"
    );
    expect(resolveManualChunkName("/repo/web/src/components/acp_debug.tsx")).toBe(
      "route-agents-debug"
    );
    expect(resolveManualChunkName("/repo/web/src/components/acp_debug_loader.ts")).toBe(
      "route-agents-debug-loader"
    );
    expect(resolveManualChunkName("/repo/web/src/components/acp_panel.tsx")).toBe(
      "route-acp-shared"
    );
    expect(resolveManualChunkName("/repo/web/src/html_escape.ts")).toBe(
      "route-app-shared"
    );
    expect(resolveManualChunkName("/repo/web/src/components/thread_rich_text.tsx")).toBe(
      "route-rich-text-shared"
    );
    expect(resolveManualChunkName("\0/repo/web/src/components/acp_debug.tsx?import")).toBe(
      "route-agents-debug"
    );
  });

  it("keeps team route code in the team chunk", () => {
    expect(resolveChunkGroup("/repo/web/src/pages/team_page.tsx")).toBe(
      "route-teams"
    );
    expect(resolveChunkGroup("/repo/web/src/pages/team_member_acp_panel.tsx")).toBe(
      "route-teams-agent-acp"
    );
  });

  it("does not modulepreload lazy agents workbench or debug chunks", () => {
    expect(
      resolveModulePreloadDeps([
        "assets/route-agents-abc.js",
        "assets/route-app-shared-abc.js",
        "assets/route-agents-debug-loader-abc.js",
        "assets/route-agents-workbench-abc.js",
        "assets/route-agents-debug-abc.js",
        "assets/route-acp-plan-abc.js",
        "assets/route-acp-shared-abc.js",
        "assets/route-acp-tools-abc.js",
        "assets/route-teams-agent-acp-abc.js",
        "assets/vendor-mantine-abc.js",
      ])
    ).toEqual([
      "assets/route-agents-abc.js",
      "assets/vendor-mantine-abc.js",
    ]);
  });
});

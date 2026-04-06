import { describe, expect, it } from "vitest";
import config from "./vite.config";

function resolveManualChunk(id: string): string | undefined {
  const output = config.build?.rollupOptions?.output;
  const chunkConfig = Array.isArray(output) ? output[0] : output;
  const manualChunks =
    chunkConfig && "manualChunks" in chunkConfig ? chunkConfig.manualChunks : null;
  expect(typeof manualChunks).toBe("function");
  return (manualChunks as (id: string) => string | undefined)(id);
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

describe("vite manualChunks", () => {
  it("routes heavy agents workbench modules into a separate lazy chunk", () => {
    expect(resolveManualChunk("/repo/web/src/components/agents_workbench.tsx")).toBe(
      "route-agents-workbench"
    );
    expect(resolveManualChunk("/repo/web/src/components/output_body.tsx")).toBe(
      "route-agents-workbench"
    );
    expect(resolveManualChunk("/repo/web/src/components/input_dock.tsx")).toBe(
      "route-agents-workbench"
    );
    expect(resolveManualChunk("/repo/web/src/components/acp_panel.tsx")).toBe(
      "route-agents-workbench"
    );
    expect(resolveManualChunk("/repo/web/src/components/terminal_output.tsx")).toBe(
      "route-agents-terminal"
    );
    expect(
      resolveManualChunk("/repo/web/src/components/acp_conversation.tsx")
    ).toBe("route-agents-workbench");
    expect(resolveManualChunk("/repo/web/src/components/acp_debug.tsx")).toBe(
      "route-agents-debug"
    );
    expect(resolveManualChunk("/repo/web/src/pages/team/team_thread_rich_text.tsx")).toBe(
      "route-teams-rich-text"
    );
    expect(resolveManualChunk("/repo/web/src/pages/team/team_markdown.ts")).toBe(
      "route-teams-rich-text"
    );
  });

  it("keeps the agent index shell in the primary agents route chunk", () => {
    expect(resolveManualChunk("/repo/web/src/components/agents_panel.tsx")).toBe(
      "route-agents"
    );
    expect(resolveManualChunk("/repo/web/src/api.ts")).toBe("route-agents");
    expect(resolveManualChunk("/repo/web/src/acp.ts")).toBe("route-agents");
    expect(resolveManualChunk("/repo/web/src/push.ts")).toBe("route-agents");
    expect(resolveManualChunk("/repo/web/src/connection_status.ts")).toBe(
      "route-agents"
    );
    expect(resolveManualChunk("/repo/web/src/scroll.ts")).toBe(
      "route-agents"
    );
    expect(resolveManualChunk("/repo/web/src/html_escape.ts")).toBe(
      "route-agents"
    );
    expect(resolveManualChunk("/repo/web/src/components/workbench_connection_badge.tsx")).toBe(
      "route-agents"
    );
    expect(resolveManualChunk("/repo/web/src/auth_redirect.ts")).toBe(
      "route-agents"
    );
    expect(resolveManualChunk("/repo/web/src/error_banner.tsx")).toBe(
      "route-agents"
    );
    expect(resolveManualChunk("/repo/web/src/components/workbench_header_menu.tsx")).toBe(
      "route-agents"
    );
    expect(resolveManualChunk("/repo/web/src/components/acp_panel_helpers.ts")).toBe(
      "route-agents"
    );
    expect(resolveManualChunk("/repo/web/src/hooks/use_acp_conversation.ts")).toBe(
      undefined
    );
    expect(resolveManualChunk("/repo/web/src/thread_markdown.ts")).toBe(
      "route-agents-workbench"
    );
    expect(resolveManualChunk("/repo/web/src/markdown.ts")).toBe(
      "route-agents-workbench"
    );
    expect(resolveManualChunk("/repo/web/src/components/thread_rich_text.tsx")).toBe(
      "route-agents-workbench"
    );
  });

  it("pins shared ui primitives away from the agents debug/workbench chunks", () => {
    expect(resolveManualChunk("/repo/web/src/ui/primitives.tsx")).toBe(
      "route-ui-shared"
    );
    expect(resolveManualChunk("/repo/web/src/ui/floating_surfaces.ts")).toBe(
      "route-ui-shared"
    );
    expect(resolveManualChunk("/repo/web/src/ui/tailwind_classes.ts")).toBe(
      "route-ui-shared"
    );
    expect(resolveManualChunk("/repo/web/src/input_ime.ts")).toBe(
      "route-ui-shared"
    );
    expect(resolveManualChunk("/repo/web/src/components/status_badge.tsx")).toBe(
      "route-ui-shared"
    );
    expect(
      resolveManualChunk("/repo/web/node_modules/@mantine/core/esm/components/Button/Button.mjs")
    ).toBe("vendor-mantine");
    expect(
      resolveManualChunk("/repo/web/node_modules/@mantine/hooks/esm/use-media-query/use-media-query.mjs")
    ).toBe("vendor-mantine");
  });

  it("keeps team route code in the team chunk", () => {
    expect(resolveManualChunk("/repo/web/src/pages/team_page.tsx")).toBe(
      "route-teams"
    );
    expect(resolveManualChunk("/repo/web/src/pages/team_member_acp_panel.tsx")).toBe(
      "route-teams-agent-acp"
    );
  });

  it("does not modulepreload lazy agents workbench or debug chunks", () => {
    expect(
      resolveModulePreloadDeps([
        "assets/route-agents-abc.js",
        "assets/route-agents-workbench-abc.js",
        "assets/route-agents-debug-abc.js",
        "assets/route-teams-agent-acp-abc.js",
        "assets/vendor-mantine-abc.js",
      ])
    ).toEqual([
      "assets/route-agents-abc.js",
      "assets/vendor-mantine-abc.js",
    ]);
  });
});

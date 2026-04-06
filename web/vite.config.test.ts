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
  });

  it("keeps the agent index shell in the primary agents route chunk", () => {
    expect(resolveManualChunk("/repo/web/src/components/agents_panel.tsx")).toBe(
      "route-agents"
    );
    expect(resolveManualChunk("/repo/web/src/acp.ts")).toBe("route-agents");
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
  });

  it("keeps team route code in the team chunk", () => {
    expect(resolveManualChunk("/repo/web/src/pages/team_page.tsx")).toBe(
      "route-teams"
    );
  });

  it("does not modulepreload lazy agents workbench or debug chunks", () => {
    expect(
      resolveModulePreloadDeps([
        "assets/route-agents-abc.js",
        "assets/route-agents-workbench-abc.js",
        "assets/route-agents-debug-abc.js",
        "assets/vendor-mantine-abc.js",
      ])
    ).toEqual([
      "assets/route-agents-abc.js",
      "assets/vendor-mantine-abc.js",
    ]);
  });
});

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const ROUTE_AGENTS_IDS = [
  "/src/api.ts",
  "/src/connection_status.ts",
  "/src/error_banner.tsx",
  "/src/scroll.ts",
  "/src/html_escape.ts",
  "/src/worktree_defaults.ts",
  "/src/input_history.ts",
  "/src/push.ts",
  "/src/auth_redirect.ts",
  "/src/webauthn.ts",
  "/src/components/workbench_connection_badge.tsx",
  "/src/components/workbench_header_menu.tsx",
  "/src/components/acp_panel_helpers.ts",
  "/src/components/agents_panel.tsx",
  "/src/components/output_header.tsx",
  "/src/components/output_error_boundary.tsx",
  "/src/acp.ts",
  "/src/agent_ws.ts",
  "/src/output_cache.ts",
];

const ROUTE_UI_SHARED_IDS = [
  "/src/ui/primitives.tsx",
  "/src/ui/floating_surfaces.ts",
  "/src/ui/tailwind_classes.ts",
  "/src/input_ime.ts",
  "/src/components/status_badge.tsx",
];

const ROUTE_AUTH_IDS = [
  "/src/pages/admin_page.tsx",
  "/src/pages/join_page.tsx",
  "/src/pages/auth_pages.tsx",
];

const ROUTE_AGENTS_WORKBENCH_IDS = [
  "/src/markdown.ts",
  "/src/thread_markdown.ts",
  "/src/components/thread_rich_text.tsx",
  "/src/components/agents_workbench.tsx",
  "/src/components/output_body.tsx",
  "/src/components/input_dock.tsx",
  "/src/components/acp_panel.tsx",
  "/src/components/acp_conversation.tsx",
  "/src/components/acp_plan.tsx",
  "/src/components/acp_request_user_input_cards.tsx",
  "/src/components/acp_tool_",
];

const ROUTE_TEAMS_RICH_TEXT_IDS = [
  "/src/pages/team/team_markdown.ts",
  "/src/pages/team/team_thread_rich_text.tsx",
];

function normalizeChunkId(id: string): string {
  return id.split("\\").join("/");
}

function includesAny(id: string, needles: string[]): boolean {
  return needles.some((needle) => id.includes(needle));
}

export function resolveChunkGroupName(id: string): string | undefined {
  const normalized = normalizeChunkId(id);
  if (normalized.includes("/node_modules/@mantine/")) {
    return "vendor-mantine";
  }
  if (
    normalized.includes("/node_modules/highlight.js/") ||
    normalized.includes("/node_modules/markdown-it/")
  ) {
    return "vendor-markdown";
  }
  if (normalized.includes("/node_modules/qrcode/")) {
    return "vendor-qrcode";
  }
  if (includesAny(normalized, ROUTE_UI_SHARED_IDS)) {
    return "route-ui-shared";
  }
  if (normalized.includes("/src/pages/team_member_acp_panel.tsx")) {
    return "route-teams-agent-acp";
  }
  if (includesAny(normalized, ROUTE_TEAMS_RICH_TEXT_IDS)) {
    return "route-teams-rich-text";
  }
  if (normalized.includes("/src/pages/team_page.tsx") || normalized.includes("/src/pages/team/")) {
    return "route-teams";
  }
  if (includesAny(normalized, ROUTE_AUTH_IDS)) {
    return "route-auth";
  }
  if (normalized.includes("/src/components/acp_debug.tsx")) {
    return "route-agents-debug";
  }
  if (normalized.includes("/src/components/terminal_output.tsx")) {
    return "route-agents-terminal";
  }
  if (includesAny(normalized, ROUTE_AGENTS_WORKBENCH_IDS)) {
    return "route-agents-workbench";
  }
  if (includesAny(normalized, ROUTE_AGENTS_IDS)) {
    return "route-agents";
  }
  return undefined;
}

const CHUNK_GROUPS = [
  { name: "vendor-mantine", priority: 100 },
  { name: "vendor-markdown", priority: 90 },
  { name: "vendor-qrcode", priority: 80 },
  { name: "route-ui-shared", priority: 70 },
  { name: "route-teams-agent-acp", priority: 60 },
  { name: "route-teams-rich-text", priority: 55 },
  { name: "route-teams", priority: 50 },
  { name: "route-auth", priority: 45 },
  { name: "route-agents-debug", priority: 40 },
  { name: "route-agents-terminal", priority: 35 },
  { name: "route-agents-workbench", priority: 30 },
  { name: "route-agents", priority: 20 },
].map(({ name, priority }) => ({
  name,
  priority,
  test(id: string) {
    return resolveChunkGroupName(id) === name;
  },
}));

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "node",
  },
  build: {
    modulePreload: {
      resolveDependencies(_url, deps) {
        return deps.filter(
          (dep) =>
            !dep.includes("route-auth-") &&
            !dep.includes("route-agents-workbench-") &&
            !dep.includes("route-agents-debug-") &&
            !dep.includes("route-teams-agent-acp-") &&
            !dep.includes("route-teams-") &&
            !dep.includes("vendor-markdown-")
        );
      },
    },
    outDir: "dist",
    emptyOutDir: true,
    chunkSizeWarningLimit: 1500,
    rollupOptions: {
      output: {
        codeSplitting: {
          groups: CHUNK_GROUPS,
        },
      },
    },
  },
});

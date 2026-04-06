import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const ROUTE_AGENTS_IDS = [
  "/src/api.ts",
  "/src/connection_status.ts",
  "/src/error_banner.tsx",
  "/src/scroll.ts",
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
  "/src/html_escape.ts",
  "/src/components/status_badge.tsx",
];

const ROUTE_RICH_TEXT_SHARED_IDS = [
  "/src/markdown.ts",
  "/src/thread_markdown.ts",
  "/src/components/thread_rich_text.tsx",
  "/src/pages/team/team_markdown.ts",
  "/src/pages/team/team_thread_rich_text.tsx",
  "/node_modules/highlight.js/",
  "/node_modules/markdown-it/",
  "/node_modules/linkify-it/",
  "/node_modules/mdurl/",
  "/node_modules/uc.micro/",
  "/node_modules/entities/",
  "/node_modules/punycode.js/",
];

const ROUTE_MANTINE_INPUT_IDS = [
  "/node_modules/@mantine/hooks/esm/utils/use-callback-ref/",
  "/node_modules/@mantine/hooks/esm/use-debounced-callback/",
  "/node_modules/@mantine/core/esm/core/utils/find-element-in-shadow-dom/",
  "/node_modules/@mantine/core/esm/components/CloseButton/",
  "/node_modules/@mantine/core/esm/components/ScrollArea/",
  "/node_modules/@mantine/core/esm/components/Input/",
  "/node_modules/@mantine/core/esm/components/InputBase/",
  "/node_modules/@mantine/core/esm/components/Combobox/",
  "/node_modules/@mantine/core/esm/components/NativeSelect/",
  "/node_modules/@mantine/core/esm/components/TextInput/",
  "/node_modules/@mantine/core/esm/components/Textarea/",
  "/node_modules/@mantine/core/esm/components/Select/",
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

function normalizeChunkId(id: string): string {
  const normalizedPath = id.split("\\").join("/");
  const withoutVirtualPrefix = normalizedPath.startsWith("\0")
    ? normalizedPath.slice(1)
    : normalizedPath;
  const queryIndex = withoutVirtualPrefix.indexOf("?");
  return queryIndex === -1
    ? withoutVirtualPrefix
    : withoutVirtualPrefix.slice(0, queryIndex);
}

function includesAny(id: string, needles: string[]): boolean {
  return needles.some((needle) => id.includes(needle));
}

export function resolveChunkGroupName(id: string): string | undefined {
  const normalized = normalizeChunkId(id);
  if (includesAny(normalized, ROUTE_MANTINE_INPUT_IDS)) {
    return "route-mantine-inputs";
  }
  if (includesAny(normalized, ROUTE_RICH_TEXT_SHARED_IDS)) {
    return "route-rich-text-shared";
  }
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

export function resolveManualChunkName(id: string): string | undefined {
  return resolveChunkGroupName(id);
}

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
        manualChunks(id) {
          return resolveManualChunkName(id);
        },
      },
    },
  },
});

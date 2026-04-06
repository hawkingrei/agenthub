import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

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
          const normalized = id.split("\\").join("/");
          if (
            normalized.includes("/src/api.ts") ||
            normalized.includes("/src/connection_status.ts") ||
            normalized.includes("/src/error_banner.tsx") ||
            normalized.includes("/src/scroll.ts") ||
            normalized.includes("/src/html_escape.ts") ||
            normalized.includes("/src/worktree_defaults.ts") ||
            normalized.includes("/src/input_history.ts") ||
            normalized.includes("/src/push.ts") ||
            normalized.includes("/src/auth_redirect.ts") ||
            normalized.includes("/src/webauthn.ts") ||
            normalized.includes("/src/components/workbench_connection_badge.tsx") ||
            normalized.includes("/src/components/workbench_header_menu.tsx") ||
            normalized.includes("/src/components/acp_panel_helpers.ts")
          ) {
            return "route-agents";
          }
          if (normalized.includes("/src/pages/team_page.tsx") || normalized.includes("/src/pages/team/")) {
            return "route-teams";
          }
          if (
            normalized.includes("/src/pages/admin_page.tsx") ||
            normalized.includes("/src/pages/join_page.tsx") ||
            normalized.includes("/src/pages/auth_pages.tsx")
          ) {
            return "route-auth";
          }
          if (
            normalized.includes("/src/components/acp_debug.tsx")
          ) {
            return "route-agents-debug";
          }
          if (normalized.includes("/src/components/terminal_output.tsx")) {
            return "route-agents-terminal";
          }
          if (
            normalized.includes("/src/components/agents_workbench.tsx") ||
            normalized.includes("/src/components/output_body.tsx") ||
            normalized.includes("/src/components/input_dock.tsx") ||
            normalized.includes("/src/components/acp_panel.tsx") ||
            normalized.includes("/src/components/acp_conversation.tsx") ||
            normalized.includes("/src/components/acp_plan.tsx") ||
            normalized.includes("/src/components/acp_tool_") ||
            normalized.includes("/src/components/acp_request_user_input_cards.tsx") ||
            normalized.includes("/src/components/thread_rich_text.tsx")
          ) {
            return "route-agents-workbench";
          }
          if (
            normalized.includes("/src/components/agents_panel.tsx") ||
            normalized.includes("/src/components/output_header.tsx") ||
            normalized.includes("/src/components/output_error_boundary.tsx") ||
            normalized.includes("/src/acp.ts") ||
            normalized.includes("/src/agent_ws.ts") ||
            normalized.includes("/src/output_cache.ts")
          ) {
            return "route-agents";
          }
          if (!normalized.includes("/node_modules/")) return undefined;
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
          return undefined;
        },
      },
    },
  },
});

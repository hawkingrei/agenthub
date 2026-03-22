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
            normalized.includes("/src/components/agents_panel.tsx") ||
            normalized.includes("/src/components/create_agent_modal.tsx") ||
            normalized.includes("/src/components/agent_node_section.tsx") ||
            normalized.includes("/src/components/output_body.tsx") ||
            normalized.includes("/src/components/output_header.tsx") ||
            normalized.includes("/src/components/output_error_boundary.tsx") ||
            normalized.includes("/src/components/permission_modal.tsx") ||
            normalized.includes("/src/components/input_dock.tsx") ||
            normalized.includes("/src/components/acp_panel.tsx") ||
            normalized.includes("/src/components/acp_conversation.tsx") ||
            normalized.includes("/src/hooks/use_acp_conversation.ts") ||
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

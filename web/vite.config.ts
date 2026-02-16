import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "node",
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    chunkSizeWarningLimit: 1500,
    rollupOptions: {
      output: {
        manualChunks(id) {
          const normalized = id.split("\\").join("/");
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

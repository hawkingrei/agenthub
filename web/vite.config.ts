import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      registerType: "autoUpdate",
      injectRegister: null,
      strategies: "injectManifest",
      srcDir: "src",
      filename: "sw.js",
      includeAssets: ["pwa-192.png", "pwa-512.png"],
      manifest: {
        name: "AgentHub",
        short_name: "AgentHub",
        description: "Remote control and management for AI agents.",
        theme_color: "#1b3a57",
        background_color: "#f6efe6",
        display: "standalone",
        start_url: "/",
        icons: [
          {
            src: "pwa-192.png",
            sizes: "192x192",
            type: "image/png",
          },
          {
            src: "pwa-512.png",
            sizes: "512x512",
            type: "image/png",
          },
          {
            src: "pwa-512.png",
            sizes: "512x512",
            type: "image/png",
            purpose: "maskable",
          },
        ],
      },
    }),
  ],
  test: {
    environment: "node",
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});

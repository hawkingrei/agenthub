import { describe, expect, it } from "vitest";

import manifestRaw from "../public/manifest.webmanifest?raw";
import serviceWorkerRaw from "../public/sw.js?raw";

type WebAppManifest = {
  id?: string;
  name?: string;
  short_name?: string;
  start_url?: string;
  scope?: string;
  display?: string;
  background_color?: string;
  theme_color?: string;
  icons?: Array<{
    src?: string;
    sizes?: string;
    type?: string;
  }>;
};

describe("PWA public assets", () => {
  it("keeps installable manifest metadata stable", () => {
    const manifest = JSON.parse(manifestRaw) as WebAppManifest;

    expect(manifest.id).toBe("/");
    expect(manifest.name).toBe("AgentHub");
    expect(manifest.short_name).toBe("AgentHub");
    expect(manifest.start_url).toBe("/");
    expect(manifest.scope).toBe("/");
    expect(manifest.display).toBe("standalone");
    expect(manifest.background_color).toMatch(/^#[0-9a-f]{6}$/i);
    expect(manifest.theme_color).toMatch(/^#[0-9a-f]{6}$/i);
    expect(manifest.icons).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          src: "/pwa-192.png",
          sizes: "192x192",
          type: "image/png",
        }),
        expect.objectContaining({
          src: "/pwa-512.png",
          sizes: "512x512",
          type: "image/png",
        }),
      ])
    );
  });

  it("keeps the service worker free of fetch interception and precaching", () => {
    expect(serviceWorkerRaw).toContain('self.addEventListener("install"');
    expect(serviceWorkerRaw).toContain('self.addEventListener("activate"');
    expect(serviceWorkerRaw).toContain('self.addEventListener("push"');
    expect(serviceWorkerRaw).toContain('self.addEventListener("notificationclick"');
    expect(serviceWorkerRaw).not.toMatch(/addEventListener\(\s*["'`]fetch["'`]/);
    expect(serviceWorkerRaw).not.toMatch(/\bonfetch\b/);
    expect(serviceWorkerRaw).not.toMatch(/\bcaches\b/);
    expect(serviceWorkerRaw).not.toMatch(/\bprecache\b/i);
  });
});

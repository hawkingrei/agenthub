import { defineConfig, devices } from "@playwright/test";
import path from "node:path";
import { fileURLToPath } from "node:url";

const port = Number(process.env.PLAYWRIGHT_PORT ?? 5173);
const baseURL = `http://127.0.0.1:${port}`;
const disableWebServer = process.env.PLAYWRIGHT_NO_WEBSERVER === "1";
const minimalRuntime = process.env.PLAYWRIGHT_MINIMAL_RUNTIME === "1";
const enableSystemChrome = process.env.PLAYWRIGHT_SYSTEM_CHROME === "1";
const enableE2eCoverage = process.env.PLAYWRIGHT_E2E_COVERAGE === "1";
const configDir = path.dirname(fileURLToPath(import.meta.url));

const webServer = disableWebServer
  ? undefined
  : {
      command: `npm run dev -- --host 127.0.0.1 --port ${port} --strictPort`,
      cwd: configDir,
      url: baseURL,
      reuseExistingServer: !process.env.CI,
    };

export default defineConfig({
  testDir: "./tests/e2e",
  testMatch: "**/*.e2e.ts",
  timeout: 30_000,
  expect: {
    timeout: 5_000,
  },
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI || enableE2eCoverage ? 1 : undefined,
  use: {
    baseURL,
    trace: minimalRuntime ? "off" : "retain-on-failure",
    screenshot: minimalRuntime ? "off" : "only-on-failure",
    video: minimalRuntime ? "off" : "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
    ...(enableSystemChrome
      ? [
          {
            name: "system-chrome",
            use: {
              ...devices["Desktop Chrome"],
              channel: "chrome" as const,
            },
          },
        ]
      : []),
  ],
  webServer,
});

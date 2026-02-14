import { defineConfig, devices } from "@playwright/test";

const port = Number(process.env.PLAYWRIGHT_PORT ?? 5173);
const baseURL = `http://127.0.0.1:${port}`;
const disableWebServer = process.env.PLAYWRIGHT_NO_WEBSERVER === "1";
const minimalRuntime = process.env.PLAYWRIGHT_MINIMAL_RUNTIME === "1";

const webServer = disableWebServer
  ? undefined
  : {
      command: `npm run dev -- --host 127.0.0.1 --port ${port} --strictPort`,
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
  workers: process.env.CI ? 1 : undefined,
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
    {
      name: "system-chrome",
      use: {
        ...devices["Desktop Chrome"],
        channel: "chrome",
      },
    },
  ],
  webServer,
});

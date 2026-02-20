import { expect, test as base } from "@playwright/test";
import type { TestInfo } from "@playwright/test";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import coverageLib from "istanbul-lib-coverage";
import reportLib from "istanbul-lib-report";
import reports from "istanbul-reports";
import {
  mergeCoverageEntries,
  type JsCoverageEntry,
} from "./coverage_merge";

type CoverageState = {
  map: ReturnType<typeof coverageLib.createCoverageMap>;
};

const E2E_COVERAGE_ENV = "PLAYWRIGHT_E2E_COVERAGE";
const E2E_COVERAGE_ENABLED = process.env[E2E_COVERAGE_ENV] === "1";
const WEB_ROOT = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const COVERAGE_DIR = path.join(WEB_ROOT, "coverage/e2e");

const coverageState = globalThis as typeof globalThis & {
  __agenthub_e2e_coverage__?: CoverageState;
};
if (!coverageState.__agenthub_e2e_coverage__) {
  coverageState.__agenthub_e2e_coverage__ = {
    map: coverageLib.createCoverageMap({}),
  };
}

function isChromiumProject(testInfo: TestInfo): boolean {
  return (
    testInfo.project.name === "chromium" ||
    testInfo.project.name === "system-chrome"
  );
}

function resolveCoverageFilePath(rawUrl: string): string | null {
  let parsed: URL;
  try {
    parsed = new URL(rawUrl);
  } catch {
    return null;
  }
  const pathname = decodeURIComponent(parsed.pathname);
  if (!pathname.startsWith("/src/")) {
    return null;
  }
  const normalizedPath = pathname.toLowerCase();
  if (!/\.(?:[cm]?[jt]sx?)$/.test(normalizedPath)) {
    return null;
  }
  const resolved = path.join(WEB_ROOT, pathname.slice(1));
  if (!existsSync(resolved)) {
    return null;
  }
  return resolved;
}

function writeCoverageArtifacts(): void {
  const map = coverageState.__agenthub_e2e_coverage__?.map;
  if (!map) return;
  mkdirSync(COVERAGE_DIR, { recursive: true });
  writeFileSync(
    path.join(COVERAGE_DIR, "coverage-final.json"),
    JSON.stringify(map.toJSON())
  );
  const context = reportLib.createContext({
    dir: COVERAGE_DIR,
    coverageMap: map,
    defaultSummarizer: "nested",
  });
  reports.create("lcovonly", { file: "lcov.info" }).execute(context);
  reports.create("text-summary").execute(context);
}

export const test = base.extend({
  page: async ({ page }, runTest, testInfo) => {
    const collectCoverage = E2E_COVERAGE_ENABLED && isChromiumProject(testInfo);
    if (collectCoverage) {
      await page.coverage.startJSCoverage({
        resetOnNavigation: false,
        reportAnonymousScripts: false,
      });
    }

    await runTest(page);

    if (collectCoverage) {
      const entries = (await page.coverage.stopJSCoverage()) as JsCoverageEntry[];
      const map = coverageState.__agenthub_e2e_coverage__?.map;
      if (map) {
        await mergeCoverageEntries({
          map,
          entries,
          resolveFilePath: resolveCoverageFilePath,
        });
      }
      writeCoverageArtifacts();
    }
  },
});

export { expect };

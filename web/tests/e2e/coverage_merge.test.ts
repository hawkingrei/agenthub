import { describe, expect, it, vi } from "vitest";
import type { CoverageMapData } from "istanbul-lib-coverage";
import type { JsCoverageEntry } from "./coverage_merge";
import { mergeCoverageEntries } from "./coverage_merge";

function createEmptyCoverage(filePath: string): CoverageMapData {
  return {
    [filePath]: {
      path: filePath,
      statementMap: {},
      fnMap: {},
      branchMap: {},
      s: {},
      f: {},
      b: {},
    },
  };
}

const SAMPLE_ENTRY: JsCoverageEntry = {
  url: "http://127.0.0.1:4173/src/main.ts",
  source: "export const value = 1;\n",
  functions: [
    {
      functionName: "(empty-report)",
      isBlockCoverage: true,
      ranges: [{ startOffset: 0, endOffset: 23, count: 1 }],
    },
  ],
};

describe("mergeCoverageEntries", () => {
  it("uses modern converter signature without calling load/applyCoverage", async () => {
    const filePath = "/repo/web/src/main.ts";
    const merge = vi.fn();
    const converter = vi.fn(async () => createEmptyCoverage(filePath));

    await mergeCoverageEntries({
      map: { merge },
      entries: [SAMPLE_ENTRY],
      resolveFilePath: () => filePath,
      converter,
      logger: { warn: vi.fn() },
    });

    expect(converter).toHaveBeenCalledTimes(1);
    expect(merge).toHaveBeenCalledTimes(1);
  });

  it("supports legacy converter signature with load/applyCoverage", async () => {
    const filePath = "/repo/web/src/main.ts";
    const merge = vi.fn();
    const load = vi.fn(async () => undefined);
    const applyCoverage = vi.fn();
    const toIstanbul = vi.fn(() => createEmptyCoverage(filePath));
    const legacyFactory = vi.fn(function legacyFactory(
      _filePath: string,
      _wrapperLength: number,
      _options: { source?: string }
    ) {
      void _filePath;
      void _wrapperLength;
      void _options;
      return {
        load,
        applyCoverage,
        toIstanbul,
      };
    });

    await mergeCoverageEntries({
      map: { merge },
      entries: [SAMPLE_ENTRY],
      resolveFilePath: () => filePath,
      converter: legacyFactory,
      logger: { warn: vi.fn() },
    });

    expect(legacyFactory).toHaveBeenCalledTimes(1);
    expect(load).toHaveBeenCalledTimes(1);
    expect(applyCoverage).toHaveBeenCalledTimes(1);
    expect(toIstanbul).toHaveBeenCalledTimes(1);
    expect(merge).toHaveBeenCalledTimes(1);
  });

  it("falls back to modern conversion when legacy-style invocation fails", async () => {
    const filePath = "/repo/web/src/main.ts";
    const merge = vi.fn();
    const warn = vi.fn();
    const converter = vi.fn(async function converterWithLegacyArity(
      input: unknown,
      _wrapperLength: number,
      _options: { source?: string }
    ) {
      void _wrapperLength;
      void _options;
      if (
        !input ||
        typeof input !== "object" ||
        !("code" in input) ||
        !("coverage" in input)
      ) {
        throw new TypeError("modern signature expected");
      }
      return createEmptyCoverage(filePath);
    });

    await mergeCoverageEntries({
      map: { merge },
      entries: [SAMPLE_ENTRY],
      resolveFilePath: () => filePath,
      converter,
      logger: { warn },
    });

    expect(converter).toHaveBeenCalledTimes(2);
    expect(merge).toHaveBeenCalledTimes(1);
    expect(warn).not.toHaveBeenCalled();
  });

  it("skips entries with missing source or empty functions", async () => {
    const merge = vi.fn();
    const converter = vi.fn(async () => createEmptyCoverage("/repo/web/src/main.ts"));

    await mergeCoverageEntries({
      map: { merge },
      entries: [
        { ...SAMPLE_ENTRY, source: undefined },
        { ...SAMPLE_ENTRY, functions: [] },
      ],
      resolveFilePath: () => "/repo/web/src/main.ts",
      converter,
      logger: { warn: vi.fn() },
    });

    expect(converter).not.toHaveBeenCalled();
    expect(merge).not.toHaveBeenCalled();
  });

  it("continues when conversion fails and logs warning", async () => {
    const merge = vi.fn();
    const warn = vi.fn();
    const converter = vi.fn(async () => {
      throw new Error("conversion failed");
    });

    await mergeCoverageEntries({
      map: { merge },
      entries: [SAMPLE_ENTRY],
      resolveFilePath: () => "/repo/web/src/main.ts",
      converter,
      logger: { warn },
    });

    expect(merge).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalledTimes(1);
  });
});

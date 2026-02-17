import { parse } from "acorn";
import type { CoverageMapData } from "istanbul-lib-coverage";
import astV8ToIstanbul from "ast-v8-to-istanbul";

type JsCoverageFunction = {
  functionName: string;
  isBlockCoverage: boolean;
  ranges: Array<{
    startOffset: number;
    endOffset: number;
    count: number;
  }>;
};

export type JsCoverageEntry = {
  url: string;
  source?: string;
  functions?: JsCoverageFunction[];
};

type LegacyCoverageConverter = {
  load: () => Promise<void> | void;
  applyCoverage: (functions: JsCoverageFunction[]) => void;
  toIstanbul: () => CoverageMapData;
};

type LegacyCoverageConverterFactory = (
  filePath: string,
  wrapperLength?: number,
  options?: { source?: string }
) => LegacyCoverageConverter;

type ModernCoverageConverter = (options: {
  code: string;
  coverage: Pick<JsCoverageEntry, "url"> & { functions: JsCoverageFunction[] };
  ast: unknown;
  wrapperLength?: number;
}) => Promise<CoverageMapData>;

type CoverageConverter = LegacyCoverageConverterFactory | ModernCoverageConverter;

type CoverageMapLike = {
  merge: (coverage: CoverageMapData) => void;
};

type CoverageLogger = {
  warn: (message: string, error?: unknown) => void;
};

type MergeCoverageEntriesOptions = {
  map: CoverageMapLike;
  entries: JsCoverageEntry[];
  resolveFilePath: (rawUrl: string) => string | null;
  converter?: CoverageConverter;
  logger?: CoverageLogger;
};

const defaultConverter = astV8ToIstanbul as unknown as CoverageConverter;

function isLegacyCoverageConverter(value: unknown): value is LegacyCoverageConverter {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<LegacyCoverageConverter>;
  return (
    typeof candidate.load === "function" &&
    typeof candidate.applyCoverage === "function" &&
    typeof candidate.toIstanbul === "function"
  );
}

function isThenable<T>(value: unknown): value is Promise<T> {
  return (
    !!value &&
    (typeof value === "object" || typeof value === "function") &&
    typeof (value as { then?: unknown }).then === "function"
  );
}

async function tryLegacyConversion(
  converter: CoverageConverter,
  filePath: string,
  source: string,
  functions: JsCoverageFunction[]
): Promise<CoverageMapData | null> {
  let candidate: unknown;
  try {
    candidate = (converter as LegacyCoverageConverterFactory)(filePath, 0, {
      source,
    });
  } catch {
    return null;
  }

  if (isLegacyCoverageConverter(candidate)) {
    await candidate.load();
    candidate.applyCoverage(functions);
    return candidate.toIstanbul();
  }

  if (isThenable<CoverageMapData>(candidate)) {
    try {
      return await candidate;
    } catch {
      return null;
    }
  }

  if (candidate && typeof candidate === "object") {
    return candidate as CoverageMapData;
  }

  return null;
}

function buildCoverageAst(source: string, filePath: string): unknown {
  const options = {
    ecmaVersion: "latest" as const,
    locations: true,
    sourceFile: filePath,
  };
  try {
    return parse(source, { ...options, sourceType: "module" });
  } catch {
    return parse(source, { ...options, sourceType: "script" });
  }
}

async function convertCoverageEntry(
  converter: CoverageConverter,
  filePath: string,
  entry: JsCoverageEntry
): Promise<CoverageMapData> {
  const functions = entry.functions ?? [];
  const source = entry.source ?? "";
  const legacyCoverage = await tryLegacyConversion(
    converter,
    filePath,
    source,
    functions
  );
  if (legacyCoverage) {
    return legacyCoverage;
  }

  const ast = buildCoverageAst(source, filePath);
  return (converter as ModernCoverageConverter)({
    code: source,
    coverage: { url: entry.url, functions },
    ast,
    wrapperLength: 0,
  });
}

export async function mergeCoverageEntries({
  map,
  entries,
  resolveFilePath,
  converter = defaultConverter,
  logger = console,
}: MergeCoverageEntriesOptions): Promise<void> {
  for (const entry of entries) {
    const filePath = resolveFilePath(entry.url);
    if (!filePath) {
      continue;
    }
    const hasSource = typeof entry.source === "string" && entry.source.length > 0;
    const functions = entry.functions ?? [];
    if (!hasSource || functions.length === 0) {
      continue;
    }
    try {
      const coverage = await convertCoverageEntry(converter, filePath, entry);
      map.merge(coverage);
    } catch (error) {
      logger.warn(`[e2e-coverage] failed to merge coverage for ${filePath}`, error);
    }
  }
}

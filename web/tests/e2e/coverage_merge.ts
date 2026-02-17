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

function isModernCoverageConverter(
  converter: CoverageConverter
): converter is ModernCoverageConverter {
  return converter.length <= 1;
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
  if (isModernCoverageConverter(converter)) {
    const ast = buildCoverageAst(entry.source ?? "", filePath);
    return converter({
      code: entry.source ?? "",
      coverage: { url: entry.url, functions },
      ast,
      wrapperLength: 0,
    });
  }

  const legacyConverter = converter(filePath, 0, { source: entry.source });
  await legacyConverter.load();
  legacyConverter.applyCoverage(functions);
  return legacyConverter.toIstanbul();
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

import { readdirSync, readFileSync, statSync } from "node:fs";
import { extname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const SRC_ROOT = fileURLToPath(new URL(".", import.meta.url));
const CJK_CHAR_PATTERN = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/u;
const SOURCE_EXTENSIONS = new Set([".ts", ".tsx"]);
const TEST_FILE_PATTERN = /\.test\.(ts|tsx)$/;

type CjkMatch = {
  file: string;
  line: number;
  sample: string;
};

describe("UI language guard", () => {
  it("keeps runtime frontend source free of Chinese literals", () => {
    const sourceFiles = listRuntimeSourceFiles(SRC_ROOT);
    const matches: CjkMatch[] = [];

    for (const file of sourceFiles) {
      const content = readFileSync(file, "utf8");
      const lines = content.split("\n");
      for (let index = 0; index < lines.length; index += 1) {
        const line = lines[index];
        if (!CJK_CHAR_PATTERN.test(line)) continue;
        matches.push({
          file: relative(SRC_ROOT, file),
          line: index + 1,
          sample: line.trim().slice(0, 140),
        });
        break;
      }
    }

    const message =
      matches.length === 0
        ? undefined
        : `Chinese literals found in runtime UI source:\n${matches
            .map((item) => `- ${item.file}:${item.line} ${item.sample}`)
            .join("\n")}`;
    expect(matches, message).toEqual([]);
  });
});

function listRuntimeSourceFiles(root: string): string[] {
  const files: string[] = [];
  walkDirectory(root, files);
  return files;
}

function walkDirectory(directory: string, files: string[]): void {
  const entries = readdirSync(directory, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name.startsWith(".")) continue;
    const fullPath = join(directory, entry.name);
    if (entry.isDirectory()) {
      walkDirectory(fullPath, files);
      continue;
    }
    if (!entry.isFile()) continue;
    if (!SOURCE_EXTENSIONS.has(extname(entry.name))) continue;
    if (TEST_FILE_PATTERN.test(entry.name)) continue;
    if (!statSync(fullPath).isFile()) continue;
    files.push(fullPath);
  }
}

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const tailwindCss = readFileSync(new URL("./src/tailwind.css", import.meta.url), "utf8");

describe("tailwind theme tokens", () => {
  it("mirrors the extended notion tokens in @theme", () => {
    for (const token of [
      "--color-notion-bg-subtle",
      "--color-notion-bg-soft",
      "--color-notion-bg-panel",
      "--color-notion-code-bg",
      "--color-notion-payload-bg",
      "--color-notion-payload-border",
      "--color-notion-plan-bg",
      "--color-notion-plan-border",
      "--color-notion-plan-progress",
      "--color-notion-plan-progress-from",
      "--color-notion-plan-progress-to",
      "--color-notion-surface-card",
      "--color-notion-surface-overlay",
      "--color-notion-surface-overlay-strong",
      "--color-notion-surface-elevated",
      "--color-notion-surface-tint",
      "--color-notion-border-subtle",
      "--color-notion-border-faint",
      "--color-notion-hover-subtle",
      "--color-notion-hover-soft",
      "--color-notion-hover-strong",
      "--color-notion-bubble-user",
    ]) {
      expect(tailwindCss).toContain(token);
    }
  });
});

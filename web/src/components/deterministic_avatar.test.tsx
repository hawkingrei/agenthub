import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  DeterministicAvatar,
  renderDeterministicAvatarModel,
  resolveDeterministicAvatarSeed,
} from "./deterministic_avatar";

describe("DeterministicAvatar", () => {
  it("uses the same seed and shape for the same name/id pair", () => {
    const first = renderDeterministicAvatarModel("Worker Agent", "worker-1");
    const second = renderDeterministicAvatarModel("Worker Agent", "worker-1");

    expect(first.seed).toBe("Worker Agent::worker-1");
    expect(second.seed).toBe(first.seed);
    expect(second.variantKey).toBe(first.variantKey);
    expect(second.backgroundColor).toBe(first.backgroundColor);
    expect(second.foregroundColor).toBe(first.foregroundColor);
    expect(second.borderColor).toBe(first.borderColor);
    expect(second.accentColor).toBe(first.accentColor);
    expect(second.shadowColor).toBe(first.shadowColor);
    expect(second.cells).toEqual(first.cells);
    expect(resolveDeterministicAvatarSeed("Worker Agent", "worker-1")).toBe(first.seed);
  });

  it("changes the generated avatar when the stable id changes", () => {
    const first = renderDeterministicAvatarModel("Worker Agent", "worker-1");
    const second = renderDeterministicAvatarModel("Worker Agent", "worker-2");

    expect(second.seed).not.toBe(first.seed);
    expect(second.variantKey).not.toBe(first.variantKey);
  });

  it("renders a deterministic object avatar shell", () => {
    const html = renderToStaticMarkup(
      <DeterministicAvatar
        name="Coordinator"
        stableId="coordinator-agent"
        className="h-7 w-7 border border-black/8"
      />
    );

    expect(html).toContain('data-avatar-seed="Coordinator::coordinator-agent"');
    expect(html).toContain('data-avatar-variant="');
    expect(html).toContain('class="inline-flex shrink-0 overflow-hidden rounded-full h-7 w-7 border border-black/8"');
    expect(html).toContain("grid-template-columns:repeat(8, 1fr)");
    expect(html).toContain("image-rendering:pixelated");
  });

  it("keeps every row horizontally symmetric", () => {
    const model = renderDeterministicAvatarModel("Coordinator", "coordinator-agent");
    for (let row = 0; row < 8; row += 1) {
      for (let col = 0; col < 4; col += 1) {
        const left = model.cells[row * 8 + col];
        const right = model.cells[row * 8 + (7 - col)];
        expect(left).toBe(right);
      }
    }
  });
});

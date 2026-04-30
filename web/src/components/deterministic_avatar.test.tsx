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
    expect(second.backgroundColor).toBe(first.backgroundColor);
    expect(second.foregroundColor).toBe(first.foregroundColor);
    expect(second.cells).toEqual(first.cells);
    expect(resolveDeterministicAvatarSeed("Worker Agent", "worker-1")).toBe(first.seed);
  });

  it("changes the generated avatar when the stable id changes", () => {
    const first = renderDeterministicAvatarModel("Worker Agent", "worker-1");
    const second = renderDeterministicAvatarModel("Worker Agent", "worker-2");

    expect(second.seed).not.toBe(first.seed);
    expect(second.cells).not.toEqual(first.cells);
  });

  it("renders a deterministic SVG avatar shell", () => {
    const html = renderToStaticMarkup(
      <DeterministicAvatar
        name="Leader"
        stableId="leader-agent"
        className="h-7 w-7 border border-black/8"
      />
    );

    expect(html).toContain('data-avatar-seed="Leader::leader-agent"');
    expect(html).toContain("<svg");
    expect(html).toContain('class="inline-flex shrink-0 overflow-hidden rounded-full h-7 w-7 border border-black/8"');
    expect(html).toContain('role="presentation"');
  });
});

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
        name="Leader"
        stableId="leader-agent"
        className="h-7 w-7 border border-black/8"
      />
    );

    expect(html).toContain('data-avatar-seed="Leader::leader-agent"');
    expect(html).toContain('data-avatar-variant="');
    expect(html).toContain('class="inline-flex shrink-0 overflow-hidden rounded-full h-7 w-7 border border-black/8"');
    expect(html).toContain("<svg");
  });
});

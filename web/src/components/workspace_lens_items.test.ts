import { describe, expect, it, vi } from "vitest";
import { buildStandardWorkspaceLensItems } from "./workspace_lens_items";
import { SHARED_WORKSPACE_SEARCH_LENS_HINT } from "./workspace_lens_placeholder";

describe("buildStandardWorkspaceLensItems", () => {
  it("builds the shared workspace lens set with a search placeholder hint", () => {
    const items = buildStandardWorkspaceLensItems("tasks");

    expect(items.map((item) => item.value)).toEqual([
      "teams",
      "channels",
      "tasks",
      "members",
      "search",
    ]);
    expect(items.find((item) => item.value === "tasks")?.active).toBe(true);
    expect(items.find((item) => item.value === "search")?.title).toBe(
      SHARED_WORKSPACE_SEARCH_LENS_HINT
    );
  });

  it("optionally appends Machines and wires prefetch callbacks", () => {
    const onPrefetch = vi.fn();
    const items = buildStandardWorkspaceLensItems("nodes", {
      includeNodes: true,
      onPrefetch,
    });

    expect(items.map((item) => item.value)).toEqual([
      "teams",
      "channels",
      "tasks",
      "members",
      "search",
      "nodes",
    ]);
    items.find((item) => item.value === "members")?.onPrefetch?.();
    expect(onPrefetch).toHaveBeenCalledWith("members");
    expect(items.find((item) => item.value === "nodes")?.active).toBe(true);
  });
});

import { describe, expect, it, vi } from "vitest";
import { buildStandardWorkspaceLensItems } from "./workspace_lens_items";

describe("buildStandardWorkspaceLensItems", () => {
  it("builds the shared workspace lens set without search as a content lens", () => {
    const items = buildStandardWorkspaceLensItems("tasks");

    expect(items.map((item) => item.value)).toEqual([
      "teams",
      "channels",
      "tasks",
      "members",
    ]);
    expect(items.map((item) => item.label)).toEqual([
      "Teams",
      "Channels",
      "Tasks",
      "Members",
    ]);
    expect(items.find((item) => item.value === "tasks")?.active).toBe(true);
    expect(items.find((item) => item.value === "search")).toBeUndefined();
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
      "nodes",
    ]);
    items.find((item) => item.value === "members")?.onPrefetch?.();
    expect(onPrefetch).toHaveBeenCalledWith("members");
    expect(items.find((item) => item.value === "nodes")?.active).toBe(true);
  });
});

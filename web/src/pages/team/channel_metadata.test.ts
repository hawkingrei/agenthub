import { describe, expect, it } from "vitest";

import type { TeamChannelRecord } from "../../api";
import { buildTeamChannelItems } from "./channel_metadata";

function channelRecord(
  channel_id: string,
  overrides: Partial<TeamChannelRecord> = {}
): TeamChannelRecord {
  return {
    team_id: "team-1",
    channel_id,
    task_id: `task-${channel_id}`,
    conversation_id: `conversation-${channel_id}`,
    description: null,
    created_by_actor_id: "user:tester",
    created_at: 1,
    updated_at: 1,
    ...overrides,
  };
}

describe("buildTeamChannelItems", () => {
  it("keeps a default channel when the API has not returned channels yet", () => {
    const items = buildTeamChannelItems([]);

    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      id: "all",
      label: "# all",
    });
  });

  it("keeps the default channel singular when the API also returns all", () => {
    const items = buildTeamChannelItems([
      channelRecord("all", { description: "API default channel" }),
      channelRecord("review", { description: "  Review lane  " }),
    ]);

    expect(items.map((item) => item.id)).toEqual(["all", "review"]);
    expect(items[0]?.label).toBe("# all");
    expect(items[1]).toMatchObject({
      id: "review",
      label: "# review",
      description: "Review lane",
    });
  });

  it("returns only the default channel when API channels collapse to all", () => {
    const items = buildTeamChannelItems([channelRecord("all")]);

    expect(items).toHaveLength(1);
    expect(items[0]?.id).toBe("all");
  });
});

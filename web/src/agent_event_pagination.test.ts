import { describe, expect, it } from "vitest";

import { hasPotentialOlderAgentEvents } from "./agent_event_pagination";

describe("hasPotentialOlderAgentEvents", () => {
  it("treats any non-empty page as expandable", () => {
    expect(hasPotentialOlderAgentEvents(1)).toBe(true);
    expect(hasPotentialOlderAgentEvents(20)).toBe(true);
    expect(hasPotentialOlderAgentEvents(60)).toBe(true);
  });

  it("treats an empty page as exhausted history", () => {
    expect(hasPotentialOlderAgentEvents(0)).toBe(false);
  });
});

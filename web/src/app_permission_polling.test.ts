import { describe, expect, it, vi } from "vitest";
import {
  GLOBAL_PERMISSION_POLL_IDLE_DELAY_MS,
  schedulePermissionPollLoop,
} from "./app_permission_polling";

describe("schedulePermissionPollLoop", () => {
  it("reschedules after poll failures instead of leaking a rejection", async () => {
    const pollState = { timer: null as number | null };
    const scheduled: Array<() => Promise<void> | void> = [];
    const delays: number[] = [];
    let nextTimerId = 0;
    const pollOnce = vi.fn(async () => {
      throw new Error("boom");
    });

    schedulePermissionPollLoop(
      5000,
      pollState,
      pollOnce,
      () => false,
      (callback, delayMs) => {
        scheduled.push(callback);
        delays.push(delayMs);
        nextTimerId += 1;
        return nextTimerId;
      },
      vi.fn()
    );

    expect(scheduled).toHaveLength(1);
    expect(delays).toEqual([5000]);

    await scheduled[0]?.();

    expect(pollOnce).toHaveBeenCalledTimes(1);
    expect(scheduled).toHaveLength(2);
    expect(delays).toEqual([5000, GLOBAL_PERMISSION_POLL_IDLE_DELAY_MS]);
    expect(pollState.timer).toBe(2);
  });
});

import { describe, expect, it } from "vitest";
import { createRafThrottle } from "./raf_throttle";

describe("createRafThrottle", () => {
  it("runs callback immediately when requestAnimationFrame is unavailable", () => {
    let count = 0;
    const throttle = createRafThrottle(() => {
      count += 1;
    });
    throttle.schedule();
    throttle.schedule();
    expect(count).toBe(2);
    expect(throttle.isPending()).toBe(false);
  });

  it("coalesces repeated schedule calls into one pending frame", () => {
    let count = 0;
    const queue: FrameRequestCallback[] = [];
    const throttle = createRafThrottle(
      () => {
        count += 1;
      },
      {
        requestAnimationFrame: (cb) => {
          queue.push(cb);
          return queue.length;
        },
      }
    );

    throttle.schedule();
    throttle.schedule();
    throttle.schedule();

    expect(queue.length).toBe(1);
    expect(count).toBe(0);
    expect(throttle.isPending()).toBe(true);

    const cb = queue.shift();
    expect(cb).toBeDefined();
    cb?.(16);

    expect(count).toBe(1);
    expect(throttle.isPending()).toBe(false);
  });

  it("supports canceling pending frame callbacks", () => {
    let count = 0;
    const queue = new Map<number, FrameRequestCallback>();
    let nextId = 1;
    const cancelled: number[] = [];
    const throttle = createRafThrottle(
      () => {
        count += 1;
      },
      {
        requestAnimationFrame: (cb) => {
          const id = nextId++;
          queue.set(id, cb);
          return id;
        },
        cancelAnimationFrame: (id) => {
          cancelled.push(id);
          queue.delete(id);
        },
      }
    );

    throttle.schedule();
    expect(throttle.isPending()).toBe(true);
    throttle.cancel();
    expect(cancelled).toEqual([1]);
    expect(throttle.isPending()).toBe(false);
    expect(count).toBe(0);
    expect(queue.size).toBe(0);
  });
});

// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useLoopPage, type LoopPage } from "./use_loop_page";

(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;
type Entry = { id: number; status: string };
type Page = LoopPage<Entry, number>;
type Reader = (cursor: number | null, signal: AbortSignal) => Promise<Page>;
const page = (id: number, nextCursor: number | null): Page => ({
  items: [{ id, status: "pending" }],
  nextCursor,
});

describe("loop history pagination", () => {
  let root: Root;
  let container: HTMLDivElement;
  let value: ReturnType<typeof useLoopPage<Entry, number>>;
  function Harness({ scope, read }: { scope: string; read: Reader }) {
    value = useLoopPage(scope, read);
    return null;
  }
  const render = async (scope: string, read: Reader) => {
    await act(async () => {
      root.render(<Harness scope={scope} read={read} />);
    });
  };
  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it("uses scoped cursors, retains zero, and merges duplicate entries without duplicate rows", async () => {
    const read = vi
      .fn<Reader>()
      .mockResolvedValueOnce(page(1, 0))
      .mockResolvedValueOnce({
        items: [
          { id: 1, status: "finished" },
          { id: 2, status: "pending" },
        ],
        nextCursor: null,
      });
    await render("actor", read);
    await act(async () => value.loadMore());
    expect(read.mock.calls[1][0]).toBe(0);
    expect(value.state?.items).toEqual([
      { id: 1, status: "finished" },
      { id: 2, status: "pending" },
    ]);
    expect(value.state?.viewingOlder).toBe(true);
    await act(async () => value.loadMore());
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("aborts stale reads when switching scope and ignores a transport that still resolves", async () => {
    let finish!: (page: Page) => void;
    const read = vi
      .fn<Reader>()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      )
      .mockResolvedValueOnce(page(2, null));
    await render("old", read);
    await render("new", read);
    expect(read.mock.calls[0][1].aborted).toBe(true);
    await act(async () => finish(page(1, 1)));
    expect(value.state?.items).toEqual(page(2, null).items);
  });

  it("serializes load-more clicks and resets the cursor chain on a fresh read", async () => {
    let finish!: (page: Page) => void;
    const read = vi
      .fn<Reader>()
      .mockResolvedValueOnce(page(1, 1))
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      )
      .mockResolvedValueOnce(page(3, 3));
    await render("actor", read);
    let pending!: Promise<void>;
    act(() => {
      pending = value.loadMore();
      void value.loadMore();
    });
    expect(read).toHaveBeenCalledTimes(2);
    await act(async () => {
      finish(page(2, 2));
      await pending;
    });
    await act(async () => value.refresh());
    expect(value.state?.items).toEqual(page(3, 3).items);
    expect(value.state?.nextCursor).toBe(3);
    expect(value.state?.viewingOlder).toBe(false);
  });

  it("preserves loaded evidence and the retry cursor after a page failure", async () => {
    const read = vi
      .fn<Reader>()
      .mockResolvedValueOnce(page(1, 1))
      .mockRejectedValueOnce(new Error("offline"));
    await render("actor", read);
    await act(async () => value.loadMore());
    expect(value.state?.items).toEqual(page(1, 1).items);
    expect(value.state?.nextCursor).toBe(1);
    expect(value.state?.error).toBe("offline");
  });
});

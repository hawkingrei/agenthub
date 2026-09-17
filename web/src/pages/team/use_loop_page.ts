import { useCallback, useEffect, useRef, useState } from "react";
import { stringifyApiError } from "../../api";

export type LoopPage<T, Cursor> = { items: T[]; nextCursor: Cursor | null };
type PageState<T, Cursor> = LoopPage<T, Cursor> & {
  scope: string;
  loading: boolean;
  error: string | null;
  viewingOlder: boolean;
};

// A refresh starts a new contiguous page chain; appending never runs concurrently with a read.
export function useLoopPage<
  T extends { id: string | number },
  Cursor extends string | number,
>(
  scope: string,
  read: (
    cursor: Cursor | null,
    signal: AbortSignal,
  ) => Promise<LoopPage<T, Cursor>>,
) {
  const activeScope = useRef<string | null>(null);
  const request = useRef<{
    controller: AbortController;
    pending: boolean;
  } | null>(null);
  const [state, setState] = useState<PageState<T, Cursor> | null>(null);

  const load = useCallback(
    async (cursor: Cursor | null, append: boolean) => {
      if (activeScope.current !== scope || (append && request.current?.pending))
        return;
      request.current?.controller.abort();
      const pending = { controller: new AbortController(), pending: true };
      request.current = pending;
      setState((previous) => ({
        scope,
        items: previous?.scope === scope ? previous.items : [],
        nextCursor: previous?.scope === scope ? previous.nextCursor : null,
        viewingOlder: previous?.scope === scope && previous.viewingOlder,
        loading: true,
        error: null,
      }));
      try {
        const page = await read(cursor, pending.controller.signal);
        if (pending.controller.signal.aborted || request.current !== pending)
          return;
        setState((previous) => {
          const items =
            append && previous?.scope === scope ? previous.items : [];
          const byId = new Map(items.map((item) => [item.id, item]));
          for (const item of page.items) byId.set(item.id, item);
          return {
            scope,
            items: [...byId.values()],
            nextCursor: page.nextCursor,
            loading: false,
            error: null,
            viewingOlder: append,
          };
        });
      } catch (error) {
        if (pending.controller.signal.aborted || request.current !== pending)
          return;
        setState(
          (previous) =>
            previous && {
              ...previous,
              loading: false,
              error: stringifyApiError(error),
            },
        );
      } finally {
        pending.pending = false;
      }
    },
    [scope, read],
  );

  useEffect(() => {
    activeScope.current = scope;
    void load(null, false);
    return () => {
      activeScope.current = null;
      request.current?.controller.abort();
    };
  }, [scope, load]);

  const refresh = useCallback(() => load(null, false), [load]);
  const current = state?.scope === scope ? state : null;
  const nextCursor = current?.nextCursor ?? null;
  const loadMore = useCallback(async () => {
    if (nextCursor != null) await load(nextCursor, true);
  }, [nextCursor, load]);
  return { state: current, refresh, loadMore };
}

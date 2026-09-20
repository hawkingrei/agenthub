import { useCallback, useEffect, useRef, useState } from "react";
import { api, getApiErrorStatus, stringifyApiError } from "../../api";
import type {
  LoopConfiguration,
  LoopConfigurationUpdate,
  LoopTriggerReceipt,
} from "../../loop_types";
import {
  clearLoopActivationIntent,
  loopActivationIntentKey,
  readLoopActivationIntent,
  retainLoopActivationIntent,
} from "./loop_activation_intent";

export type LoopMemberScope = {
  token: string;
  userId: string;
  teamId: string;
  actorId: string;
};
type ConfigurationState = {
  scope: string;
  configuration: LoopConfiguration | null;
  loading: boolean;
  stale: boolean;
  busy: "configure" | "activate" | null;
  error: string | null;
  actionError: string | null;
  retryPending: boolean;
  receipt: LoopTriggerReceipt | null;
};

export function useLoopConfiguration({
  token,
  userId,
  teamId,
  actorId,
}: LoopMemberScope) {
  const scope = JSON.stringify([token, userId, teamId, actorId]);
  const storageKey = loopActivationIntentKey(userId, teamId, actorId);
  const scopeRef = useRef<string | null>(scope);
  const lifetimeRef = useRef(0);
  const requestRef = useRef(0);
  const readRef = useRef<AbortController | null>(null);
  const mutationRef = useRef<{ scope: string; lifetime: number } | null>(null);
  const intentRef = useRef<{ scope: string; id: string } | null>(null);
  const [state, setState] = useState<ConfigurationState | null>(null);

  const refresh = useCallback(async () => {
    if (
      scopeRef.current !== scope ||
      (mutationRef.current?.scope === scope &&
        mutationRef.current.lifetime === lifetimeRef.current)
    )
      return;
    readRef.current?.abort();
    const controller = new AbortController();
    readRef.current = controller;
    const request = ++requestRef.current;
    setState((previous) => ({
      scope,
      configuration: previous?.scope === scope ? previous.configuration : null,
      loading: true,
      stale: previous?.scope === scope ? previous.stale : true,
      busy: null,
      error: null,
      actionError: previous?.scope === scope ? previous.actionError : null,
      receipt: previous?.scope === scope ? previous.receipt : null,
      retryPending:
        intentRef.current?.scope === scope ||
        readLoopActivationIntent(storageKey) != null,
    }));
    try {
      const configuration = await api.getTeamMemberLoop(
        token,
        teamId,
        actorId,
        controller.signal,
      );
      if (scopeRef.current !== scope || requestRef.current !== request) return;
      setState(
        (previous) =>
          previous && {
            ...previous,
            configuration,
            loading: false,
            stale: false,
          },
      );
    } catch (error) {
      if (scopeRef.current !== scope || requestRef.current !== request) return;
      setState(
        (previous) =>
          previous && {
            ...previous,
            loading: false,
            stale: true,
            error: stringifyApiError(error),
          },
      );
    }
  }, [scope, storageKey, token, teamId, actorId]);

  useEffect(() => {
    lifetimeRef.current += 1;
    scopeRef.current = scope;
    void refresh();
    return () => {
      readRef.current?.abort();
      scopeRef.current = null;
      lifetimeRef.current += 1;
      requestRef.current += 1;
    };
  }, [scope, refresh]);

  const configure = useCallback(
    async (update: LoopConfigurationUpdate) => {
      const lifetime = lifetimeRef.current;
      if (
        scopeRef.current !== scope ||
        (mutationRef.current?.scope === scope &&
          mutationRef.current.lifetime === lifetime)
      )
        return;
      const operation = { scope, lifetime };
      const current = () =>
        scopeRef.current === scope && lifetimeRef.current === lifetime;
      mutationRef.current = operation;
      readRef.current?.abort();
      requestRef.current += 1;
      setState(
        (previous) =>
          previous && {
            ...previous,
            busy: "configure",
            loading: false,
            actionError: null,
          },
      );
      let reload = false;
      try {
        const configuration = await api.configureTeamMemberLoop(
          token,
          teamId,
          actorId,
          update,
        );
        if (current()) {
          setState(
            (previous) =>
              previous && {
                ...previous,
                configuration,
                stale: false,
                error: null,
              },
          );
        }
      } catch (error) {
        if (current()) {
          reload = true;
          const conflict = getApiErrorStatus(error) === 409;
          setState(
            (previous) =>
              previous && {
                ...previous,
                stale: true,
                actionError: conflict
                  ? `Configuration changed or preflight failed. Review the refreshed settings before trying again. ${stringifyApiError(error)}`
                  : stringifyApiError(error),
              },
          );
        }
      } finally {
        if (mutationRef.current === operation) mutationRef.current = null;
        if (current()) {
          setState((previous) => previous && { ...previous, busy: null });
          if (reload) await refresh();
        }
      }
    },
    [scope, token, teamId, actorId, refresh],
  );

  const activate = useCallback(async () => {
    const lifetime = lifetimeRef.current;
    if (
      scopeRef.current !== scope ||
      (mutationRef.current?.scope === scope &&
        mutationRef.current.lifetime === lifetime)
    )
      return;
    const operation = { scope, lifetime };
    const current = () =>
      scopeRef.current === scope && lifetimeRef.current === lifetime;
    mutationRef.current = operation;
    readRef.current?.abort();
    requestRef.current += 1;
    const intent =
      intentRef.current?.scope === scope
        ? intentRef.current.id
        : (readLoopActivationIntent(storageKey) ?? crypto.randomUUID());
    intentRef.current = { scope, id: intent };
    retainLoopActivationIntent(storageKey, intent);
    setState(
      (previous) =>
        previous && {
          ...previous,
          busy: "activate",
          loading: false,
          actionError: null,
          retryPending: true,
        },
    );
    try {
      const receipt = await api.activateTeamMemberLoop(token, teamId, actorId, {
        source_key: intent,
      });
      clearLoopActivationIntent(storageKey, intent);
      if (intentRef.current?.scope === scope && intentRef.current.id === intent)
        intentRef.current = null;
      if (current()) {
        setState(
          (previous) =>
            previous && { ...previous, receipt, retryPending: false },
        );
      }
      return receipt;
    } catch (error) {
      if (current()) {
        setState(
          (previous) =>
            previous && { ...previous, actionError: stringifyApiError(error) },
        );
      }
    } finally {
      if (mutationRef.current === operation) mutationRef.current = null;
      if (current())
        setState((previous) => previous && { ...previous, busy: null });
    }
  }, [scope, storageKey, token, teamId, actorId]);

  return {
    state: state?.scope === scope ? state : null,
    refresh,
    configure,
    activate,
  };
}

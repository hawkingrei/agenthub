import { Button, Group, Stack, Text, TextInput } from "@mantine/core";
import React from "react";
import { api } from "../api";
import { validateRecoveryView, validRecoveryNote, type NativeRecoveryView } from "../native_recovery";

type Props = {
  token?: string;
  agentId: string;
  localSessionId: string | null;
  nativeRuntime: boolean;
  canOperate: boolean;
};

type RecoveryState =
  | { kind: "unread" | "loading" | "saved" }
  | { kind: "ready" | "saving"; view: NativeRecoveryView }
  | { kind: "error"; message: string };

export function NativeRecoveryPanel(props: Props) {
  if (!props.token || !props.localSessionId || !props.nativeRuntime || !props.canOperate) return null;
  return <NativeRecoverySession
    key={JSON.stringify([props.token, props.agentId, props.localSessionId])}
    token={props.token} agentId={props.agentId} localSessionId={props.localSessionId}
  />;
}

function NativeRecoverySession({ token, agentId, localSessionId }: {
  token: string; agentId: string; localSessionId: string;
}) {
  const [state, setState] = React.useState<RecoveryState>({ kind: "unread" });
  const [note, setNote] = React.useState("");
  const mounted = React.useRef(true);
  const busy = React.useRef(false);
  React.useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const load = async () => {
    if (busy.current) return;
    busy.current = true;
    setState({ kind: "loading" });
    setNote("");
    try {
      const view = await api.getNativeRecovery(token, agentId, localSessionId);
      validateRecoveryView(view, localSessionId);
      if (mounted.current) setState({ kind: "ready", view });
    } catch {
      if (mounted.current) setState({ kind: "error", message: "Recovery state is unavailable. Refresh when this session is idle." });
    } finally {
      busy.current = false;
    }
  };

  const reconcile = async () => {
    if (busy.current || state.kind !== "ready" || !state.view.recovery.blocked || !validRecoveryNote(note)) return;
    const { view } = state;
    busy.current = true;
    setState({ kind: "saving", view });
    try {
      await api.reconcileNativeRecovery(token, agentId, localSessionId, {
        runtime_id: view.runtime_id,
        session_id: view.session_id,
        recovery_id: view.recovery.blocked!.recovery_id,
      }, note.trim());
      if (mounted.current) setState({ kind: "saved" });
    } catch {
      // The controller owns an admitted operation through disconnects. Refresh its
      // state before any explicit retry; a failed response is not permission to replay.
      if (mounted.current) setState({ kind: "error", message: "Recovery could not be confirmed. Refresh status before trying again." });
    } finally {
      busy.current = false;
    }
  };

  const pending = state.kind === "loading" || state.kind === "saving";
  const view = state.kind === "ready" || state.kind === "saving" ? state.view : null;
  const uncertain = view?.recovery.decisions.filter(decision => decision.state !== "completed").length ?? 0;
  return (
    <section aria-label="Session recovery" className="shrink-0 border-t border-notion-border px-3 py-2">
      <Stack gap="xs">
        <Group justify="space-between" gap="xs">
          {state.kind !== "unread" && <Text size="sm" fw={600}>Session recovery</Text>}
          <Button size="compact-xs" variant="subtle" loading={state.kind === "loading"} disabled={pending} onClick={() => { void load(); }}>
            {state.kind === "unread" ? "Review recovery" : "Refresh recovery status"}
          </Button>
        </Group>
        {state.kind === "error" && <Text size="sm" c="red" role="alert">{state.message}</Text>}
        {state.kind === "saved" && <Text size="sm" role="status">Recovery review saved. Send a new instruction when ready.</Text>}
        {view && !view.recovery.blocked && <Text size="sm" role="status">
          {view.recovery.waiting_turn_id ? "Answer the current question or approval in the conversation." : "No interrupted work needs reconciliation."}
        </Text>}
        {view?.recovery.blocked && <>
          <Text size="sm">Confirm the previous executor has stopped and review any effects before continuing.</Text>
          {uncertain > 0 && <Text size="sm" c="orange">{uncertain} earlier {uncertain === 1 ? "approval has" : "approvals have"} an unconfirmed outcome.</Text>}
          <TextInput label="Recovery note" description="Record which effects you checked and what is safe to do next."
            value={note} onChange={event => setNote(event.currentTarget.value)} disabled={pending}
            error={note && !validRecoveryNote(note) ? "Use a single line of at most 4096 UTF-8 bytes without control characters." : undefined}
          />
          <Group><Button size="xs" loading={state.kind === "saving"} disabled={pending || !validRecoveryNote(note)} onClick={() => { void reconcile(); }}>
            Confirm recovery review
          </Button></Group>
        </>}
      </Stack>
    </section>
  );
}

import React, { useState } from "react";
import { api } from "../api";
import { ErrorBanner } from "../error_banner";
import { ensurePushSubscription } from "../push";
import {
  publicKeyCredentialCreationOptionsFromJson,
  registerCredentialToJson,
} from "../webauthn";
import { AuthState } from "../types";
import { setLocalStorageItemSafe } from "../storage/safe_storage";

const JOIN_PAGE_CLASS = "app min-h-[var(--agenthub-vh,100vh)] px-4 py-8 md:px-6 md:py-10";
const JOIN_CARD_CLASS =
  "auth mx-auto flex w-full max-w-md flex-col gap-3 rounded-2xl border border-slate-200/80 bg-white/90 p-6 shadow-sm backdrop-blur";
const JOIN_INPUT_CLASS =
  "w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 outline-none transition focus:border-slate-500 focus:ring-2 focus:ring-slate-200";
const JOIN_PRIMARY_BUTTON_CLASS =
  "mt-1 inline-flex items-center justify-center rounded-lg bg-slate-900 px-3 py-2 text-sm font-medium text-white transition hover:bg-slate-800";

export function JoinPage({ onComplete }: { onComplete: (auth: AuthState) => void }) {
  const token = new URLSearchParams(location.search).get("token") || "";
  const [tokenError] = useState(token ? null : "missing join token");
  const [pin, setPin] = useState("");
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [deviceName, setDeviceName] = useState("");
  const [error, setError] = useState<string | null>(null);

  const onJoin = async () => {
    setError(null);
    try {
      const start = await api.joinStart({
        token,
        pin,
        username,
        display_name: displayName,
        password,
        device_name: deviceName || "Device",
      });
      const options = publicKeyCredentialCreationOptionsFromJson(start.options);
      const cred = await navigator.credentials.create({ publicKey: options });
      if (!cred) throw new Error("registration cancelled");
      const payload = registerCredentialToJson(cred as PublicKeyCredential);
      const finish = await api.joinFinish(start.challenge_id, payload);
      const next = {
        token: finish.token,
        userId: finish.user_id,
        username,
        role: "device",
      };
      setLocalStorageItemSafe("agenthub_auth", JSON.stringify(next));
      await ensurePushSubscription(finish.token);
      onComplete(next);
      location.href = "/";
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className={JOIN_PAGE_CLASS}>
      <section className={JOIN_CARD_CLASS}>
        <h2 className="text-xl font-semibold tracking-tight text-slate-900">Join Device</h2>
        {tokenError && <div className="error">{tokenError}</div>}
        {error && <ErrorBanner message={error} onClose={() => setError(null)} />}
        <input
          className={JOIN_INPUT_CLASS}
          placeholder="PIN"
          value={pin}
          onChange={(e) => setPin(e.target.value)}
        />
        <input
          className={JOIN_INPUT_CLASS}
          placeholder="Username"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
        />
        <input
          className={JOIN_INPUT_CLASS}
          placeholder="Display Name"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
        />
        <input
          className={JOIN_INPUT_CLASS}
          placeholder="Password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
        <input
          className={JOIN_INPUT_CLASS}
          placeholder="Device Name"
          value={deviceName}
          onChange={(e) => setDeviceName(e.target.value)}
        />
        <button className={JOIN_PRIMARY_BUTTON_CLASS} onClick={onJoin}>
          Join
        </button>
      </section>
    </div>
  );
}

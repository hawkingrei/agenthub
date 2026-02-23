import React, { useState } from "react";
import { api, parseApiErrorMessage } from "../api";
import { ErrorBanner } from "../error_banner";
import { ensurePushSubscription } from "../push";
import {
  publicKeyCredentialCreationOptionsFromJson,
  registerCredentialToJson,
} from "../webauthn";
import { AuthState } from "../types";
import { setLocalStorageItemSafe } from "../storage/safe_storage";
import {
  AUTH_FORM_CARD_CLASS,
  AUTH_INPUT_CLASS,
  AUTH_PAGE_CLASS,
  AUTH_PRIMARY_BUTTON_CLASS,
} from "../ui/tailwind_classes";

const JOIN_PRIMARY_BUTTON_CLASS = `mt-1 ${AUTH_PRIMARY_BUTTON_CLASS}`;

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
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  };

  return (
    <div className={AUTH_PAGE_CLASS}>
      <section className={AUTH_FORM_CARD_CLASS}>
        <h2 className="text-xl font-semibold tracking-tight text-slate-900">Join Device</h2>
        {tokenError && <div className="error">{tokenError}</div>}
        {error && <ErrorBanner message={error} onClose={() => setError(null)} />}
        <input
          className={AUTH_INPUT_CLASS}
          placeholder="PIN"
          value={pin}
          onChange={(e) => setPin(e.target.value)}
        />
        <input
          className={AUTH_INPUT_CLASS}
          placeholder="Username"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
        />
        <input
          className={AUTH_INPUT_CLASS}
          placeholder="Display Name"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
        />
        <input
          className={AUTH_INPUT_CLASS}
          placeholder="Password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
        <input
          className={AUTH_INPUT_CLASS}
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

import React, { useState } from "react";
import { api } from "../api";
import { ErrorBanner } from "../error_banner";
import { ensurePushSubscription } from "../push";
import {
  publicKeyCredentialCreationOptionsFromJson,
  registerCredentialToJson,
} from "../webauthn";
import { AuthState } from "../types";

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
      localStorage.setItem("agenthub_auth", JSON.stringify(next));
      await ensurePushSubscription(finish.token);
      onComplete(next);
      location.href = "/";
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="app">
      <section className="auth">
        <h2>Join Device</h2>
        {tokenError && <div className="error">{tokenError}</div>}
        {error && <ErrorBanner message={error} onClose={() => setError(null)} />}
        <input placeholder="PIN" value={pin} onChange={(e) => setPin(e.target.value)} />
        <input
          placeholder="Username"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
        />
        <input
          placeholder="Display Name"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
        />
        <input
          placeholder="Password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
        <input
          placeholder="Device Name"
          value={deviceName}
          onChange={(e) => setDeviceName(e.target.value)}
        />
        <button onClick={onJoin}>Join</button>
      </section>
    </div>
  );
}

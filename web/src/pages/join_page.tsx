import { useState } from "react";
import { api, stringifyApiError } from "../api";
import { ErrorBanner } from "../error_banner";
import { ensurePushSubscription } from "../push";
import {
  publicKeyCredentialCreationOptionsFromJson,
  registerCredentialToJson,
} from "../webauthn";
import { AuthState } from "../types";
import { setLocalStorageItemSafe } from "../storage/safe_storage";
import { ActionButton } from "../ui/primitives";
import {
  AUTH_FORM_CARD_CLASS,
  AUTH_INPUT_CLASS,
  AUTH_PAGE_CLASS,
} from "../ui/tailwind_classes";

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

      let next: AuthState;
      if (start.token && start.user_id) {
        next = {
          token: start.token,
          userId: start.user_id,
          username,
          role: "device",
        };
      } else {
        if (!start.challenge_id || !start.options) {
          throw new Error("invalid join response: missing challenge");
        }
        const options = publicKeyCredentialCreationOptionsFromJson(start.options);
        const cred = await navigator.credentials.create({ publicKey: options });
        if (!cred) throw new Error("registration cancelled");
        const payload = registerCredentialToJson(cred as PublicKeyCredential);
        const finish = await api.joinFinish(start.challenge_id, payload);
        next = {
          token: finish.token,
          userId: finish.user_id,
          username,
          role: "device",
        };
      }

      setLocalStorageItemSafe("agenthub_auth", JSON.stringify(next));
      await ensurePushSubscription(next.token);
      onComplete(next);
      location.href = "/";
    } catch (err) {
      setError(stringifyApiError(err));
    }
  };

  return (
    <div className={AUTH_PAGE_CLASS}>
      <form
        className={AUTH_FORM_CARD_CLASS}
        onSubmit={(event) => {
          event.preventDefault();
          void onJoin();
        }}
      >
        <h2 className="text-xl font-bold tracking-tight text-notion-text">Join Device</h2>
        {tokenError && <div className="error">{tokenError}</div>}
        {error && <ErrorBanner message={error} onClose={() => setError(null)} />}
        <input
          className={AUTH_INPUT_CLASS}
          id="join-pin"
          name="pin"
          placeholder="PIN"
          value={pin}
          autoComplete="one-time-code"
          onChange={(e) => setPin(e.target.value)}
        />
        <input
          className={AUTH_INPUT_CLASS}
          id="join-username"
          name="username"
          placeholder="Username"
          value={username}
          autoComplete="username"
          onChange={(e) => setUsername(e.target.value)}
        />
        <input
          className={AUTH_INPUT_CLASS}
          id="join-display-name"
          name="display_name"
          placeholder="Display Name"
          value={displayName}
          autoComplete="name"
          onChange={(e) => setDisplayName(e.target.value)}
        />
        <input
          className={AUTH_INPUT_CLASS}
          id="join-password"
          name="password"
          placeholder="Password"
          type="password"
          value={password}
          autoComplete="new-password"
          onChange={(e) => setPassword(e.target.value)}
        />
        <input
          className={AUTH_INPUT_CLASS}
          id="join-device-name"
          name="device_name"
          placeholder="Device Name"
          value={deviceName}
          autoComplete="organization"
          onChange={(e) => setDeviceName(e.target.value)}
        />
        <ActionButton tone="primary" size="md" type="submit" className="mt-1">
          Join
        </ActionButton>
      </form>
    </div>
  );
}

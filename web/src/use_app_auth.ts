import { useState, useCallback } from "react";
import { AuthState } from "./types";
import { getLocalStorageItemSafe, removeLocalStorageItemSafe, setLocalStorageItemSafe } from "./storage/safe_storage";
import { api, stringifyApiError } from "./api";
import { ensurePushSubscription } from "./push";
import { loginCredentialToJson, publicKeyCredentialCreationOptionsFromJson, publicKeyCredentialRequestOptionsFromJson, registerCredentialToJson } from "./webauthn";

export function useAppAuth() {
  const [auth, setAuth] = useState<AuthState | null>(() => {
    const raw = getLocalStorageItemSafe("agenthub_auth");
    if (!raw) return null;
    try {
      return JSON.parse(raw) as AuthState;
    } catch {
      removeLocalStorageItemSafe("agenthub_auth");
      return null;
    }
  });
  const [authBusy, setAuthBusy] = useState<"login" | "register" | null>(null);
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);

  const onRegister = useCallback(async (role?: string) => {
    if (authBusy) return;
    setAuthBusy("register");
    setError(null);
    try {
      const start = await api.registerStart(
        username,
        displayName,
        role,
        role === "root" ? password : undefined
      );

      let next: AuthState;
      if (start.token && start.user_id) {
        next = {
          token: start.token,
          userId: start.user_id,
          username,
          role: start.role ?? role ?? "device",
        };
      } else {
        if (!start.challenge_id || !start.options) {
          throw new Error("invalid registration response: missing challenge");
        }
        const options = publicKeyCredentialCreationOptionsFromJson(start.options);
        const cred = await navigator.credentials.create({ publicKey: options });
        if (!cred) throw new Error("registration cancelled");
        const payload = registerCredentialToJson(cred as PublicKeyCredential);
        const finish = await api.registerFinish(start.challenge_id, payload);
        next = {
          token: finish.token,
          userId: finish.user_id,
          username,
          role: finish.role,
        };
      }

      setLocalStorageItemSafe("agenthub_auth", JSON.stringify(next));
      setAuth(next);
      await ensurePushSubscription(next.token);
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    } finally {
      setAuthBusy(null);
    }
  }, [authBusy, username, displayName, password]);

  const onLogin = useCallback(async () => {
    if (authBusy) return;
    setAuthBusy("login");
    setError(null);
    try {
      const start = await api.loginStart(username, password);

      let next: AuthState;
      if (start.token && start.user_id) {
        next = {
          token: start.token,
          userId: start.user_id,
          username,
          role: start.role ?? "unknown",
        };
      } else if (start.registration_options) {
        if (!start.challenge_id) {
          throw new Error("invalid login response: missing challenge for registration");
        }
        const options = publicKeyCredentialCreationOptionsFromJson(start.registration_options);
        const cred = await navigator.credentials.create({ publicKey: options });
        if (!cred) throw new Error("registration cancelled");
        const payload = registerCredentialToJson(cred as PublicKeyCredential);
        const finish = await api.loginRegisterFinish(start.challenge_id, payload);
        next = {
          token: finish.token,
          userId: finish.user_id,
          username,
          role: finish.role,
        };
      } else {
        if (!start.challenge_id || !start.options) {
          throw new Error("invalid login response: missing challenge");
        }
        const options = publicKeyCredentialRequestOptionsFromJson(start.options);
        const cred = await navigator.credentials.get({ publicKey: options });
        if (!cred) throw new Error("login cancelled");
        const payload = loginCredentialToJson(cred as PublicKeyCredential);
        const finish = await api.loginFinish(start.challenge_id, payload);
        next = {
          token: finish.token,
          userId: finish.user_id,
          username,
          role: finish.role,
        };
      }

      setLocalStorageItemSafe("agenthub_auth", JSON.stringify(next));
      setAuth(next);
      await ensurePushSubscription(next.token);
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    } finally {
      setAuthBusy(null);
    }
  }, [authBusy, username, password]);

  const onLogout = useCallback(() => {
    removeLocalStorageItemSafe("agenthub_auth");
    setAuth(null);
  }, []);

  return {
    auth,
    setAuth,
    authBusy,
    username,
    setUsername,
    displayName,
    setDisplayName,
    password,
    setPassword,
    error,
    setError,
    onRegister,
    onLogin,
    onLogout,
  };
}

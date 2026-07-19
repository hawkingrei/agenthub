import React from "react";
import { ActionButton } from "../ui/primitives";
import {
  AUTH_ACTIONS_CLASS,
  AUTH_FORM_CARD_CLASS,
  AUTH_INPUT_CLASS,
  AUTH_PRIMARY_BUTTON_CLASS,
  AUTH_SECONDARY_BUTTON_CLASS,
} from "../ui/tailwind_classes";

export type LoginViewProps = {
  authBusy: "login" | "register" | null;
  rootInitialized: boolean | null;
  username: string;
  password: string;
  displayName: string;
  setUsername: (value: string) => void;
  setPassword: (value: string) => void;
  setDisplayName: (value: string) => void;
  onLogin: () => Promise<void>;
  onRegister: (role: string) => Promise<void>;
};

export const LoginView = React.memo(function LoginView({
  authBusy,
  rootInitialized,
  username,
  password,
  displayName,
  setUsername,
  setPassword,
  setDisplayName,
  onLogin,
  onRegister,
}: LoginViewProps) {
  if (rootInitialized === null) {
    return (
      <div className={AUTH_FORM_CARD_CLASS} aria-busy="true">
        <h2 className="text-xl font-bold tracking-tight text-notion-text">
          Checking setup
        </h2>
      </div>
    );
  }

  const isFirstRun = rootInitialized === false;

  return (
    <form
      className={AUTH_FORM_CARD_CLASS}
      onSubmit={(event) => {
        event.preventDefault();
        if (isFirstRun) {
          void onRegister("root");
        } else {
          void onLogin();
        }
      }}
    >
      <div className="space-y-2">
        <div className="flex flex-wrap items-center gap-2">
          <h2 className="text-xl font-bold tracking-tight text-notion-text">
            {isFirstRun ? "First-run setup" : "Login"}
          </h2>
          {isFirstRun ? (
            <span className="rounded-md border border-state-warning-border bg-state-warning-bg px-2 py-0.5 text-[10px] font-bold uppercase text-state-warning-text">
              Root required
            </span>
          ) : null}
        </div>
        {isFirstRun ? (
          <p className="text-[13px] leading-relaxed text-notion-text-muted">
            Create the first root operator for this AgentHub instance. Runtime role
            and provider credentials still stay in the instance config.
          </p>
        ) : null}
      </div>
      <input
        className={AUTH_INPUT_CLASS}
        id="login-username"
        name="username"
        placeholder="Username"
        value={username}
        disabled={authBusy !== null}
        autoComplete="username"
        onChange={(e) => setUsername(e.target.value)}
      />
      <input
        className={AUTH_INPUT_CLASS}
        id="login-password"
        name="password"
        placeholder="Password"
        type="password"
        value={password}
        disabled={authBusy !== null}
        autoComplete="current-password"
        onChange={(e) => setPassword(e.target.value)}
      />
      {rootInitialized === false ? (
        <div className="space-y-3">
          <input
            className={AUTH_INPUT_CLASS}
            id="login-display-name"
            name="display_name"
            placeholder="Display Name"
            value={displayName}
            disabled={authBusy !== null}
            autoComplete="name"
            onChange={(e) => setDisplayName(e.target.value)}
          />
          <dl className="grid gap-2 rounded-lg border border-notion-border bg-notion-sidebar/30 p-3 text-[12px] leading-relaxed">
            <div className="grid gap-1 sm:grid-cols-[6.5rem_minmax(0,1fr)]">
              <dt className="font-bold uppercase text-notion-text-muted">Scope</dt>
              <dd className="text-notion-text">Root account bootstrap</dd>
            </div>
            <div className="grid gap-1 sm:grid-cols-[6.5rem_minmax(0,1fr)]">
              <dt className="font-bold uppercase text-notion-text-muted">Config</dt>
              <dd className="text-notion-text">
                Server role and provider credentials remain operator-managed.
              </dd>
            </div>
          </dl>
        </div>
      ) : null}
      <div className={AUTH_ACTIONS_CLASS}>
        {rootInitialized === false ? (
          <ActionButton
            tone="secondary"
            className={AUTH_SECONDARY_BUTTON_CLASS}
            disabled={authBusy !== null}
            onClick={() => onRegister("root")}
          >
            {authBusy === "register" ? "Bootstrapping..." : "Initialize Root"}
          </ActionButton>
        ) : null}
        <ActionButton
          tone="primary"
          type="submit"
          className={AUTH_PRIMARY_BUTTON_CLASS}
          disabled={authBusy !== null}
        >
          {authBusy === "login" ? "Logging in..." : "Login"}
        </ActionButton>
      </div>
    </form>
  );
});

LoginView.displayName = "LoginView";

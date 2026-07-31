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
  const isFirstRun = rootInitialized === false;

  return (
    <form
      className={AUTH_FORM_CARD_CLASS}
      onSubmit={(event) => {
        event.preventDefault();
        void onLogin();
      }}
    >
      <div className="space-y-2">
        <p className="text-xs font-bold uppercase text-notion-text-muted">
          {isFirstRun ? "First-run setup" : "AgentHub"}
        </p>
        <h2 className="text-xl font-bold tracking-tight text-notion-text">
          {isFirstRun ? "Initialize this AgentHub instance" : "Login"}
        </h2>
        {isFirstRun ? (
          <p className="text-sm leading-6 text-notion-text-muted">
            Create the root account that controls this web instance. Runtime
            role, network listeners, and provider credentials stay in the local
            instance configuration.
          </p>
        ) : null}
      </div>
      {isFirstRun ? (
        <div className="space-y-3 border-y border-notion-border bg-notion-sidebar px-4 py-3 text-sm text-notion-text">
          <div>
            <p className="font-semibold">Root bootstrap</p>
            <p className="mt-1 text-notion-text-muted">
              This step creates the first operator account for browser access.
            </p>
          </div>
          <div>
            <p className="font-semibold">Instance configuration</p>
            <p className="mt-1 text-notion-text-muted">
              Use agenthub init or ~/.agenthub/config.toml for server role,
              internal gRPC, and provider API endpoints or keys.
            </p>
          </div>
        </div>
      ) : null}
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
      {isFirstRun ? (
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
      ) : null}
      <div className={AUTH_ACTIONS_CLASS}>
        {isFirstRun ? (
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

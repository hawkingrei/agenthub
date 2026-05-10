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
  return (
    <form
      className={AUTH_FORM_CARD_CLASS}
      onSubmit={(event) => {
        event.preventDefault();
        void onLogin();
      }}
    >
      <h2 className="text-xl font-bold tracking-tight text-notion-text">
        Login
      </h2>
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

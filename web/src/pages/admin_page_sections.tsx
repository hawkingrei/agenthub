import { ActionButton } from "../ui/primitives";
import {
  ADMIN_CARD_CLASS,
  ADMIN_CARD_TITLE_CLASS,
  ADMIN_DANGER_BUTTON_CLASS,
  ADMIN_EMPTY_TEXT_CLASS,
  ADMIN_FORM_ROW_CLASS,
  ADMIN_INPUT_CLASS,
  ADMIN_KV_LIST_CLASS,
  ADMIN_KV_ROW_CLASS,
  ADMIN_LABEL_CLASS,
  ADMIN_LIST_CLASS,
  ADMIN_LIST_ITEM_CLASS,
  ADMIN_MUTED_TEXT_CLASS,
  ADMIN_PRIMARY_BUTTON_CLASS,
  ADMIN_SECONDARY_BUTTON_CLASS,
  ADMIN_VALUE_CLASS,
} from "../ui/tailwind_classes";
import type {
  AdminAuditsSectionProps,
  AdminDevicesSectionProps,
  AdminJoinSectionProps,
  AdminLinkersSectionProps,
  AdminSafePathsSectionProps,
  AdminSystemSectionProps,
  AdminUiSectionProps,
  AdminVapidSectionProps,
} from "./admin_page";

export function AdminSafePathsSection({
  safePaths: section,
}: {
  safePaths: AdminSafePathsSectionProps;
}) {
  return (
    <div className={ADMIN_CARD_CLASS}>
      <h3 className={ADMIN_CARD_TITLE_CLASS}>Safe Paths</h3>
      <div className={ADMIN_FORM_ROW_CLASS}>
        <input
          className={ADMIN_INPUT_CLASS}
          placeholder="Add safe path"
          value={section.safePathInput}
          onChange={(e) => section.setSafePathInput(e.target.value)}
        />
        <ActionButton
          className={ADMIN_PRIMARY_BUTTON_CLASS}
          tone="primary"
          size="md"
          onClick={section.onAddSafePath}
        >
          Add Path
        </ActionButton>
      </div>
      <div className={ADMIN_FORM_ROW_CLASS}>
        <label className="checkbox inline-flex items-center gap-2 text-sm text-slate-700">
          <input
            type="checkbox"
            checked={
              section.safePaths.length > 0 &&
              section.safePaths.every((path) =>
                section.selectedSafePaths.has(path.path)
              )
            }
            onChange={section.onToggleAllSafePaths}
          />
          Select All
        </label>
        <ActionButton
          className={ADMIN_DANGER_BUTTON_CLASS}
          tone="danger"
          size="md"
          onClick={section.onDeleteSelectedSafePaths}
        >
          Delete Selected
        </ActionButton>
      </div>
      <ul className={ADMIN_LIST_CLASS}>
        {section.safePaths.map((path) => (
          <li className={ADMIN_LIST_ITEM_CLASS} key={path.path}>
            <label className="checkbox inline-flex items-center">
              <input
                type="checkbox"
                checked={section.selectedSafePaths.has(path.path)}
                onChange={() => section.onToggleSafePath(path.path)}
              />
            </label>
            <span className="mono flex-1 text-sm text-slate-800">{path.path}</span>
            <ActionButton
              className={ADMIN_DANGER_BUTTON_CLASS}
              tone="danger"
              size="md"
              onClick={() => section.onDeleteSafePath(path.path)}
            >
              Delete
            </ActionButton>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function AdminJoinSection({
  join: section,
  joinLinkCopyState,
  onCopyJoinLink,
}: {
  join: AdminJoinSectionProps;
  joinLinkCopyState: "idle" | "copied" | "failed";
  onCopyJoinLink: () => void;
}) {
  return (
    <div className={`${ADMIN_CARD_CLASS} join-card`}>
      <h3 className={ADMIN_CARD_TITLE_CLASS}>Join Device</h3>
      <p className={ADMIN_MUTED_TEXT_CLASS}>
        Use the token/link below on the destination browser. QR onboarding is no longer
        required.
      </p>
      {section.joinUrl ? (
        <div className="flex flex-wrap items-center gap-2">
          <a
            className={`${ADMIN_MUTED_TEXT_CLASS} break-all underline underline-offset-2`}
            href={section.joinUrl}
          >
            Join link: {section.joinUrl}
          </a>
          <ActionButton tone="secondary" size="sm" onClick={onCopyJoinLink}>
            {joinLinkCopyState === "copied"
              ? "Copied"
              : joinLinkCopyState === "failed"
                ? "Copy failed"
                : "Copy link"}
          </ActionButton>
        </div>
      ) : null}
      {section.joinToken ? (
        <p className={ADMIN_MUTED_TEXT_CLASS}>Token: {section.joinToken}</p>
      ) : null}
      {section.joinPin ? <p className={ADMIN_MUTED_TEXT_CLASS}>PIN: {section.joinPin}</p> : null}
    </div>
  );
}

export function AdminVapidSection({
  vapid: section,
}: {
  vapid: AdminVapidSectionProps;
}) {
  return (
    <div className={ADMIN_CARD_CLASS}>
      <h3 className={ADMIN_CARD_TITLE_CLASS}>VAPID Keys</h3>
      {section.vapidInfo ? (
        <div className={ADMIN_KV_LIST_CLASS}>
          <div className={ADMIN_KV_ROW_CLASS}>
            <span className={ADMIN_LABEL_CLASS}>Subject</span>
            <span className={ADMIN_VALUE_CLASS}>{section.vapidInfo.subject}</span>
          </div>
          <div className={ADMIN_KV_ROW_CLASS}>
            <span className={ADMIN_LABEL_CLASS}>Public Key</span>
            <span className={`${ADMIN_VALUE_CLASS} mono`}>
              {section.vapidInfo.public_key}
            </span>
          </div>
          <div className={ADMIN_KV_ROW_CLASS}>
            <span className={ADMIN_LABEL_CLASS}>Keys Path</span>
            <span className={`${ADMIN_VALUE_CLASS} mono`}>
              {section.vapidInfo.keys_path}
            </span>
          </div>
        </div>
      ) : (
        <p className={ADMIN_EMPTY_TEXT_CLASS}>VAPID keys not loaded.</p>
      )}
      <div className={ADMIN_FORM_ROW_CLASS}>
        <ActionButton
          className={ADMIN_SECONDARY_BUTTON_CLASS}
          tone="secondary"
          size="md"
          onClick={section.onRotateVapid}
        >
          Rotate Keys
        </ActionButton>
      </div>
    </div>
  );
}

export function AdminLinkersSection({
  linkers: section,
}: {
  linkers: AdminLinkersSectionProps;
}) {
  const principal = section.slockLinker?.principal ?? null;
  return (
    <div className={ADMIN_CARD_CLASS}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-3">
          <img
            src="/slock-icon.png"
            alt=""
            aria-hidden="true"
            className="h-9 w-9 shrink-0 rounded-lg border border-notion-border bg-white p-1 shadow-sm"
            decoding="async"
          />
          <div className="min-w-0">
            <h3 className={ADMIN_CARD_TITLE_CLASS}>Slock Linker</h3>
            <p className={ADMIN_MUTED_TEXT_CLASS}>
              Server-side Slock OAuth connection for AgentHub-controlled tools.
            </p>
          </div>
        </div>
        <span className="rounded-md border border-notion-border bg-notion-sidebar/50 px-2 py-1 text-xs font-semibold uppercase text-notion-muted">
          {section.slockLinker?.status ?? "Not configured"}
        </span>
      </div>

      <div className="mt-4 grid gap-3 md:grid-cols-2">
        <label className="flex flex-col gap-1">
          <span className={ADMIN_LABEL_CLASS}>API Origin</span>
          <input
            className={ADMIN_INPUT_CLASS}
            value={section.slockApiOrigin}
            onChange={(event) => section.setSlockApiOrigin(event.currentTarget.value)}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className={ADMIN_LABEL_CLASS}>Client ID</span>
          <input
            className={ADMIN_INPUT_CLASS}
            value={section.slockClientId}
            onChange={(event) => section.setSlockClientId(event.currentTarget.value)}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className={ADMIN_LABEL_CLASS}>Client Secret</span>
          <input
            className={ADMIN_INPUT_CLASS}
            type="password"
            placeholder={
              section.slockLinker?.client_secret_configured
                ? "Configured"
                : "Required"
            }
            value={section.slockClientSecret}
            onChange={(event) => section.setSlockClientSecret(event.currentTarget.value)}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className={ADMIN_LABEL_CLASS}>Scopes</span>
          <input
            className={ADMIN_INPUT_CLASS}
            value={section.slockScopesInput}
            onChange={(event) => section.setSlockScopesInput(event.currentTarget.value)}
          />
        </label>
        <label className="flex flex-col gap-1 md:col-span-2">
          <span className={ADMIN_LABEL_CLASS}>Return URL</span>
          <input
            className={ADMIN_INPUT_CLASS}
            value={section.slockReturnUrl}
            onChange={(event) => section.setSlockReturnUrl(event.currentTarget.value)}
          />
        </label>
      </div>

      <div className={ADMIN_FORM_ROW_CLASS}>
        <ActionButton
          className={ADMIN_PRIMARY_BUTTON_CLASS}
          tone="primary"
          size="md"
          onClick={section.onSaveSlockLinker}
        >
          Save Slock
        </ActionButton>
        <ActionButton
          className={ADMIN_SECONDARY_BUTTON_CLASS}
          tone="secondary"
          size="md"
          onClick={section.onCreateSlockLinkAttempt}
        >
          Create Link Attempt
        </ActionButton>
      </div>

      {section.slockLinkAttempt ? (
        <div className={`${ADMIN_KV_LIST_CLASS} mt-3`}>
          <div className={ADMIN_KV_ROW_CLASS}>
            <span className={ADMIN_LABEL_CLASS}>State</span>
            <span className={`${ADMIN_VALUE_CLASS} mono`}>
              {section.slockLinkAttempt.state}
            </span>
          </div>
          <div className={ADMIN_KV_ROW_CLASS}>
            <span className={ADMIN_LABEL_CLASS}>Expires</span>
            <span className={ADMIN_VALUE_CLASS}>
              {new Date(section.slockLinkAttempt.expires_at * 1000).toLocaleString()}
            </span>
          </div>
        </div>
      ) : null}

      <div className="mt-4 flex flex-col gap-2">
        <label className="flex flex-col gap-1">
          <span className={ADMIN_LABEL_CLASS}>Callback URL Or Code</span>
          <input
            className={ADMIN_INPUT_CLASS}
            value={section.slockCallbackInput}
            onChange={(event) => section.setSlockCallbackInput(event.currentTarget.value)}
          />
        </label>
        <div className={ADMIN_FORM_ROW_CLASS}>
          <ActionButton
            className={ADMIN_SECONDARY_BUTTON_CLASS}
            tone="secondary"
            size="md"
            onClick={section.onExchangeSlockCode}
          >
            Exchange Code
          </ActionButton>
        </div>
      </div>

      {principal ? (
        <div className={`${ADMIN_KV_LIST_CLASS} mt-3`}>
          <div className={ADMIN_KV_ROW_CLASS}>
            <span className={ADMIN_LABEL_CLASS}>Principal</span>
            <span className={ADMIN_VALUE_CLASS}>{principal.display_name}</span>
          </div>
          <div className={ADMIN_KV_ROW_CLASS}>
            <span className={ADMIN_LABEL_CLASS}>Type</span>
            <span className={ADMIN_VALUE_CLASS}>{principal.principal_type}</span>
          </div>
          <div className={ADMIN_KV_ROW_CLASS}>
            <span className={ADMIN_LABEL_CLASS}>Subject</span>
            <span className={`${ADMIN_VALUE_CLASS} mono`}>{principal.subject}</span>
          </div>
          <div className={ADMIN_KV_ROW_CLASS}>
            <span className={ADMIN_LABEL_CLASS}>Server</span>
            <span className={ADMIN_VALUE_CLASS}>
              {[principal.server_slug, principal.server_id].filter(Boolean).join(" / ") ||
                "-"}
            </span>
          </div>
        </div>
      ) : (
        <p className={`${ADMIN_EMPTY_TEXT_CLASS} mt-3`}>
          Slock identity has not been linked.
        </p>
      )}
    </div>
  );
}

function AdminToggleField({
  checked,
  disabled = false,
  label,
  description,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  label: string;
  description: string;
  onChange: (value: boolean) => void;
}) {
  return (
    <label
      className={`flex items-start gap-3 rounded-xl border border-notion-border bg-notion-sidebar/20 px-4 py-3 ${
        disabled ? "opacity-60" : "cursor-pointer hover:bg-notion-hover/50"
      }`}
    >
      <input
        type="checkbox"
        className="mt-0.5 h-4 w-4 rounded border-notion-border text-notion-accent focus:ring-notion-accent/20"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      <span className="min-w-0 space-y-1">
        <span className="block text-sm font-semibold text-notion-text">{label}</span>
        <span className={ADMIN_MUTED_TEXT_CLASS}>{description}</span>
      </span>
    </label>
  );
}

export function AdminDevicesSection({
  devices: section,
}: {
  devices: AdminDevicesSectionProps;
}) {
  return (
    <div className={ADMIN_CARD_CLASS}>
      <h3 className={ADMIN_CARD_TITLE_CLASS}>Devices</h3>
      <ul className={ADMIN_LIST_CLASS}>
        {section.devices.map((device) => (
          <li className={ADMIN_LIST_ITEM_CLASS} key={device.id}>
            <span className="text-sm text-slate-800">
              {device.name} - {device.status}
            </span>
            {device.status === "active" ? (
              <ActionButton
                className={ADMIN_DANGER_BUTTON_CLASS}
                tone="danger"
                size="md"
                onClick={() => section.onRevokeDevice(device.id)}
              >
                Revoke
              </ActionButton>
            ) : null}
          </li>
        ))}
      </ul>
    </div>
  );
}

export function AdminAuditsSection({
  audits: section,
}: {
  audits: AdminAuditsSectionProps;
}) {
  return (
    <div className={ADMIN_CARD_CLASS}>
      <h3 className={ADMIN_CARD_TITLE_CLASS}>Login Audits</h3>
      <ul className={ADMIN_LIST_CLASS}>
        {section.audits.map((audit) => (
          <li className={ADMIN_LIST_ITEM_CLASS} key={audit.id}>
            <span className={ADMIN_MUTED_TEXT_CLASS}>
              {new Date(audit.ts * 1000).toLocaleString()} - {` ${audit.event}`}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function AdminUiSection({
  ui: section,
}: {
  ui: AdminUiSectionProps;
}) {
  return (
    <div className={ADMIN_CARD_CLASS}>
      <h3 className={ADMIN_CARD_TITLE_CLASS}>UI Settings</h3>
      <div className="flex flex-col gap-3">
        <AdminToggleField
          checked={section.developerMode}
          onChange={section.onDeveloperModeChange}
          label="Developer Mode"
          description="Applies to this browser only. Affects Agents and Teams."
        />
        <p className={ADMIN_MUTED_TEXT_CLASS}>
          Default behavior: enabled in development and tests, disabled in production builds.
        </p>
      </div>
    </div>
  );
}

export function AdminSystemSection({
  system: section,
}: {
  system: AdminSystemSectionProps;
}) {
  return (
    <div className={ADMIN_CARD_CLASS}>
      <h3 className={ADMIN_CARD_TITLE_CLASS}>System Configuration</h3>
      <div className="flex flex-col gap-3">
        <AdminToggleField
          checked={section.passkeyEnabled ?? false}
          disabled={section.passkeyEnabled === null}
          onChange={section.onPasskeyEnabledChange}
          label="Enable Passkey"
          description={
            section.passkeyEnabled === null
              ? "Loading configuration..."
              : "Global setting. When disabled, only password login/registration is allowed."
          }
        />
      </div>
    </div>
  );
}

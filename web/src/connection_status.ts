export type SseConnectionState =
  | "idle"
  | "connecting"
  | "connected"
  | "reconnecting";

export type ConnectionBadge = {
  label: string;
  title: string;
  tone: "ok" | "warn" | "bad" | "muted";
};

export const OFFLINE_MESSAGE = "Offline. Unable to connect to server.";
export const UPSTREAM_HTML_MESSAGE =
  "Connection unavailable (gateway response). Reconnecting...";
const GENERIC_REQUEST_MESSAGE = "Request failed.";
const MAX_ERROR_TEXT_LENGTH = 240;

export function deriveConnectionBadge(
  networkOnline: boolean,
  hasStreamTarget: boolean,
  sseState: SseConnectionState
): ConnectionBadge {
  if (!networkOnline) {
    return {
      label: "Offline · SSE disconnected",
      title: "Network offline. SSE is disconnected.",
      tone: "bad",
    };
  }
  if (!hasStreamTarget) {
    return {
      label: "Online · SSE idle",
      title: "Network online. No active SSE stream target.",
      tone: "muted",
    };
  }
  switch (sseState) {
    case "connected":
      return {
        label: "Online · SSE connected",
        title: "Network online. SSE is connected.",
        tone: "ok",
      };
    case "connecting":
      return {
        label: "Online · SSE connecting",
        title: "Network online. SSE is connecting.",
        tone: "warn",
      };
    case "reconnecting":
      return {
        label: "Online · SSE reconnecting",
        title: "Network online. SSE is reconnecting.",
        tone: "warn",
      };
    case "idle":
    default:
      return {
        label: "Online · SSE idle",
        title: "Network online. SSE is idle.",
        tone: "muted",
      };
  }
}

export function sanitizeErrorBannerMessage(
  rawMessage: string,
  networkOnline: boolean
): string {
  const compact = normalizeErrorText(rawMessage);
  if (!compact) {
    return networkOnline ? GENERIC_REQUEST_MESSAGE : OFFLINE_MESSAGE;
  }
  if (isLikelyHtmlErrorDocument(compact)) {
    return networkOnline ? UPSTREAM_HTML_MESSAGE : OFFLINE_MESSAGE;
  }
  if (!networkOnline && isLikelyConnectivityError(compact)) {
    return OFFLINE_MESSAGE;
  }
  return compact.length > MAX_ERROR_TEXT_LENGTH
    ? `${compact.slice(0, MAX_ERROR_TEXT_LENGTH)}...`
    : compact;
}

export function shouldHideErrorBannerMessage(message: string): boolean {
  return normalizeErrorText(message) === UPSTREAM_HTML_MESSAGE;
}

function normalizeErrorText(rawMessage: string): string {
  return rawMessage.replace(/\s+/g, " ").trim();
}

function isLikelyHtmlErrorDocument(message: string): boolean {
  const lower = message.toLowerCase();
  if (lower.startsWith("<!doctype html")) return true;
  if (lower.includes("</html>")) return true;
  return /<(?:html|body|head|title|script|style|meta|link)\b/i.test(message);
}

function isLikelyConnectivityError(message: string): boolean {
  const lower = message.toLowerCase();
  return (
    lower.includes("failed to fetch") ||
    lower.includes("fetch failed") ||
    lower.includes("networkerror") ||
    lower.includes("network error") ||
    lower.includes("failed to connect") ||
    lower.includes("could not connect") ||
    lower.includes("cannot connect") ||
    lower.includes("connection refused") ||
    lower.includes("connection reset") ||
    lower.includes("connection timed out") ||
    lower.includes("connection timeout") ||
    lower.includes("timeout") ||
    lower.includes("gateway") ||
    lower.includes("cloudflare") ||
    lower.includes("service unavailable") ||
    lower.includes("bad gateway") ||
    lower.includes("sse")
  );
}

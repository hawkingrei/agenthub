import { describe, expect, it } from "vitest";
import {
  deriveConnectionBadge,
  OFFLINE_MESSAGE,
  sanitizeErrorBannerMessage,
  UPSTREAM_HTML_MESSAGE,
} from "./connection_status";

describe("connection badge derivation", () => {
  it("returns offline badge when network is down", () => {
    const badge = deriveConnectionBadge(false, true, "connected");
    expect(badge.label).toBe("Offline · SSE disconnected");
    expect(badge.tone).toBe("bad");
  });

  it("returns idle badge when no stream target exists", () => {
    const badge = deriveConnectionBadge(true, false, "connected");
    expect(badge.label).toBe("Online · SSE idle");
    expect(badge.tone).toBe("muted");
  });

  it("returns connected badge when stream is connected", () => {
    const badge = deriveConnectionBadge(true, true, "connected");
    expect(badge.label).toBe("Online · SSE connected");
    expect(badge.tone).toBe("ok");
  });

  it("returns reconnecting badge while stream retries", () => {
    const badge = deriveConnectionBadge(true, true, "reconnecting");
    expect(badge.label).toBe("Online · SSE reconnecting");
    expect(badge.tone).toBe("warn");
  });

  it("returns connecting badge while stream opens", () => {
    const badge = deriveConnectionBadge(true, true, "connecting");
    expect(badge.label).toBe("Online · SSE connecting");
    expect(badge.tone).toBe("warn");
  });
});

describe("error banner sanitization", () => {
  it("maps offline connectivity errors to a stable message", () => {
    const message = sanitizeErrorBannerMessage("failed to fetch", false);
    expect(message).toBe(OFFLINE_MESSAGE);
  });

  it("keeps non-connectivity validation errors while offline", () => {
    const message = sanitizeErrorBannerMessage("workdir is required", false);
    expect(message).toBe("workdir is required");
  });

  it("maps html gateway responses to a compact reconnect message", () => {
    const html = "<!doctype html><html><head><title>Cloudflare</title></head><body>error</body></html>";
    const message = sanitizeErrorBannerMessage(html, true);
    expect(message).toBe(UPSTREAM_HTML_MESSAGE);
  });

  it("does not treat non-tag substrings as html documents", () => {
    const message = sanitizeErrorBannerMessage(
      "error<bodyguard> is not an html page",
      true
    );
    expect(message).toBe("error<bodyguard> is not an html page");
  });

  it("keeps non-network business errors that mention connection", () => {
    const message = sanitizeErrorBannerMessage(
      "database connection pool exhausted",
      false
    );
    expect(message).toBe("database connection pool exhausted");
  });

  it("returns generic request failure for empty online errors", () => {
    const message = sanitizeErrorBannerMessage("", true);
    expect(message).toBe("Request failed.");
  });

  it("returns offline message for whitespace-only offline errors", () => {
    const message = sanitizeErrorBannerMessage("   \n\t  ", false);
    expect(message).toBe(OFFLINE_MESSAGE);
  });

  it("truncates long plain text errors", () => {
    const message = sanitizeErrorBannerMessage("x".repeat(300), true);
    expect(message.endsWith("...")).toBe(true);
    expect(message.length).toBeLessThanOrEqual(243);
  });
});

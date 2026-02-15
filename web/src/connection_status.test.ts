import { describe, expect, it } from "vitest";
import {
  deriveConnectionBadge,
  sanitizeErrorBannerMessage,
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
});

describe("error banner sanitization", () => {
  it("maps offline connectivity errors to a stable message", () => {
    const message = sanitizeErrorBannerMessage("failed to fetch", false);
    expect(message).toBe("Offline. Unable to connect to server.");
  });

  it("keeps non-connectivity validation errors while offline", () => {
    const message = sanitizeErrorBannerMessage("workdir is required", false);
    expect(message).toBe("workdir is required");
  });

  it("maps html gateway responses to a compact reconnect message", () => {
    const html = "<!doctype html><html><head><title>Cloudflare</title></head><body>error</body></html>";
    const message = sanitizeErrorBannerMessage(html, true);
    expect(message).toBe(
      "Connection unavailable (gateway response). Reconnecting..."
    );
  });

  it("truncates long plain text errors", () => {
    const message = sanitizeErrorBannerMessage("x".repeat(300), true);
    expect(message.endsWith("...")).toBe(true);
    expect(message.length).toBeLessThanOrEqual(243);
  });
});

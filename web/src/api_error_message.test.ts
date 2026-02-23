import { describe, expect, it } from "vitest";
import { parseApiErrorMessage } from "./api";

describe("parseApiErrorMessage", () => {
  it("extracts error field from Error JSON message", () => {
    expect(parseApiErrorMessage(new Error("{\"error\":\"user not found\"}"))).toBe(
      "user not found"
    );
  });

  it("extracts error field from raw JSON string", () => {
    expect(parseApiErrorMessage("{\"error\":\"forbidden\"}")).toBe("forbidden");
  });

  it("keeps plain text error messages", () => {
    expect(parseApiErrorMessage(new Error("request failed"))).toBe("request failed");
  });

  it("reads object error field", () => {
    expect(parseApiErrorMessage({ error: "denied" })).toBe("denied");
  });

  it("reads object message field and parses JSON-shaped payload", () => {
    expect(parseApiErrorMessage({ message: "{\"error\":\"bad credentials\"}" })).toBe(
      "bad credentials"
    );
  });

  it("returns null for unsupported object shape", () => {
    expect(parseApiErrorMessage({ detail: "x" })).toBeNull();
  });

  it("returns null for empty and nullish inputs", () => {
    expect(parseApiErrorMessage(new Error(""))).toBeNull();
    expect(parseApiErrorMessage(null)).toBeNull();
    expect(parseApiErrorMessage(undefined)).toBeNull();
  });
});

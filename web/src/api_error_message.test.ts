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
});

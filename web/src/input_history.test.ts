import { describe, expect, it } from "vitest";
import {
  parseInputHistory,
  pushInputHistory,
  shouldStoreInputHistoryValue,
} from "./input_history";

describe("parseInputHistory", () => {
  it("returns empty list for invalid data", () => {
    expect(parseInputHistory(null)).toEqual([]);
    expect(parseInputHistory("not-json")).toEqual([]);
    expect(parseInputHistory("{}")).toEqual([]);
  });

  it("normalizes values and removes duplicates", () => {
    const history = parseInputHistory(
      JSON.stringify(["  cargo test  ", "", "cargo test", "git status"])
    );
    expect(history).toEqual(["cargo test", "git status"]);
  });
});

describe("pushInputHistory", () => {
  it("adds newest command to front and keeps unique values", () => {
    const first = pushInputHistory([], "cargo test");
    expect(first).toEqual(["cargo test"]);

    const second = pushInputHistory(first, "git status");
    expect(second).toEqual(["git status", "cargo test"]);

    const third = pushInputHistory(second, "cargo test");
    expect(third).toEqual(["cargo test", "git status"]);
  });

  it("ignores empty commands", () => {
    expect(pushInputHistory(["cargo test"], "  ")).toEqual(["cargo test"]);
  });

  it("does not persist obvious sensitive assignments", () => {
    expect(pushInputHistory([], "OPENAI_API_KEY=sk-test")).toEqual([]);
    expect(pushInputHistory([], "--password=topsecret")).toEqual([]);
    expect(pushInputHistory([], "authorization: bearer abc.def.ghi")).toEqual([]);
  });
});

describe("shouldStoreInputHistoryValue", () => {
  it("allows regular command content", () => {
    expect(shouldStoreInputHistoryValue("cargo test ./web")).toBe(true);
  });

  it("blocks sensitive key patterns", () => {
    expect(shouldStoreInputHistoryValue("token=abc123")).toBe(false);
    expect(shouldStoreInputHistoryValue("private_key: foo")).toBe(false);
    expect(
      shouldStoreInputHistoryValue(
        "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----"
      )
    ).toBe(false);
  });
});

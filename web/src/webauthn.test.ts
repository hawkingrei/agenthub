import { describe, expect, it } from "vitest";

import {
  publicKeyCredentialCreationOptionsFromJson,
  publicKeyCredentialRequestOptionsFromJson,
} from "./webauthn";

describe("WebAuthn option parsing", () => {
  it("rejects null registration options with a useful error", () => {
    expect(() => publicKeyCredentialCreationOptionsFromJson(null)).toThrow(
      "missing WebAuthn registration options"
    );
  });

  it("rejects null authentication options with a useful error", () => {
    expect(() => publicKeyCredentialRequestOptionsFromJson(null)).toThrow(
      "missing WebAuthn authentication options"
    );
  });
});

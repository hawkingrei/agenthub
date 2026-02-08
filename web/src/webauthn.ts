export function publicKeyCredentialCreationOptionsFromJson(
  options: unknown
): PublicKeyCredentialCreationOptions {
  const maybe = options as { publicKey?: PublicKeyCredentialCreationOptions };
  const o = (maybe.publicKey ?? options) as PublicKeyCredentialCreationOptions;
  const challenge = toArrayBuffer(o.challenge, "challenge");
  const user = o.user as PublicKeyCredentialUserEntity;
  const userId = toArrayBuffer(user.id, "user.id");
  const exclude = (o.excludeCredentials ?? []).map((c) => ({
    ...c,
    id: toArrayBuffer(c.id, "excludeCredentials.id"),
  }));
  return {
    ...o,
    challenge,
    user: { ...user, id: userId },
    excludeCredentials: exclude,
  } as PublicKeyCredentialCreationOptions;
}

export function publicKeyCredentialRequestOptionsFromJson(
  options: unknown
): PublicKeyCredentialRequestOptions {
  const maybe = options as { publicKey?: PublicKeyCredentialRequestOptions };
  const o = (maybe.publicKey ?? options) as PublicKeyCredentialRequestOptions;
  const challenge = toArrayBuffer(o.challenge, "challenge");
  const allow = (o.allowCredentials ?? []).map((c) => ({
    ...c,
    id: toArrayBuffer(c.id, "allowCredentials.id"),
  }));
  return {
    ...o,
    challenge,
    allowCredentials: allow,
  } as PublicKeyCredentialRequestOptions;
}

export function registerCredentialToJson(cred: PublicKeyCredential) {
  const response = cred.response as AuthenticatorAttestationResponse;
  return {
    id: cred.id,
    rawId: bufferToBase64Url(cred.rawId),
    type: cred.type,
    response: {
      clientDataJSON: bufferToBase64Url(response.clientDataJSON),
      attestationObject: bufferToBase64Url(response.attestationObject),
      transports: response.getTransports?.() ?? [],
    },
  };
}

export function loginCredentialToJson(cred: PublicKeyCredential) {
  const response = cred.response as AuthenticatorAssertionResponse;
  return {
    id: cred.id,
    rawId: bufferToBase64Url(cred.rawId),
    type: cred.type,
    response: {
      clientDataJSON: bufferToBase64Url(response.clientDataJSON),
      authenticatorData: bufferToBase64Url(response.authenticatorData),
      signature: bufferToBase64Url(response.signature),
      userHandle: response.userHandle
        ? bufferToBase64Url(response.userHandle)
        : null,
    },
  };
}

export function urlBase64ToUint8Array(value: string): Uint8Array {
  return new Uint8Array(base64UrlToBuffer(value));
}

function bufferToBase64Url(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  bytes.forEach((b) => (binary += String.fromCharCode(b)));
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64UrlToBuffer(value: string): ArrayBuffer {
  const base64 = value.replace(/-/g, "+").replace(/_/g, "/");
  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

function toArrayBuffer(input: unknown, label: string): ArrayBuffer {
  if (!input) {
    throw new Error(`missing ${label}`);
  }
  if (typeof input === "string") {
    return base64UrlToBuffer(input);
  }
  if (input instanceof ArrayBuffer) {
    return input;
  }
  if (input instanceof Uint8Array) {
    return input.buffer;
  }
  if (Array.isArray(input)) {
    return new Uint8Array(input).buffer;
  }
  throw new Error(`unsupported ${label} type`);
}

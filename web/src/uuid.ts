const hex = Array.from({ length: 256 }, (_, i) => i.toString(16).padStart(2, "0"));

const toUuidString = (bytes: Uint8Array): string => {
  return (
    hex[bytes[0]] +
    hex[bytes[1]] +
    hex[bytes[2]] +
    hex[bytes[3]] +
    "-" +
    hex[bytes[4]] +
    hex[bytes[5]] +
    "-" +
    hex[bytes[6]] +
    hex[bytes[7]] +
    "-" +
    hex[bytes[8]] +
    hex[bytes[9]] +
    "-" +
    hex[bytes[10]] +
    hex[bytes[11]] +
    hex[bytes[12]] +
    hex[bytes[13]] +
    hex[bytes[14]] +
    hex[bytes[15]]
  );
};

const getRandomBytes = (length: number): Uint8Array => {
  const bytes = new Uint8Array(length);
  if (typeof crypto !== "undefined" && "getRandomValues" in crypto) {
    crypto.getRandomValues(bytes);
    return bytes;
  }
  for (let i = 0; i < length; i += 1) {
    bytes[i] = Math.floor(Math.random() * 256);
  }
  return bytes;
};

let lastTimestamp = 0;
let lastCounter = 0;

export function uuidV7(): string {
  let timestamp = Date.now();
  if (timestamp === lastTimestamp) {
    lastCounter = (lastCounter + 1) & 0x0fff;
    if (lastCounter === 0) {
      while (Date.now() === timestamp) {
        // Busy wait for next millisecond to preserve ordering.
      }
      timestamp = Date.now();
    }
  }
  if (timestamp !== lastTimestamp) {
    lastTimestamp = timestamp;
    const rand = getRandomBytes(2);
    lastCounter = ((rand[0] << 8) | rand[1]) & 0x0fff;
  }

  const bytes = getRandomBytes(16);
  bytes[0] = (timestamp >>> 40) & 0xff;
  bytes[1] = (timestamp >>> 32) & 0xff;
  bytes[2] = (timestamp >>> 24) & 0xff;
  bytes[3] = (timestamp >>> 16) & 0xff;
  bytes[4] = (timestamp >>> 8) & 0xff;
  bytes[5] = timestamp & 0xff;
  bytes[6] = 0x70 | ((lastCounter >>> 8) & 0x0f);
  bytes[7] = lastCounter & 0xff;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  return toUuidString(bytes);
}

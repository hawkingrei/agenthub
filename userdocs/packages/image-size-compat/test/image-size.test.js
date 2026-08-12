import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { imageSizeFromFile } from "../fromFile.js";
import { imageSize } from "../index.js";

const ONE_PIXEL_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
  "base64",
);

const MALFORMED_HEIF = Uint8Array.from([
  0x00, 0x00, 0x00, 0x10, 0x66, 0x74, 0x79, 0x70,
  0x61, 0x76, 0x69, 0x66, 0x00, 0x00, 0x00, 0x00,
  0x00, 0x00, 0x00, 0x24, 0x6d, 0x65, 0x74, 0x61,
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08,
  0x69, 0x70, 0x72, 0x70, 0x00, 0x00, 0x00, 0x14,
  0x69, 0x70, 0x63, 0x6f, 0x00, 0x00, 0x00, 0x00,
  0x69, 0x73, 0x70, 0x65, 0x00, 0x00, 0x00, 0x00,
  0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
]);

const MALFORMED_ICNS = Uint8Array.from([
  0x69, 0x63, 0x6e, 0x73, 0x00, 0x00, 0x00, 0x10,
  0x69, 0x73, 0x33, 0x32, 0x00, 0x00, 0x00, 0x00,
]);

test("reads dimensions from memory", () => {
  assert.deepEqual(imageSize(ONE_PIXEL_PNG), {
    width: 1,
    height: 1,
    type: "png",
  });
});

test("reads dimensions from a file", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "agenthub-image-size-"));
  const imagePath = path.join(directory, "pixel.png");

  try {
    await writeFile(imagePath, ONE_PIXEL_PNG);
    assert.deepEqual(await imageSizeFromFile(imagePath), {
      width: 1,
      height: 1,
      type: "png",
    });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("returns control for zero-length container entries", () => {
  for (const input of [MALFORMED_HEIF, MALFORMED_ICNS]) {
    try {
      imageSize(input);
    } catch (error) {
      assert.match(error.message, /invalid image data/i);
    }
  }
});

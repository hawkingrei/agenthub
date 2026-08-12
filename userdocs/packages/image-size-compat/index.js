import { readFileSync } from "node:fs";
import probe from "probe-image-size";

import { normalizeImageSize } from "./normalize.js";

export function imageSize(input) {
  const data = typeof input === "string" ? readFileSync(input) : input;
  return normalizeImageSize(probe.sync(data));
}

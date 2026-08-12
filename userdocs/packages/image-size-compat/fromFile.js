import { createReadStream } from "node:fs";
import probe from "probe-image-size";

import { normalizeImageSize } from "./normalize.js";

export async function imageSizeFromFile(filePath) {
  return normalizeImageSize(await probe(createReadStream(filePath)));
}

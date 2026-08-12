export function normalizeImageSize(result) {
  if (!result?.width || !result?.height) {
    throw new TypeError("Unsupported or invalid image data");
  }

  return {
    width: result.width,
    height: result.height,
    type: result.type,
  };
}

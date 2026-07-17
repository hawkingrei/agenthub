import { api, type ObjectUploadRecord, type TeamUploadRequest } from "../../api";

const TEAM_IMAGE_UPLOAD_ALLOWED_TYPES = new Set([
  "image/png",
  "image/jpeg",
  "image/webp",
  "image/gif",
]);

export type TeamImageUploadPreparedRequest = TeamUploadRequest & {
  markdownAlt: string;
};

export type TeamImageUploadResult = {
  upload: ObjectUploadRecord;
  markdown: string;
};

export function isTeamImageUploadContentTypeAllowed(contentType: string): boolean {
  return TEAM_IMAGE_UPLOAD_ALLOWED_TYPES.has(contentType.trim().toLowerCase());
}

export function normalizeTeamImageUploadFileName(fileName: string): string {
  const trimmed = fileName.trim();
  if (!trimmed || trimmed === "." || trimmed === "..") {
    return "image";
  }
  return trimmed.replace(/[\\/]+/g, "-");
}

export function resolveTeamUploadedImageUrl(upload: ObjectUploadRecord): string {
  const publicUrl = upload.public_url?.trim();
  return publicUrl && publicUrl.length > 0 ? publicUrl : upload.object_key;
}

export function buildTeamUploadedImageMarkdown(upload: ObjectUploadRecord): string {
  const alt = escapeMarkdownImageAlt(upload.original_filename || "image");
  return `![${alt}](${resolveTeamUploadedImageUrl(upload)})`;
}

export function insertMarkdownAtCursor(
  draft: string,
  markdown: string,
  cursor: number | null
): { text: string; cursor: number } {
  const insertionPoint =
    cursor == null || Number.isNaN(cursor)
      ? draft.length
      : Math.max(0, Math.min(cursor, draft.length));
  const before = draft.slice(0, insertionPoint);
  const after = draft.slice(insertionPoint);
  const prefix = before.length > 0 && !before.endsWith("\n") ? "\n" : "";
  const suffix = after.length > 0 && !after.startsWith("\n") ? "\n" : "";
  const inserted = `${prefix}${markdown}${suffix}`;
  return {
    text: `${before}${inserted}${after}`,
    cursor: insertionPoint + inserted.length,
  };
}

export async function prepareTeamImageUploadRequest(
  file: File
): Promise<TeamImageUploadPreparedRequest> {
  const contentType = file.type.trim().toLowerCase();
  if (!isTeamImageUploadContentTypeAllowed(contentType)) {
    throw new Error("Image upload accepts PNG, JPEG, WebP, or GIF files");
  }
  const bytes = await file.arrayBuffer();
  const expectedSha256 = await sha256Hex(bytes);
  const fileName = normalizeTeamImageUploadFileName(file.name);
  return {
    file_name: fileName,
    content_type: contentType,
    bytes_base64: arrayBufferToBase64(bytes),
    expected_size_bytes: bytes.byteLength,
    expected_sha256: expectedSha256,
    markdownAlt: fileName,
  };
}

export async function uploadTeamImageForGraphBed({
  token,
  teamId,
  file,
}: {
  token: string;
  teamId: string;
  file: File;
}): Promise<TeamImageUploadResult> {
  const payload = await prepareTeamImageUploadRequest(file);
  const upload = await api.uploadTeamImage(token, teamId, payload);
  return {
    upload,
    markdown: buildTeamUploadedImageMarkdown(upload),
  };
}

function escapeMarkdownImageAlt(value: string): string {
  return value.replace(/[[\]\\]/g, "\\$&").replace(/\n/g, " ").trim() || "image";
}

function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  const chunkSize = 0x8000;
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    const chunk = bytes.subarray(offset, offset + chunkSize);
    binary += String.fromCharCode(...chunk);
  }
  return btoa(binary);
}

async function sha256Hex(buffer: ArrayBuffer): Promise<string> {
  const bytes = new Uint8Array(buffer);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

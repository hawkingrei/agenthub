// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  buildTeamUploadedImageMarkdown,
  insertMarkdownAtCursor,
  isTeamImageUploadContentTypeAllowed,
  normalizeTeamImageUploadFileName,
  prepareTeamImageUploadRequest,
  uploadTeamImageForGraphBed,
} from "./team_image_upload";
import { api, type ObjectUploadRecord } from "../../api";

function buildUpload(overrides: Partial<ObjectUploadRecord> = {}): ObjectUploadRecord {
  return {
    id: "upload-1",
    owner_scope: "teams/team-1",
    backend: "s3",
    object_key: "images/teams/team-1/upload-1.png",
    original_filename: "diagram.png",
    content_type: "image/png",
    size_bytes: 4,
    sha256: "sha",
    public_url: "https://cdn.example.test/diagram.png",
    created_by_actor_id: "human",
    publish_state: "published",
    created_at: 1,
    published_at: 1,
    cleanup_after: null,
    ...overrides,
  };
}

describe("team image upload helpers", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("prepares a verified base64 request for allowlisted raster images", async () => {
    const file = new File([new Uint8Array([1, 2, 3, 4])], "diagram.png", {
      type: "IMAGE/PNG",
    });

    const request = await prepareTeamImageUploadRequest(file);

    expect(request).toMatchObject({
      file_name: "diagram.png",
      content_type: "image/png",
      bytes_base64: "AQIDBA==",
      expected_size_bytes: 4,
      expected_sha256:
        "9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a",
      markdownAlt: "diagram.png",
    });
  });

  it("rejects svg and html-backed uploads before calling the API", () => {
    expect(isTeamImageUploadContentTypeAllowed("image/svg+xml")).toBe(false);
    expect(isTeamImageUploadContentTypeAllowed("text/html")).toBe(false);
    expect(isTeamImageUploadContentTypeAllowed("image/webp")).toBe(true);
  });

  it("normalizes unsafe or empty file names before storing image uploads", () => {
    expect(normalizeTeamImageUploadFileName("")).toBe("image");
    expect(normalizeTeamImageUploadFileName(" .. ")).toBe("image");
    expect(normalizeTeamImageUploadFileName("folder\\nested/diagram.png")).toBe(
      "folder-nested-diagram.png"
    );
  });

  it("rejects unsupported image payloads during request preparation", async () => {
    const file = new File([new Uint8Array([1, 2, 3, 4])], "diagram.svg", {
      type: "image/svg+xml",
    });

    await expect(prepareTeamImageUploadRequest(file)).rejects.toThrow(
      "Image upload accepts PNG, JPEG, WebP, or GIF files"
    );
  });

  it("builds markdown from public URLs and inserts it at the cursor", () => {
    const markdown = buildTeamUploadedImageMarkdown({
      id: "upload-1",
      owner_scope: "teams/team-1",
      backend: "s3",
      object_key: "images/teams/team-1/upload-1.png",
      original_filename: "diagram[1].png",
      content_type: "image/png",
      size_bytes: 4,
      sha256: "sha",
      public_url: "https://cdn.example.test/diagram.png",
      created_by_actor_id: "human",
      publish_state: "published",
      created_at: 1,
      published_at: 1,
      cleanup_after: null,
    });

    expect(markdown).toBe("![diagram\\[1\\].png](https://cdn.example.test/diagram.png)");
    expect(insertMarkdownAtCursor("before after", markdown, 6)).toEqual({
      text: "before\n![diagram\\[1\\].png](https://cdn.example.test/diagram.png)\n after",
      cursor: 65,
    });
  });

  it("falls back to object keys and default alt text when public URLs are missing", () => {
    expect(
      buildTeamUploadedImageMarkdown(
        buildUpload({
          original_filename: "",
          public_url: "  ",
        })
      )
    ).toBe("![image](images/teams/team-1/upload-1.png)");
  });

  it("inserts markdown at draft bounds and preserves newline spacing", () => {
    expect(insertMarkdownAtCursor("before", "![image](url)", null)).toEqual({
      text: "before\n![image](url)",
      cursor: 20,
    });
    expect(insertMarkdownAtCursor("before", "![image](url)", -10)).toEqual({
      text: "![image](url)\nbefore",
      cursor: 14,
    });
  });

  it("uploads a prepared request and returns graph-bed markdown", async () => {
    const uploadSpy = vi.spyOn(api, "uploadTeamImage").mockResolvedValue(buildUpload());
    const file = new File([new Uint8Array([1, 2, 3, 4])], "diagram.png", {
      type: "image/png",
    });

    await expect(
      uploadTeamImageForGraphBed({
        token: "token-1",
        teamId: "team-1",
        file,
      })
    ).resolves.toMatchObject({
      markdown: "![diagram.png](https://cdn.example.test/diagram.png)",
    });
    expect(uploadSpy).toHaveBeenCalledWith("token-1", "team-1", {
      file_name: "diagram.png",
      content_type: "image/png",
      bytes_base64: "AQIDBA==",
      expected_size_bytes: 4,
      expected_sha256:
        "9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a",
      markdownAlt: "diagram.png",
    });
  });
});

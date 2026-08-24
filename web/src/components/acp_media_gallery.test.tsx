import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AcpMediaGallery, resolveAcpMediaSource } from "./acp_media_gallery";

describe("AcpMediaGallery", () => {
  it("renders supported inline images", () => {
    const html = renderToStaticMarkup(
      <AcpMediaGallery
        media={[
          {
            type: "image",
            name: "result.png",
            mime_type: "image/png",
            data: "aW1hZ2U=",
          },
        ]}
      />
    );

    expect(html).toContain('data-acp-media-gallery="true"');
    expect(html).toContain("data:image/png;base64,aW1hZ2U=");
    expect(html).toContain('alt="result.png"');
  });

  it("rejects unsupported or unsafe image sources", () => {
    expect(
      resolveAcpMediaSource({
        type: "image",
        mime_type: "image/svg+xml",
        data: "PHN2Zz4=",
      })
    ).toBeNull();
    expect(
      resolveAcpMediaSource({
        type: "image",
        mime_type: "image/png",
        uri: "file:///tmp/image.png",
      })
    ).toBeNull();
  });
});

import React from "react";
import type { AcpMedia } from "../acp";

const SAFE_IMAGE_MIME_TYPES = new Set([
  "image/png",
  "image/jpeg",
  "image/webp",
  "image/gif",
]);

export function resolveAcpMediaSource(media: AcpMedia): string | null {
  const mimeType = media.mime_type.trim().toLowerCase();
  if (!SAFE_IMAGE_MIME_TYPES.has(mimeType)) return null;
  if (media.data) return `data:${mimeType};base64,${media.data}`;
  if (!media.uri) return null;
  if (/^https?:\/\//i.test(media.uri)) return media.uri;
  if (media.uri.startsWith(`data:${mimeType};base64,`)) return media.uri;
  return null;
}

export const AcpMediaGallery = React.memo(function AcpMediaGallery({
  media,
  compact = false,
}: {
  media?: AcpMedia[];
  compact?: boolean;
}) {
  const items = (media ?? [])
    .map((item) => ({ item, source: resolveAcpMediaSource(item) }))
    .filter((entry): entry is { item: AcpMedia; source: string } => Boolean(entry.source));
  if (items.length === 0) return null;

  return (
    <div
      className={`grid gap-2 ${compact ? "grid-cols-[repeat(auto-fill,minmax(7rem,10rem))]" : "grid-cols-[repeat(auto-fill,minmax(10rem,16rem))]"}`}
      data-acp-media-gallery="true"
    >
      {items.map(({ item, source }, index) => (
        <a
          key={`${source.slice(0, 80)}:${index}`}
          href={source}
          target="_blank"
          rel="noreferrer"
          className="block overflow-hidden rounded-xl border border-black/10 bg-slate-50 shadow-sm transition hover:border-black/20"
          title={item.name ? `Open ${item.name}` : "Open image"}
        >
          <img
            src={source}
            alt={item.name ?? `Image ${index + 1}`}
            loading="lazy"
            referrerPolicy="no-referrer"
            className={`w-full object-contain ${compact ? "max-h-40" : "max-h-72"}`}
          />
        </a>
      ))}
    </div>
  );
});

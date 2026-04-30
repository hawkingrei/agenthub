import React from "react";

type DeterministicAvatarProps = {
  name?: string | null;
  stableId?: string | number | null;
  className?: string;
  title?: string;
  ariaHidden?: boolean;
};

type DeterministicAvatarModel = {
  seed: string;
  backgroundColor: string;
  foregroundColor: string;
  cells: readonly boolean[];
};

function normalizeAvatarSeed(name?: string | null, stableId?: string | number | null): string {
  const normalizedName = name?.trim() || "unknown";
  const normalizedId =
    stableId == null ? "unknown" : String(stableId).trim() || "unknown";
  return `${normalizedName}::${normalizedId}`;
}

function hashAvatarSeed(seed: string): number {
  let hash = 0x811c9dc5;
  for (let index = 0; index < seed.length; index += 1) {
    hash ^= seed.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash >>> 0;
}

function buildAvatarCells(hash: number): boolean[] {
  const cells: boolean[] = [];
  let state = hash || 0x811c9dc5;
  for (let row = 0; row < 5; row += 1) {
    const mirroredRow: boolean[] = [];
    for (let col = 0; col < 3; col += 1) {
      state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
      mirroredRow.push((state & 1) === 0);
    }
    cells.push(
      mirroredRow[0] ?? false,
      mirroredRow[1] ?? false,
      mirroredRow[2] ?? false,
      mirroredRow[1] ?? false,
      mirroredRow[0] ?? false
    );
  }
  return cells;
}

function buildDeterministicAvatarModel(
  name?: string | null,
  stableId?: string | number | null
): DeterministicAvatarModel {
  const seed = normalizeAvatarSeed(name, stableId);
  const hash = hashAvatarSeed(seed);
  const hue = hash % 360;
  const saturation = 56 + ((hash >>> 9) % 18);
  const backgroundLightness = 92 - ((hash >>> 17) % 8);
  const foregroundLightness = 34 + ((hash >>> 25) % 12);
  return {
    seed,
    backgroundColor: `hsl(${hue} ${saturation}% ${backgroundLightness}%)`,
    foregroundColor: `hsl(${hue} ${saturation + 8}% ${foregroundLightness}%)`,
    cells: buildAvatarCells(hash),
  };
}

export function resolveDeterministicAvatarSeed(
  name?: string | null,
  stableId?: string | number | null
): string {
  return normalizeAvatarSeed(name, stableId);
}

export function renderDeterministicAvatarModel(
  name?: string | null,
  stableId?: string | number | null
): DeterministicAvatarModel {
  return buildDeterministicAvatarModel(name, stableId);
}

export const DeterministicAvatar = React.memo(function DeterministicAvatar({
  name,
  stableId,
  className = "",
  title,
  ariaHidden = true,
}: DeterministicAvatarProps) {
  const model = buildDeterministicAvatarModel(name, stableId);
  return (
    <span
      className={`inline-flex shrink-0 overflow-hidden rounded-full ${className}`.trim()}
      data-avatar-seed={model.seed}
      title={title}
      aria-hidden={ariaHidden}
    >
      <svg
        viewBox="0 0 48 48"
        className="h-full w-full"
        role="presentation"
        focusable="false"
      >
        <rect width="48" height="48" rx="24" fill={model.backgroundColor} />
        {model.cells.map((filled, index) => {
          if (!filled) {
            return null;
          }
          const row = Math.floor(index / 5);
          const col = index % 5;
          const x = 7 + col * 7;
          const y = 7 + row * 7;
          return (
            <rect
              key={`${row}-${col}`}
              x={x}
              y={y}
              width="6"
              height="6"
              rx="1.5"
              fill={model.foregroundColor}
            />
          );
        })}
      </svg>
    </span>
  );
});

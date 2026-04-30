import React from "react";

type DeterministicAvatarProps = {
  name?: string | null;
  stableId?: string | number | null;
  className?: string;
  title?: string;
  ariaHidden?: boolean;
};

type PixelTone = "foreground" | "accent" | "shadow";

type DeterministicAvatarModel = {
  seed: string;
  variantKey: number;
  backgroundColor: string;
  foregroundColor: string;
  accentColor: string;
  shadowColor: string;
  borderColor: string;
  cells: readonly (PixelTone | null)[];
};

const GRID_SIZE = 8;
const MIRROR_COLUMNS = GRID_SIZE / 2;

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

function resolveTone(value: number): PixelTone | null {
  if (value < 2) {
    return null;
  }
  if (value < 5) {
    return "foreground";
  }
  if (value === 5) {
    return "accent";
  }
  return "shadow";
}

function buildSymmetricCells(hash: number): readonly (PixelTone | null)[] {
  const cells: (PixelTone | null)[] = Array.from({ length: GRID_SIZE * GRID_SIZE }, () => null);
  let state = hash || 0x811c9dc5;
  for (let row = 0; row < GRID_SIZE; row += 1) {
    const rowBias = (hash >>> (row % 16)) & 3;
    for (let col = 0; col < MIRROR_COLUMNS; col += 1) {
      state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
      const base = (state >>> ((col % 4) * 3)) & 7;
      const value = (base + rowBias) & 7;
      const tone = resolveTone(value);
      const mirroredCol = GRID_SIZE - 1 - col;
      cells[row * GRID_SIZE + col] = tone;
      cells[row * GRID_SIZE + mirroredCol] = tone;
    }
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
  const saturation = 24 + ((hash >>> 8) % 12);
  const backgroundLightness = 86 + ((hash >>> 16) % 7);
  const variantKey = hash & 1023;
  return {
    seed,
    variantKey,
    backgroundColor: `hsl(${hue} ${saturation}% ${backgroundLightness}%)`,
    foregroundColor: `hsl(${(hue + 10) % 360} ${Math.max(22, saturation + 2)}% 32%)`,
    accentColor: `hsl(${(hue + 165) % 360} ${Math.min(48, saturation + 12)}% 52%)`,
    shadowColor: `hsl(${(hue + 6) % 360} ${Math.max(18, saturation - 2)}% 23%)`,
    borderColor: `hsl(${hue} ${Math.max(16, saturation - 8)}% ${backgroundLightness - 11}%)`,
    cells: buildSymmetricCells(hash),
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
      data-avatar-variant={String(model.variantKey)}
      title={title}
      aria-hidden={ariaHidden}
      style={{
        backgroundColor: model.backgroundColor,
        boxShadow: `inset 0 0 0 1px ${model.borderColor}`,
      }}
    >
      <span
        className="grid h-full w-full"
        role="presentation"
        style={{
          gridTemplateColumns: `repeat(${GRID_SIZE}, 1fr)`,
          gridTemplateRows: `repeat(${GRID_SIZE}, 1fr)`,
          imageRendering: "pixelated",
        }}
      >
        {model.cells.map((tone, index) => {
          const backgroundColor =
            tone === "foreground"
              ? model.foregroundColor
              : tone === "accent"
                ? model.accentColor
                : tone === "shadow"
                  ? model.shadowColor
                  : "transparent";
          return (
            <span
              key={index}
              aria-hidden="true"
              style={{ backgroundColor }}
            />
          );
        })}
      </span>
    </span>
  );
});

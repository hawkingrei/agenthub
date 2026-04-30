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
  variantKey: number;
  backgroundColor: string;
  foregroundColor: string;
  accentColor: string;
  borderColor: string;
  silhouetteIndex: number;
  centerIndex: number;
  topperIndex: number;
  orbitIndex: number;
  baseIndex: number;
};

type LayerProps = {
  foregroundColor: string;
  accentColor: string;
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

function decomposeVariantKey(variantKey: number): [number, number, number, number, number] {
  return [
    variantKey & 3,
    (variantKey >>> 2) & 3,
    (variantKey >>> 4) & 3,
    (variantKey >>> 6) & 3,
    (variantKey >>> 8) & 3,
  ];
}

const SILHOUETTE_LAYERS: readonly ((props: LayerProps) => React.ReactNode)[] = [
  ({ foregroundColor }) => (
    <g>
      <circle cx="24" cy="25" r="11.5" fill={foregroundColor} />
      <rect x="16" y="31" width="16" height="4.5" rx="2.25" fill={foregroundColor} />
    </g>
  ),
  ({ foregroundColor }) => (
    <g>
      <rect x="13" y="14" width="22" height="22" rx="7" fill={foregroundColor} />
      <rect x="18" y="32" width="12" height="4" rx="2" fill={foregroundColor} />
    </g>
  ),
  ({ foregroundColor }) => (
    <g>
      <path d="M24 11L35 18V31L24 38L13 31V18L24 11Z" fill={foregroundColor} />
      <rect x="18" y="31" width="12" height="4" rx="2" fill={foregroundColor} />
    </g>
  ),
  ({ foregroundColor }) => (
    <g>
      <path
        d="M24 12C31.4 12 36.5 17.6 36.5 24.3C36.5 31.3 30.8 36 24 36C17.2 36 11.5 31.3 11.5 24.3C11.5 17.6 16.6 12 24 12Z"
        fill={foregroundColor}
      />
      <rect x="17" y="31.5" width="14" height="3.5" rx="1.75" fill={foregroundColor} />
    </g>
  ),
];

const CENTER_LAYERS: readonly ((props: LayerProps) => React.ReactNode)[] = [
  ({ accentColor }) => (
    <path
      d="M24 18.5L25.9 22.4L30.2 22.9L27 25.9L27.8 30.1L24 28.1L20.2 30.1L21 25.9L17.8 22.9L22.1 22.4Z"
      fill={accentColor}
    />
  ),
  ({ accentColor }) => (
    <g>
      <rect x="20" y="19.5" width="8" height="8" rx="2.4" fill={accentColor} />
      <rect x="18.5" y="28.5" width="11" height="2.4" rx="1.2" fill={accentColor} />
    </g>
  ),
  ({ accentColor }) => (
    <g>
      <circle cx="21" cy="22.5" r="2.4" fill={accentColor} />
      <circle cx="27" cy="22.5" r="2.4" fill={accentColor} />
      <rect x="20.5" y="27" width="7" height="2.2" rx="1.1" fill={accentColor} />
    </g>
  ),
  ({ accentColor }) => (
    <path
      d="M24 18C28.2 18 30.7 20.6 30.7 24.2C30.7 27.9 27.8 30.6 24 30.6C20.2 30.6 17.3 27.9 17.3 24.2C17.3 20.6 19.8 18 24 18ZM24 20.4C21.5 20.4 19.9 22 19.9 24.2C19.9 26.5 21.7 28.1 24 28.1C26.3 28.1 28.1 26.5 28.1 24.2C28.1 22 26.5 20.4 24 20.4Z"
      fill={accentColor}
    />
  ),
];

const TOPPER_LAYERS: readonly ((props: LayerProps) => React.ReactNode)[] = [
  ({ accentColor }) => (
    <path d="M24 10L27.8 14H20.2L24 10Z" fill={accentColor} />
  ),
  ({ accentColor }) => (
    <g>
      <rect x="22.8" y="9.8" width="2.4" height="6.4" rx="1.2" fill={accentColor} />
      <circle cx="24" cy="9.4" r="1.9" fill={accentColor} />
    </g>
  ),
  ({ accentColor }) => (
    <path
      d="M18.5 12.5C19.8 10.7 22 9.8 24 9.8C26 9.8 28.2 10.7 29.5 12.5L27.7 14.1C26.8 12.9 25.4 12.2 24 12.2C22.6 12.2 21.2 12.9 20.3 14.1Z"
      fill={accentColor}
    />
  ),
  () => null,
];

const ORBIT_LAYERS: readonly ((props: LayerProps) => React.ReactNode)[] = [
  ({ accentColor }) => (
    <circle cx="24" cy="24" r="15.5" fill="none" stroke={accentColor} strokeWidth="1.6" strokeDasharray="3.2 2.4" />
  ),
  ({ accentColor }) => (
    <g fill={accentColor}>
      <circle cx="13" cy="21" r="1.4" />
      <circle cx="35" cy="19" r="1.5" />
      <circle cx="33" cy="32" r="1.3" />
    </g>
  ),
  ({ accentColor }) => (
    <path d="M12 30C16 33.4 20.1 35 24 35C27.9 35 32 33.4 36 30" fill="none" stroke={accentColor} strokeWidth="1.8" strokeLinecap="round" />
  ),
  ({ accentColor }) => (
    <g>
      <rect x="10.5" y="13" width="4.4" height="4.4" rx="1.5" fill={accentColor} />
      <rect x="33.1" y="28.4" width="4.4" height="4.4" rx="1.5" fill={accentColor} />
    </g>
  ),
];

const BASE_LAYERS: readonly ((props: LayerProps) => React.ReactNode)[] = [
  ({ foregroundColor }) => (
    <rect x="15" y="36.2" width="18" height="2.8" rx="1.4" fill={foregroundColor} opacity="0.22" />
  ),
  ({ foregroundColor }) => (
    <path d="M16.5 37.2C19.2 35.9 21.7 35.3 24 35.3C26.3 35.3 28.8 35.9 31.5 37.2" fill="none" stroke={foregroundColor} strokeWidth="2" strokeLinecap="round" opacity="0.22" />
  ),
  ({ accentColor }) => (
    <rect x="18" y="36" width="12" height="2.4" rx="1.2" fill={accentColor} opacity="0.4" />
  ),
  () => null,
];

function buildDeterministicAvatarModel(
  name?: string | null,
  stableId?: string | number | null
): DeterministicAvatarModel {
  const seed = normalizeAvatarSeed(name, stableId);
  const hash = hashAvatarSeed(seed);
  const variantKey = hash & 1023;
  const [silhouetteIndex, centerIndex, topperIndex, orbitIndex, baseIndex] =
    decomposeVariantKey(variantKey);
  const hue = hash % 360;
  const saturation = 50 + ((hash >>> 10) % 18);
  const backgroundLightness = 92 - ((hash >>> 18) % 6);
  return {
    seed,
    variantKey,
    backgroundColor: `hsl(${hue} ${saturation}% ${backgroundLightness}%)`,
    foregroundColor: `hsl(${(hue + 18) % 360} ${Math.max(42, saturation - 6)}% 32%)`,
    accentColor: `hsl(${(hue + 210) % 360} ${Math.min(84, saturation + 10)}% 52%)`,
    borderColor: `hsl(${hue} ${Math.max(38, saturation - 10)}% ${backgroundLightness - 11}%)`,
    silhouetteIndex,
    centerIndex,
    topperIndex,
    orbitIndex,
    baseIndex,
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
  const layerProps = {
    foregroundColor: model.foregroundColor,
    accentColor: model.accentColor,
  };
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
      <svg
        viewBox="0 0 48 48"
        className="h-full w-full"
        role="presentation"
        focusable="false"
      >
        <rect width="48" height="48" rx="24" fill={model.backgroundColor} />
        {ORBIT_LAYERS[model.orbitIndex]?.(layerProps) ?? null}
        {SILHOUETTE_LAYERS[model.silhouetteIndex]?.(layerProps) ?? null}
        {CENTER_LAYERS[model.centerIndex]?.(layerProps) ?? null}
        {TOPPER_LAYERS[model.topperIndex]?.(layerProps) ?? null}
        {BASE_LAYERS[model.baseIndex]?.(layerProps) ?? null}
      </svg>
    </span>
  );
});

type HorizontalEllipsisIconProps = {
  className?: string;
};

export function HorizontalEllipsisIcon({
  className = "block h-4 w-4 shrink-0 text-black/55",
}: HorizontalEllipsisIconProps) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 20 20"
      className={className}
      fill="currentColor"
    >
      <path d="M4 11.375a1.375 1.375 0 1 0 0-2.75 1.375 1.375 0 0 0 0 2.75m6 0a1.375 1.375 0 1 0 0-2.75 1.375 1.375 0 0 0 0 2.75m6 0a1.375 1.375 0 1 0 0-2.75 1.375 1.375 0 0 0 0 2.75" />
    </svg>
  );
}

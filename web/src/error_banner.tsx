import React from "react";
import { IconButton } from "./ui/primitives";

type ErrorBannerProps = {
  message: string;
  onClose?: () => void;
};

const ERROR_BANNER_CLASS =
  "error flex items-start justify-between gap-3 rounded-xl border border-rose-200 bg-rose-50/90 px-4 py-3 text-rose-900 shadow-sm";
const ERROR_TEXT_CLASS = "error-text text-sm leading-5";
const ERROR_CLOSE_CLASS =
  "error-close inline-flex h-7 w-7 items-center justify-center rounded-md border border-rose-200 bg-white text-sm font-semibold text-rose-700 transition hover:bg-rose-100";

export function ErrorBanner({ message, onClose }: ErrorBannerProps) {
  return (
    <div className={ERROR_BANNER_CLASS} role="alert">
      <span className={ERROR_TEXT_CLASS}>{message}</span>
      {onClose && (
        <IconButton
          className={ERROR_CLOSE_CLASS}
          onClick={onClose}
          aria-label="Dismiss error"
          size="sm"
        >
          x
        </IconButton>
      )}
    </div>
  );
}

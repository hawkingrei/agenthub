import React from "react";

type ErrorBannerProps = {
  message: string;
  onClose?: () => void;
};

export function ErrorBanner({ message, onClose }: ErrorBannerProps) {
  return (
    <div className="error" role="alert">
      <span className="error-text">{message}</span>
      {onClose && (
        <button
          type="button"
          className="error-close"
          onClick={onClose}
          aria-label="Dismiss error"
        >
          x
        </button>
      )}
    </div>
  );
}

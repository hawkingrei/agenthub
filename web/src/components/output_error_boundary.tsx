import React from "react";

type OutputErrorBoundaryProps = {
  children: React.ReactNode;
  onReset?: () => void;
};

type OutputErrorBoundaryState = {
  hasError: boolean;
};

const OUTPUT_ERROR_FALLBACK_BODY_CLASS =
  "output-body rounded-2xl border border-slate-200/80 bg-white/85 shadow-sm backdrop-blur";
const OUTPUT_ERROR_FALLBACK_CARD_CLASS =
  "output-error mx-auto my-6 flex max-w-md flex-col items-start gap-2 rounded-xl border border-rose-200 bg-rose-50/70 p-4 text-rose-900";
const OUTPUT_ERROR_RETRY_BUTTON_CLASS =
  "ghost rounded-lg border border-rose-300 bg-white px-3 py-2 text-sm font-medium text-rose-700 hover:border-rose-400";

export class OutputErrorBoundary extends React.Component<
  OutputErrorBoundaryProps,
  OutputErrorBoundaryState
> {
  state: OutputErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): OutputErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: unknown, info: unknown) {
    reportRenderError(error, info);
  }

  handleReset = () => {
    this.setState({ hasError: false });
    this.props.onReset?.();
  };

  render() {
    if (!this.state.hasError) {
      return this.props.children;
    }
    return (
      <div className={OUTPUT_ERROR_FALLBACK_BODY_CLASS}>
        <div className={OUTPUT_ERROR_FALLBACK_CARD_CLASS}>
          <div className="title">Output failed to render</div>
          <div className="meta">Retry rendering the latest output.</div>
          <button className={OUTPUT_ERROR_RETRY_BUTTON_CLASS} onClick={this.handleReset}>
            Retry
          </button>
        </div>
      </div>
    );
  }
}

function reportRenderError(error: unknown, info: unknown) {
  const detail = {
    source: "output_error_boundary",
    error,
    info,
  };
  if (
    "dispatchEvent" in globalThis &&
    typeof globalThis.dispatchEvent === "function" &&
    typeof CustomEvent === "function"
  ) {
    globalThis.dispatchEvent(
      new CustomEvent("agenthub:output-render-error", {
        detail,
      })
    );
  }
  if ("reportError" in globalThis && typeof globalThis.reportError === "function") {
    globalThis.reportError(error);
  }
}

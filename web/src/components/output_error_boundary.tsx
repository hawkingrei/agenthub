import React from "react";

type OutputErrorBoundaryProps = {
  children: React.ReactNode;
  onReset?: () => void;
};

type OutputErrorBoundaryState = {
  hasError: boolean;
};

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
      <div className="output-body">
        <div className="output-error">
          <div className="title">Output failed to render</div>
          <div className="meta">Retry rendering the latest output.</div>
          <button className="ghost" onClick={this.handleReset}>
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

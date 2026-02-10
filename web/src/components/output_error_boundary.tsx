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
    console.error("Output render failed", error, info);
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
          <div className="meta">Check the console for details.</div>
          <button className="ghost" onClick={this.handleReset}>
            Retry
          </button>
        </div>
      </div>
    );
  }
}

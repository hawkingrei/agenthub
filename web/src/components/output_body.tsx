import React from "react";
import { AcpPanel, AcpPanelProps } from "./acp_panel";
import { TerminalOutput } from "./terminal_output";
import { OutputLine } from "../output_cache";

type OutputBodyProps = {
  terminalRef: React.RefObject<HTMLDivElement>;
  onTerminalScroll?: () => void;
  isOutputLoading: boolean;
  outputs: OutputLine[];
  ansi: (input: string) => string;
  acpPanelProps: AcpPanelProps;
};

export function OutputBody({
  terminalRef,
  onTerminalScroll,
  isOutputLoading,
  outputs,
  ansi,
  acpPanelProps,
}: OutputBodyProps) {
  return (
    <div className="output-body">
      {isOutputLoading ? (
        <div className="output-loading">
          <i className="bi bi-hourglass-split spinner" aria-hidden="true" />
          <div className="label">Waiting for output</div>
        </div>
      ) : acpPanelProps.acpView.hasAcp ? (
        <AcpPanel {...acpPanelProps} />
      ) : outputs.length === 0 ? (
        <div className="output-empty">
          <div className="title">No output yet</div>
          <div className="meta">Send input or start the agent to see output.</div>
        </div>
      ) : (
        <TerminalOutput
          outputs={outputs}
          ansi={ansi}
          containerRef={terminalRef}
          onScroll={onTerminalScroll}
        />
      )}
    </div>
  );
}

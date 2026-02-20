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

const OUTPUT_BODY_CLASS =
  "output-body rounded-2xl border border-slate-200/80 bg-white/85 shadow-sm backdrop-blur";
const OUTPUT_LOADING_CLASS =
  "output-loading flex h-full min-h-40 flex-col items-center justify-center gap-2 text-slate-600";
const OUTPUT_EMPTY_CLASS =
  "output-empty flex h-full min-h-40 flex-col items-center justify-center gap-1 rounded-xl border border-dashed border-slate-300 bg-slate-50/60 p-4 text-center";

export const OutputBody = React.memo(function OutputBody({
  terminalRef,
  onTerminalScroll,
  isOutputLoading,
  outputs,
  ansi,
  acpPanelProps,
}: OutputBodyProps) {
  return (
    <div className={OUTPUT_BODY_CLASS}>
      {isOutputLoading ? (
        <div className={OUTPUT_LOADING_CLASS}>
          <i className="bi bi-hourglass-split spinner" aria-hidden="true" />
          <div className="label">Waiting for output</div>
        </div>
      ) : acpPanelProps.acpView.hasAcp ? (
        <AcpPanel {...acpPanelProps} />
      ) : outputs.length === 0 ? (
        <div className={OUTPUT_EMPTY_CLASS}>
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
});

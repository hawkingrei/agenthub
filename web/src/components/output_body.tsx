import { Box } from "@mantine/core";
import React from "react";
import { AcpPanel, AcpPanelProps } from "./acp_panel";
import { TerminalOutput } from "./terminal_output";
import { OutputLine } from "../output_cache";
import {
  OUTPUT_BODY_ACP_ROOT_CLASS,
  OUTPUT_BODY_EMPTY_CLASS,
  OUTPUT_BODY_LOADING_CLASS,
  OUTPUT_BODY_ROOT_CLASS,
} from "../ui/tailwind_classes";

type OutputBodyProps = {
  terminalRef: React.RefObject<HTMLDivElement>;
  onTerminalScroll?: () => void;
  isOutputLoading: boolean;
  isConversationLoading?: boolean;
  outputs: OutputLine[];
  ansi: (input: string) => string;
  acpPanelProps: AcpPanelProps;
};

export const OutputBody = React.memo(function OutputBody({
  terminalRef,
  onTerminalScroll,
  isOutputLoading,
  isConversationLoading = false,
  outputs,
  ansi,
  acpPanelProps,
}: OutputBodyProps) {
  const showLoading = isOutputLoading || isConversationLoading;
  const bodyRootClassName = acpPanelProps.acpView.hasAcp
    ? OUTPUT_BODY_ACP_ROOT_CLASS
    : OUTPUT_BODY_ROOT_CLASS;
  return (
    <Box className={bodyRootClassName}>
      {showLoading ? (
        <Box className={OUTPUT_BODY_LOADING_CLASS}>
          <i className="bi bi-hourglass-split spinner" aria-hidden="true" />
          <Box className="label">Loading...</Box>
        </Box>
      ) : acpPanelProps.acpView.hasAcp ? (
        <AcpPanel {...acpPanelProps} />
      ) : outputs.length === 0 ? (
        <Box className={OUTPUT_BODY_EMPTY_CLASS}>
          <Box className="title">No output yet</Box>
          <Box className="meta">Send input or start the agent to see output.</Box>
        </Box>
      ) : (
        <TerminalOutput
          outputs={outputs}
          ansi={ansi}
          containerRef={terminalRef}
          onScroll={onTerminalScroll}
        />
      )}
    </Box>
  );
});

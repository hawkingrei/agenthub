import { Box } from "@mantine/core";
import React from "react";
import { OutputLine } from "../output_cache";

type TerminalOutputProps = {
  outputs: OutputLine[];
  ansi: (input: string) => string;
  containerRef?: React.RefObject<HTMLDivElement>;
  onScroll?: () => void;
};

const TERMINAL_CONTAINER_CLASS =
  "terminal flex-1 min-h-0 overflow-auto rounded-lg border border-notion-border bg-brand-primary p-3 font-mono text-[12px] leading-relaxed text-white shadow-inner";
const TERMINAL_LINE_BASE_CLASS = "line m-0 min-w-max whitespace-pre [tab-size:2]";
const TERMINAL_STDOUT_CLASS = "text-white";
const TERMINAL_STDERR_CLASS = "text-red-300";
const TERMINAL_SYSTEM_CLASS = "text-cyan-300";

export function TerminalOutput({
  outputs,
  ansi,
  containerRef,
  onScroll,
}: TerminalOutputProps) {
  return (
    <Box
      className={TERMINAL_CONTAINER_CLASS}
      ref={containerRef}
      onScroll={onScroll}
    >
      {outputs.map((line) => {
        const key = `id-${line.event_id}`;
        const toneClass =
          line.stream === "stderr"
            ? TERMINAL_STDERR_CLASS
            : line.stream === "system"
              ? TERMINAL_SYSTEM_CLASS
              : TERMINAL_STDOUT_CLASS;
        return (
          <Box
            component="pre"
            key={key}
            className={`${TERMINAL_LINE_BASE_CLASS} ${line.stream} ${toneClass}`}
            dangerouslySetInnerHTML={{
              __html: ansi(line.message),
            }}
          />
        );
      })}
    </Box>
  );
}

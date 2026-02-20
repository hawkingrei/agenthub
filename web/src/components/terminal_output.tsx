import React from "react";
import { OutputLine } from "../output_cache";

type TerminalOutputProps = {
  outputs: OutputLine[];
  ansi: (input: string) => string;
  containerRef?: React.RefObject<HTMLDivElement>;
  onScroll?: () => void;
};

const TERMINAL_CONTAINER_CLASS =
  "terminal h-full overflow-auto rounded-xl border border-slate-200/60 bg-slate-950 px-2 py-2 font-mono text-[13px] leading-6 text-slate-100";
const TERMINAL_LINE_BASE_CLASS = "line break-words whitespace-pre-wrap px-2";
const TERMINAL_STDOUT_CLASS = "text-slate-100";
const TERMINAL_STDERR_CLASS = "text-rose-300";
const TERMINAL_SYSTEM_CLASS = "text-cyan-300";

export function TerminalOutput({
  outputs,
  ansi,
  containerRef,
  onScroll,
}: TerminalOutputProps) {
  return (
    <div
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
          <div
            key={key}
            className={`${TERMINAL_LINE_BASE_CLASS} ${line.stream} ${toneClass}`}
            dangerouslySetInnerHTML={{
              __html: ansi(line.message),
            }}
          />
        );
      })}
    </div>
  );
}

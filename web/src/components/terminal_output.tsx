import React from "react";
import { OutputLine } from "../output_cache";

type TerminalOutputProps = {
  outputs: OutputLine[];
  ansi: (input: string) => string;
  containerRef?: React.RefObject<HTMLDivElement>;
  onScroll?: () => void;
};

export function TerminalOutput({
  outputs,
  ansi,
  containerRef,
  onScroll,
}: TerminalOutputProps) {
  return (
    <div className="terminal" ref={containerRef} onScroll={onScroll}>
      {outputs.map((line) => {
        const key = `id-${line.event_id}`;
        return (
        <div
          key={key}
          className={`line ${line.stream}`}
          dangerouslySetInnerHTML={{
            __html: ansi(line.message),
          }}
        />
        );
      })}
    </div>
  );
}

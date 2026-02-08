import React from "react";
import { OutputLine } from "../output_cache";

type TerminalOutputProps = {
  outputs: OutputLine[];
  ansi: (input: string) => string;
};

export function TerminalOutput({ outputs, ansi }: TerminalOutputProps) {
  return (
    <div className="terminal">
      {outputs.map((line) => {
        const key =
          line.seq != null
            ? `seq-${line.seq}`
            : `${line.ts}-${line.stream}-${line.message}`;
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

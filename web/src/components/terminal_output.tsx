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

declare module "@chenglou/pretext" {
  export interface PreparedText {
    text: string;
  }

  export function prepare(text: string, font: string): PreparedText;
  export function layout(
    prepared: PreparedText,
    maxWidth: number,
    lineHeight: number
  ): { lineCount: number; height: number };
}

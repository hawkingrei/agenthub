import type { MantineThemeOverride } from "@mantine/core";

export const mantineTheme: MantineThemeOverride = {
  fontFamily: '"Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif',
  fontFamilyMonospace:
    '"Source Code Pro", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
  primaryColor: "notion-accent",
  defaultRadius: "sm",
  radius: {
    xs: "2px",
    sm: "4px",
    md: "6px",
    lg: "8px",
    xl: "12px",
  },
  colors: {
    "notion-accent": [
      "#e8f2fd",
      "#d1e5fb",
      "#a3cbf7",
      "#75b1f3",
      "#4797ef",
      "#2383e2", // notion accent blue
      "#1c69b5",
      "#154f88",
      "#0e355a",
      "#071a2d",
    ],
  },
  headings: {
    fontFamily: '"Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif',
    fontWeight: "700",
  },
};

import type { MantineThemeOverride } from "@mantine/core";

export const mantineTheme: MantineThemeOverride = {
  fontFamily: '"Space Grotesk", system-ui, sans-serif',
  fontFamilyMonospace:
    '"Source Code Pro", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
  primaryColor: "brand",
  primaryShade: 6,
  defaultRadius: "sm",
  radius: {
    sm: "8px",
    md: "10px",
    lg: "16px",
  },
  colors: {
    brand: [
      "#eef4f8",
      "#d9e7f2",
      "#b3cde4",
      "#8cb3d5",
      "#669ac7",
      "#3f80b8",
      "#24689f",
      "#1b3a57",
      "#143045",
      "#0f2435",
    ],
  },
  headings: {
    fontFamily: '"Space Grotesk", system-ui, sans-serif',
    fontWeight: "600",
  },
};

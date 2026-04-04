/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  // Keep current app look stable during gradual migration from handcrafted CSS.
  corePlugins: {
    preflight: false,
  },
  theme: {
    extend: {
      colors: {
        brand: {
          primary: "#0f172a",
          "primary-hover": "#1e293b",
        },
        notion: {
          bg: "#ffffff",
          sidebar: "#fbfbfa",
          text: "#37352f",
          "text-muted": "#787774",
          border: "#e2e8f0",
          hover: "#efefee",
          accent: "#2383e2",
          "accent-bg": "rgba(35, 131, 226, 0.05)",
        },
        ui: {
          surface: "#ffffff",
          "surface-soft": "#f8fafc",
          "surface-muted": "#f1f5f9",
          border: "#e2e8f0",
          "border-strong": "#cbd5e1",
          "border-emphasis": "#64748b",
          "text-primary": "#0f172a",
          "text-secondary": "#334155",
          "text-muted": "#64748b",
          "text-inverse": "#ffffff",
        },
        state: {
          info: {
            bg: "#eff6ff",
            border: "#bfdbfe",
            text: "#1d4ed8",
          },
          warning: {
            bg: "#fffbeb",
            border: "#fde68a",
            text: "#b45309",
          },
          success: {
            bg: "#ecfdf5",
            border: "#86efac",
            text: "#15803d",
          },
        },
      },
      spacing: {
        "ctrl-x": "0.75rem",
        "ctrl-y": "0.5rem",
        "ctrl-y-sm": "0.375rem",
      },
      fontSize: {
        "ui-xs": ["0.75rem", { lineHeight: "1rem" }],
        "ui-sm": ["0.875rem", { lineHeight: "1.25rem" }],
      },
    },
  },
  plugins: [],
};

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
          "bg-subtle": "#f7f7f5",
          "bg-soft": "#fbfbfa",
          "bg-panel": "#f8f5ee",
          sidebar: "#f7f7f5",
          "code-bg": "#1e1e1e",
          "payload-bg": "#fcfcfa",
          "payload-border": "#dde2db",
          "plan-bg": "#fffbf1",
          "plan-border": "#efd9a9",
          "plan-progress": "#e7ebe5",
          "plan-progress-from": "#203b2d",
          "plan-progress-to": "#4b6b5d",
          "surface-card": "rgba(255,255,255,0.94)",
          "surface-overlay": "rgba(255,255,255,0.88)",
          "surface-overlay-strong": "rgba(255,255,255,0.98)",
          "surface-elevated": "rgba(255,255,255,0.92)",
          "surface-tint": "#f0f1f0",
          text: "#37352f",
          "text-muted": "#787774",
          border: "#e9e9e7",
          "border-subtle": "rgba(0,0,0,0.06)",
          "border-faint": "rgba(0,0,0,0.05)",
          hover: "#f1f1ef",
          "hover-subtle": "rgba(0,0,0,0.03)",
          "hover-soft": "rgba(0,0,0,0.035)",
          "hover-strong": "rgba(0,0,0,0.06)",
          accent: "#2383e2",
          "accent-bg": "#e8f2fd",
          "bubble-user": "#edf2ff",
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
      boxShadow: {
        "notion-float":
          "0 20px 24px rgba(25,25,25,0.05), 0 5px 8px rgba(25,25,25,0.027), 0 0 0 1px rgba(42,28,0,0.07)",
        "notion-soft": "0 1px 3px rgba(15,23,42,0.04)",
        "notion-row": "0 1px 2px rgba(15,23,42,0.04)",
        "notion-card": "0 1px 3px rgba(15,23,42,0.05)",
        "notion-tab": "0 1px 1px rgba(15,23,42,0.04)",
        "notion-dock": "0 10px 30px rgba(15,23,42,0.10)",
        "notion-topline": "0 -1px 0 rgba(15,23,42,0.04)",
      },
    },
  },
  plugins: [],
};

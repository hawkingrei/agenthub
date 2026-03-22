import js from "@eslint/js";
import tseslint from "@typescript-eslint/eslint-plugin";
import tsParser from "@typescript-eslint/parser";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";

// Keep the project hook lint contract stable across plugin upgrades.
// New compiler-oriented rules should be adopted intentionally in follow-up changes.
// `eslint-plugin-react` does not yet advertise ESLint 10 support, so keep the
// React-specific lint surface on `react-hooks` until upstream catches up.
const stableReactHookRules = {
  "react-hooks/rules-of-hooks": "error",
  "react-hooks/exhaustive-deps": "warn",
};

const baseLanguageOptions = {
  ecmaVersion: "latest",
  sourceType: "module",
  globals: {
    ...globals.browser,
    ...globals.node,
  },
  parserOptions: {
    ecmaFeatures: {
      jsx: true,
    },
  },
};

export default [
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      "coverage/**",
      "test-results/**",
      "playwright-report/**",
    ],
  },
  js.configs.recommended,
  {
    files: ["**/*.{js,jsx,ts,tsx}"],
    languageOptions: baseLanguageOptions,
    plugins: {
      "react-hooks": reactHooks,
    },
    rules: {
      ...stableReactHookRules,
    },
  },
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      ...baseLanguageOptions,
      parser: tsParser,
    },
    plugins: {
      "@typescript-eslint": tseslint,
    },
    rules: {
      ...tseslint.configs.recommended.rules,
      "no-undef": "off",
      "no-console": "warn",
    },
  },
  {
    files: ["src/app.tsx", "src/pages/**/*.{ts,tsx}"],
    rules: {
      "@typescript-eslint/no-use-before-define": [
        "error",
        {
          functions: false,
          classes: true,
          variables: true,
          enums: true,
          typedefs: false,
        },
      ],
    },
  },
];

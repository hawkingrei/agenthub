import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { App } from "./app";
import "highlight.js/styles/github-dark.css";
import "@mantine/core/styles.css";
import "bootstrap-icons/font/bootstrap-icons.css";
import "./styles.css";
import { mantineTheme } from "./ui/mantine_theme";

const root = document.getElementById("root");
if (!root) {
  throw new Error("root element missing");
}

createRoot(root).render(
  <React.StrictMode>
    <MantineProvider theme={mantineTheme}>
      <App />
    </MantineProvider>
  </React.StrictMode>
);

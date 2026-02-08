import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app";
import { registerSW } from "virtual:pwa-register";
import "highlight.js/styles/github-dark.css";
import "./styles.css";
import "bootstrap-icons/font/bootstrap-icons.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("root element missing");
}

registerSW({ immediate: true });

createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

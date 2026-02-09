import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app";
import "highlight.js/styles/github-dark.css";
import "./styles.css";
import "bootstrap-icons/font/bootstrap-icons.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("root element missing");
}

createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

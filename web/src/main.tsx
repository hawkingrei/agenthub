import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app";
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

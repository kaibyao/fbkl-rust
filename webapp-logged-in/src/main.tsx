import "./App.css"; // Doesn't work w/ aliased paths
import "@fontsource/open-sans";
import "@logged-in/src/preload-polyfill";
import * as React from "react";
import { App } from "@logged-in/src/App";
import { createRoot } from "react-dom/client";

const rootEl = document.getElementById("fbkl-application");
if (rootEl) {
  createRoot(rootEl).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
} else {
  console.error("Root element not found.");
}

import "@fontsource/open-sans";
import "@public/src/preload-polyfill";
import * as React from "react";
import { App } from "@public/src/App";
import { createRoot } from "react-dom/client";

const rootEl = document.getElementById("fbkl-public");
if (rootEl) {
  createRoot(rootEl).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
} else {
  console.error("Root element not found.");
}

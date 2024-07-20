import { resolve } from "path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    manifest: true,
    rollupOptions: {
      input: "./src/main.tsx",
    },
  },
  resolve: {
    alias: {
      "@logged-in": resolve(__dirname),
      "@components": resolve(__dirname, "src", "components"),
      "@routes": resolve(__dirname, "src", "routes"),
    },
  },
  server: {
    port: 3100,
  },
});

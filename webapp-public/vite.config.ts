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
      "@public/": "../",
      "@components/": "../src/components/",
      "@routes/": "../src/routes/",
    },
  },
  server: {
    port: 3200,
  },
});

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(() => {
  return {
    plugins: [react()],
    build: {
      manifest: true,
    },
    resolve: {
      alias: {
        "@logged-in/": "../",
        "@components/": "../src/components/",
        "@routes/": "../src/routes/",
      },
    },
  };
});

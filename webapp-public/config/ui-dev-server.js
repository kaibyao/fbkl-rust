import { createServer } from "vite";
import { resolve } from "path";

(async () => {
  const server = await createServer({
    configFile: resolve(__dirname, "./vite.config.js"),
    root: resolve(__dirname, ".."),
    server: {
      port: 3200,
    },
  });

  console.log("Building webapp-public...");
  await server.listen();
  server.printUrls();
  console.log("🚀 Launched the webapp-public dev server 🚀");
})();

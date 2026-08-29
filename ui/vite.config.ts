import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": new URL("./src", import.meta.url).pathname } },
  // dragonfly-core owns the API. proxying keeps the dev origin identical to the
  // kiosk origin, so nothing behaves differently between `just ui` and `just kiosk`.
  server: {
    proxy: {
      "/ws": { target: "ws://127.0.0.1:8787", ws: true },
      "/api": { target: "http://127.0.0.1:8787" },
    },
  },
});

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    host: "127.0.0.1",
    port: 47880,
    strictPort: true,
    proxy: {
      "/v1": { target: "http://127.0.0.1:47890", ws: true },
      "/health": "http://127.0.0.1:47890",
      "/ready": "http://127.0.0.1:47890",
      "/version": "http://127.0.0.1:47890",
      "/openapi.json": "http://127.0.0.1:47890",
      "/docs": "http://127.0.0.1:47890",
      "/metrics": "http://127.0.0.1:47890",
    },
  },
  preview: {
    host: "127.0.0.1",
    port: 47880,
    proxy: {
      "/v1": "http://127.0.0.1:47890",
      "/health": "http://127.0.0.1:47890",
      "/ready": "http://127.0.0.1:47890",
      "/version": "http://127.0.0.1:47890",
      "/openapi.json": "http://127.0.0.1:47890",
      "/docs": "http://127.0.0.1:47890",
      "/metrics": "http://127.0.0.1:47890",
    },
  },
  test: {
    environment: "jsdom",
  },
});

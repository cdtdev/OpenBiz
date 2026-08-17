import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    // Emitted assets are embedded into the Rust binary (CLAUDE.md §1: one binary).
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    proxy: {
      "/healthz": "http://127.0.0.1:8080",
      "/api": "http://127.0.0.1:8080",
    },
  },
});

/// <reference types="vitest/config" />
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
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
    // Stated rather than left to the default. A test step that reports success because it found
    // nothing to run is worse than having no step: CI goes green for a component tree nobody has
    // asserted anything about, and the gap is invisible precisely where it would be noticed.
    passWithNoTests: false,
  },
});

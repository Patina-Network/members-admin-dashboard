import react from "@vitejs/plugin-react";
import tsconfigPaths from "vite-tsconfig-paths";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react(), tsconfigPaths()],
  test: {
    environment: "jsdom",
    setupFiles: ["src/lib/test/setup.ts"],
    clearMocks: true,
  },
  server: {
    proxy: {
      "/version": "http://localhost:8080",
      "/api": {
        target: "http://localhost:8080",
        changeOrigin: false,
      },
    },
  },
});

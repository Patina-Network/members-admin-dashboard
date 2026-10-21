import { defineConfig } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "http://localhost:8080/api/openapi.json",
  output: "src/lib/api/generated",
  plugins: [
    "@hey-api/typescript",
    "@hey-api/sdk",
    {
      name: "@hey-api/client-fetch",
      runtimeConfigPath: "./src/lib/api/hey-api.ts",
    },
    "@tanstack/react-query",
  ],
});

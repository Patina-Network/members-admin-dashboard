import type { CreateClientConfig } from "@/lib/api/generated/client.gen";

export const createClientConfig: CreateClientConfig = (config) => ({
  ...config,
  baseUrl: "",
});

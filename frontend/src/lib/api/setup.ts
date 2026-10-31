import { client } from "@/lib/api/generated/client.gen";
import { authQueryKey } from "@/lib/api/queries/auth";
import { queryClient } from "@/lib/queryClient";

export function setupApi() {
  client.interceptors.response.use((response) => {
    if (response.status === 401 || response.status === 403) {
      void queryClient.invalidateQueries({ queryKey: authQueryKey });
    }

    return response;
  });
}

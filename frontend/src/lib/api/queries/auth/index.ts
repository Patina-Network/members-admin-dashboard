import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { currentSession, logout } from "@/lib/api/generated/sdk.gen";

export const authQueryKey = ["auth", "session"] as const;

export function useAuthQuery() {
  return useQuery({
    queryKey: authQueryKey,
    queryFn: async ({ signal }) => {
      const { data } = await currentSession({ signal, throwOnError: true });
      return data;
    },
    retry: false,
    staleTime: 0,
    refetchInterval: 60_000,
    refetchOnWindowFocus: "always",
  });
}

export function useLogoutMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (csrfToken: string) => {
      const { data } = await logout({
        headers: { "X-CSRF-Token": csrfToken },
        throwOnError: true,
      });
      return data;
    },
    onSuccess: async ({ redirectUrl }) => {
      await queryClient.cancelQueries();
      queryClient.clear();
      window.location.assign(redirectUrl);
    },
  });
}

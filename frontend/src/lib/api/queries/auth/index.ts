import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { currentSession, logout } from "@/lib/api/generated/sdk.gen";

export const authQueryKey = ["auth", "session"] as const;

class SessionRequestError extends Error {
  constructor(readonly status: number) {
    super("Unable to check your session.");
  }
}

export function useAuthQuery() {
  return useQuery({
    queryKey: authQueryKey,
    queryFn: async ({ signal }) => {
      const { data, error, response } = await currentSession({ signal });
      if (!data) {
        if (!response) throw error;
        throw new SessionRequestError(response.status);
      }
      return data;
    },
    retry: (failureCount, error) =>
      failureCount < 2 &&
      (error instanceof TypeError ||
        (error instanceof SessionRequestError &&
          [408, 429, 500, 502, 503, 504].includes(error.status))),
    retryDelay: (attempt) => Math.min(500 * 2 ** attempt, 1000),
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

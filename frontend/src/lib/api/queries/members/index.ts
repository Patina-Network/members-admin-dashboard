import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type { MemberList, NewMember } from "@/lib/api/generated/types.gen";

import { listQueryKey } from "@/lib/api/generated/@tanstack/react-query.gen";
import { create, currentSession, list } from "@/lib/api/generated/sdk.gen";

const membersKey = listQueryKey();
const uncertainWriteMessage =
  "Could not confirm whether the member was added. Reload the page before trying again.";

export const useMembersQuery = () => {
  return useQuery<MemberList>({
    retry: false,
    queryKey: membersKey,
    queryFn: ({ signal }) => fetchMembers(signal),
  });
};

export const useAddMemberMutation = () => {
  const queryClient = useQueryClient();
  return useMutation({
    retry: false,
    mutationFn: createMember,
    onSuccess: async () => {
      await queryClient.cancelQueries({ queryKey: membersKey });
      await queryClient.invalidateQueries({ queryKey: membersKey });
    },
  });
};

async function fetchMembers(signal: AbortSignal) {
  const result = await list({ signal });
  if (!result.data) {
    throw new Error(result.error?.error ?? "Unable to load members");
  }
  return result.data;
}

async function createMember(body: NewMember) {
  const { data: session } = await currentSession();
  if (!session) {
    throw new Error("Unable to check your session. Please try again.");
  }
  if (!session.user?.isAdmin || !session.csrfToken) {
    throw new Error(
      "Please sign in with an admin account before adding members.",
    );
  }
  const result = await create({
    body,
    headers: { "X-CSRF-Token": session.csrfToken },
  }).catch(() => {
    throw new Error(uncertainWriteMessage);
  });
  if (!result.data) {
    if (!result.response || result.response.status >= 500) {
      throw new Error(uncertainWriteMessage);
    }
    throw new Error(
      result.response.status === 409 ?
        "A member with this email or social account already exists."
      : (result.error?.error ?? "Unable to add member. Please try again."),
    );
  }
  return result.data;
}

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type {
  Member,
  MemberList,
  MemberUpdate,
  NewMember,
} from "@/lib/api/generated/types.gen";

import {
  detailsQueryKey,
  listQueryKey,
} from "@/lib/api/generated/@tanstack/react-query.gen";
import {
  archive,
  create,
  currentSession,
  details,
  list,
  update,
} from "@/lib/api/generated/sdk.gen";

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

export const useMemberDetailsQuery = (memberId: number) => {
  return useQuery({
    queryKey: detailsQueryKey({ path: { member_id: memberId } }),
    gcTime: 0,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    queryFn: ({ signal }) => fetchMemberDetails(memberId, signal),
  });
};

async function fetchMemberDetails(memberId: number, signal: AbortSignal) {
  const result = await details({ path: { member_id: memberId }, signal }).catch(
    () => {
      throw new Error("Unable to load member details. Please try again.");
    },
  );
  if (!result.data) {
    const status = result.response?.status;
    throw new Error(
      status === 401 ? "Your session expired. Please sign in again."
      : status === 403 ? "You do not have permission to view this member."
      : status === 404 ? "This member could not be found."
      : "Unable to load member details. Please try again.",
    );
  }
  return result.data;
}

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

async function getAdminCsrfToken(action: "adding" | "updating" | "archiving") {
  const { data: session } = await currentSession();
  if (!session) {
    throw new Error("Unable to check your session. Please try again.");
  }
  if (!session.user?.isAdmin || !session.csrfToken) {
    throw new Error(
      `Please sign in with an admin account before ${action} members.`,
    );
  }
  return session.csrfToken;
}

async function createMember(body: NewMember) {
  const csrfToken = await getAdminCsrfToken("adding");
  const result = await create({
    body,
    headers: { "X-CSRF-Token": csrfToken },
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

export const useUpdateMemberMutation = () => {
  const queryClient = useQueryClient();
  return useMutation({
    retry: false,
    mutationFn: async ({
      memberId,
      body,
    }: {
      memberId: number;
      body: MemberUpdate;
    }) => {
      const csrfToken = await getAdminCsrfToken("updating");
      const result = await update({
        headers: { "X-CSRF-Token": csrfToken },
        path: { member_id: memberId },
        body,
      }).catch(() => {
        throw new Error(
          "Could not confirm whether the member was updated. Reload before trying again.",
        );
      });
      if (!result.data) {
        throw new Error(
          !result.response || result.response.status >= 500 ?
            "Could not confirm whether the member was updated. Reload before trying again."
          : (
            result.response.status === 409 &&
            result.error?.error?.includes("member changed")
          ) ?
            "Another admin changed this member. Close and reopen the form to review their changes before saving."
          : result.response.status === 409 ?
            "A member with this email or social account already exists."
          : (result.error?.error ?? "Unable to update member."),
        );
      }
      return result.data;
    },
    onSuccess: async (member: Member) => {
      await queryClient.cancelQueries({ queryKey: membersKey });
      queryClient.setQueryData<MemberList>(membersKey, (current) =>
        current ?
          {
            ...current,
            members: current.members.map((row) =>
              row.member_id === member.member_id ? member : row,
            ),
          }
        : current,
      );
      void queryClient.invalidateQueries({ queryKey: membersKey });
    },
  });
};

export const useArchiveMemberMutation = () => {
  const queryClient = useQueryClient();
  return useMutation({
    retry: false,
    mutationFn: async (memberId: number) => {
      const uncertain =
        "Could not confirm whether the member was archived. Reload before trying again.";
      const csrfToken = await getAdminCsrfToken("archiving");
      const result = await archive({
        path: { member_id: memberId },
        headers: { "X-CSRF-Token": csrfToken },
      }).catch(() => {
        throw new Error(uncertain);
      });
      if (result.response?.status !== 204) {
        throw new Error(
          !result.response || result.response.status >= 500 ?
            uncertain
          : (result.error?.error ?? "Unable to archive member."),
        );
      }
      return memberId;
    },
    onSuccess: async (memberId) => {
      await queryClient.cancelQueries({ queryKey: membersKey });
      queryClient.setQueryData<MemberList>(membersKey, (current) => {
        if (!current) return current;
        const members = current.members.filter(
          (row) => row.member_id !== memberId,
        );
        return { members, total: members.length };
      });
      void queryClient.invalidateQueries({ queryKey: membersKey });
    },
  });
};

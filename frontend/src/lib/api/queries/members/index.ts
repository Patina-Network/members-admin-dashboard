import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type {
  BulkMembers,
  ListData,
  Member,
  MemberUpdate,
  NewMember,
} from "@/lib/api/generated/types.gen";

import {
  detailsQueryKey,
  listQueryKey,
} from "@/lib/api/generated/@tanstack/react-query.gen";
import {
  archive,
  bulkImport,
  create,
  currentSession,
  details,
  list,
  update,
} from "@/lib/api/generated/sdk.gen";

export const membersPerPage = 10;

const membersKey = listQueryKey();
const uncertainWriteMessage =
  "Could not confirm whether the member was added. Reload the page before trying again.";

export const useMembersQuery = (
  query: string,
  page = 1,
  roles: string[] = [],
  enabled = true,
) => {
  const params = {
    query: query.trim(),
    page,
    page_size: membersPerPage,
    roles: [...roles].sort().join(","),
  };
  return useQuery({
    retry: false,
    enabled,
    queryKey: listQueryKey({ query: params }),
    queryFn: ({ signal }) => fetchMembers(params, signal),
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
      void queryClient.invalidateQueries({ queryKey: membersKey });
    },
  });
};

async function fetchMembers(
  query: NonNullable<ListData["query"]>,
  signal: AbortSignal,
) {
  const result = await list({ query, signal });

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

      void queryClient.invalidateQueries({ queryKey: membersKey });
      void queryClient.invalidateQueries({
        queryKey: detailsQueryKey({ path: { member_id: member.member_id } }),
      });
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
    onSuccess: async () => {
      await queryClient.cancelQueries({ queryKey: membersKey });
      void queryClient.invalidateQueries({ queryKey: membersKey });
    },
  });
};

export const useBulkImportMembersMutation = () => {
  const queryClient = useQueryClient();

  return useMutation({
    retry: false,
    mutationFn: async (body: BulkMembers) => {
      const uncertain =
        "Could not confirm import results. Refresh before retrying.";

      const csrfToken = await getAdminCsrfToken("adding");
      const result = await bulkImport({
        body,
        headers: { "X-CSRF-Token": csrfToken },
      }).catch(() => {
        throw new Error(uncertain);
      });

      if (!result.data)
        throw new Error(
          !result.response || result.response.status >= 500 ?
            uncertain
          : (result.error?.error ?? "Unable to import members."),
        );

      return result.data;
    },
    onSettled: async (data) => {
      await queryClient.cancelQueries({ queryKey: membersKey });

      for (const result of data?.results ?? []) {
        if (result.outcome === "updated")
          void queryClient.invalidateQueries({
            queryKey: detailsQueryKey({
              path: { member_id: result.member.member_id },
            }),
          });
      }

      void queryClient.invalidateQueries({ queryKey: membersKey });
    },
  });
};

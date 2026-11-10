import type { ReactNode } from "react";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";

import type { Member } from "@/lib/api/generated/types.gen";

import { listQueryKey } from "@/lib/api/generated/@tanstack/react-query.gen";
import {
  archive,
  bulkImport,
  create,
  currentSession,
  list,
  update,
} from "@/lib/api/generated/sdk.gen";
import {
  matchesMemberSearch,
  useAddMemberMutation,
  useArchiveMemberMutation,
  useBulkImportMembersMutation,
  useMembersQuery,
  useUpdateMemberMutation,
} from "@/lib/api/queries/members";

vi.mock("@/lib/api/generated/sdk.gen", () => ({
  archive: vi.fn(),
  bulkImport: vi.fn(),
  create: vi.fn(),
  currentSession: vi.fn(),
  details: vi.fn(),
  list: vi.fn(),
  update: vi.fn(),
}));

const member: Member = {
  member_id: 42,
  first_name: "Alice",
  last_name: "Smith",
  email: "alice@example.com",
  joined_at: null,
  discord_id: "123",
  discord_username: "discord-alice",
  instagram_username: "insta-alice",
  linkedin_username: "linked-alice",
  career: "Engineer",
  industry: "Technology",
  role: "mentor",
};
const key = listQueryKey();
function setup(populated = true) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  if (populated) client.setQueryData(key, { members: [member], total: 1 });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, wrapper };
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(currentSession).mockResolvedValue({
    data: { user: { isAdmin: true }, csrfToken: "token" },
  } as Awaited<ReturnType<typeof currentSession>>);
  vi.mocked(list).mockRejectedValue(new Error("refresh failed"));
});

it.each([
  " ALI ",
  "smith",
  "alice@",
  "123",
  "discord-",
  "insta-",
  "linked-",
  "42",
])("matches the server's supported prefixes: %s", (query) => {
  expect(matchesMemberSearch(member, query)).toBe(true);
});
it.each(["lice", "example.com", "Engineer", "Technology", "mentor"])(
  "excludes substrings and unsupported fields: %s",
  (query) => {
    expect(matchesMemberSearch(member, query)).toBe(false);
  },
);

it("updates every search view after a create even when refresh fails", async () => {
  const { wrapper } = setup();
  const added = {
    ...member,
    member_id: 43,
    first_name: "Bob",
    email: "bob@example.com",
  };
  vi.mocked(create).mockResolvedValue({ data: added } as Awaited<
    ReturnType<typeof create>
  >);
  const { result } = renderHook(
    () => ({
      alice: useMembersQuery("alice"),
      bob: useMembersQuery("bob"),
      add: useAddMemberMutation(),
    }),
    { wrapper },
  );
  await act(async () => {
    await result.current.add.mutateAsync({
      email: added.email,
      first_name: "Bob",
      last_name: "Smith",
    });
  });
  await waitFor(() =>
    expect(result.current.bob.data?.members).toEqual([added]),
  );
  expect(result.current.alice.data?.members).toEqual([member]);
  expect(result.current.alice.data?.total).toBe(2);
  expect(result.current.bob.data?.total).toBe(2);
});

it("moves edited members between search views without waiting for refresh", async () => {
  const { wrapper } = setup();
  const changed = { ...member, first_name: "Bob", email: "bob@example.com" };
  vi.mocked(update).mockResolvedValue({ data: changed } as Awaited<
    ReturnType<typeof update>
  >);
  const { result } = renderHook(
    () => ({
      alice: useMembersQuery("alice"),
      bob: useMembersQuery("bob"),
      edit: useUpdateMemberMutation(),
    }),
    { wrapper },
  );
  await act(async () => {
    await result.current.edit.mutateAsync({
      memberId: 42,
      body: {
        first_name: "Bob",
        email: changed.email,
        expected_updated_at: "2020-01-01T00:00:00Z",
      },
    });
  });
  await waitFor(() => expect(result.current.alice.data?.members).toEqual([]));
  expect(result.current.bob.data?.members).toEqual([changed]);
  expect(result.current.bob.data?.total).toBe(1);
});

it("does not decrement an archive already reflected in the cache", async () => {
  const { client, wrapper } = setup();
  vi.mocked(archive).mockImplementation(async () => {
    client.setQueryData(key, {
      members: [{ ...member, member_id: 99 }],
      total: 1,
    });
    return {
      data: undefined,
      request: new Request("http://localhost/api/members/42"),
      response: new Response(null, { status: 204 }),
    };
  });
  const { result } = renderHook(useArchiveMemberMutation, { wrapper });
  await act(async () => {
    await result.current.mutateAsync(42);
  });
  expect(client.getQueryData(key)).toMatchObject({ total: 1 });
});

it("does not turn a pending list into an incomplete list after import", async () => {
  const { client, wrapper } = setup(false);
  vi.mocked(list).mockImplementation(() => new Promise<never>(() => {}));
  vi.mocked(bulkImport).mockResolvedValue({
    data: { results: [{ index: 0, outcome: "created", member }] },
  } as Awaited<ReturnType<typeof bulkImport>>);
  const { result } = renderHook(
    () => ({
      members: useMembersQuery(""),
      bulk: useBulkImportMembersMutation(),
    }),
    { wrapper },
  );
  await act(async () => {
    await result.current.bulk.mutateAsync({ mode: "create", members: [] });
  });
  expect(client.getQueryData(key)).toBeUndefined();
  client.clear();
});

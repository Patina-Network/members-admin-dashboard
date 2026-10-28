import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import DatabasePage from "@/app/database/Database.page";
import {
  create,
  currentSession,
  details,
  list,
  update,
} from "@/lib/api/generated/sdk.gen";

vi.mock("@/lib/api/generated/sdk.gen", () => ({
  create: vi.fn(),
  currentSession: vi.fn(),
  details: vi.fn(),
  update: vi.fn(),
  list: vi.fn(),
}));
const member = {
  member_id: 1,
  email: "person@example.com",
  first_name: "First",
  last_name: "Last",
  joined_at: null,
  discord_id: null,
  discord_username: null,
};
const response = new Response(null, { status: 200 });
function renderPage(mutationRetry: boolean | number = false) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: mutationRetry },
    },
  });
  render(
    <MantineProvider env="test">
      <QueryClientProvider client={queryClient}>
        <DatabasePage />
      </QueryClientProvider>
    </MantineProvider>,
  );
  return queryClient;
}
async function openForm() {
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Add" }));
  await user.click(
    await screen.findByRole("button", { name: "Add One Member" }),
  );
  return user;
}
async function fillForm(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText(/^Email/), "person@example.com");
  await user.type(screen.getByLabelText(/^First Name/), "First");
  await user.type(screen.getByLabelText(/^Last Name/), "Last");
}

function deferred<T>() {
  let resolve: (value: T) => void = () => {
    throw new Error("Promise not initialized");
  };
  const promise = new Promise<T>((complete) => {
    resolve = complete;
  });
  return { promise, resolve };
}

function submitForm() {
  const form = screen.getByRole("dialog").querySelector("form");
  if (!form) throw new Error("Member form is missing");
  fireEvent.submit(form);
}

const savedMember = () => ({
  data: member,
  request: new Request("http://localhost/api/members"),
  response,
});
const memberList = (members: (typeof member)[]) => ({
  data: { members, total: members.length },
  request: new Request("http://localhost/api/members"),
  response,
});

beforeEach(() => {
  vi.mocked(details).mockResolvedValue({
    ...savedMember(),
    data: {
      ...member,
      linkedin_username: "alex",
      instagram_username: null,
      career: "Engineer",
      industry: "Technology",
      profile: "Existing notes",
      role: "mentor",
    },
  });
  vi.mocked(currentSession).mockResolvedValue({
    data: {
      user: { id: "admin", name: "Admin", isAdmin: true },
      csrfToken: "test-csrf",
      loginEnabled: true,
    },
    request: new Request("http://localhost/api/auth/session"),
    response,
  });
  vi.mocked(list).mockResolvedValue({
    data: { members: [], total: 0 },
    request: new Request("http://localhost/api/members"),
    response,
  });
});
describe("add member", () => {
  it("offers both modes, validates, submits, and refreshes the table and count", async () => {
    vi.mocked(create).mockResolvedValue({
      data: member,
      request: new Request("http://localhost/api/members"),
      response,
    });
    renderPage();
    const user = await openForm();
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Add" }),
    );
    expect(await screen.findByText("Enter a valid email")).toBeInTheDocument();
    expect(create).not.toHaveBeenCalled();
    await fillForm(user);
    vi.mocked(list).mockResolvedValue({
      data: { members: [member], total: 1 },
      request: new Request("http://localhost/api/members"),
      response,
    });
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Add" }),
    );
    expect(await screen.findByText("1 member")).toBeInTheDocument();
    expect(screen.getByText(member.email)).toBeInTheDocument();
    expect(create).toHaveBeenCalledWith({
      headers: { "X-CSRF-Token": "test-csrf" },
      body: {
        email: member.email,
        first_name: "First",
        last_name: "Last",
        joined_at: null,
        discord_id: null,
        discord_username: null,
        linkedin_username: null,
        instagram_username: null,
        career: null,
        industry: null,
        profile: null,
        role: null,
      },
    });
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(
      screen.getByRole("button", { name: "Upload Bulk Member(s)" }),
    );
    expect(
      screen.getByText("Drag a CSV here or click to select one"),
    ).toBeInTheDocument();
  });
  it("submits all optional member fields with trimmed values", async () => {
    vi.mocked(create).mockResolvedValue(savedMember());
    renderPage();
    const user = await openForm();
    await fillForm(user);
    const details = screen.getByRole("button", {
      name: /Social & Professional/,
    });
    expect(details).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByLabelText("Role")).toBeVisible();
    await user.click(details);
    expect(details).toHaveAttribute("aria-expanded", "true");
    await user.selectOptions(screen.getByLabelText("Role"), "mentor");
    await user.type(screen.getByLabelText("Discord ID"), "123");
    await user.click(details);
    expect(details).toHaveAttribute("aria-expanded", "false");
    await user.click(details);
    expect(screen.getByLabelText("Discord ID")).toHaveValue("123");
    await user.clear(screen.getByLabelText("Discord ID"));
    await user.type(
      screen.getByLabelText("LinkedIn Username"),
      " alex-rivera ",
    );
    await user.type(
      screen.getByLabelText("Instagram Username"),
      " alex.rivera ",
    );
    await user.type(screen.getByLabelText("Career"), " Engineer ");
    await user.type(screen.getByLabelText("Industry"), " Technology ");
    await user.type(screen.getByLabelText("Profile notes"), " Private notes ");
    submitForm();
    await waitFor(() =>
      expect(create).toHaveBeenCalledWith({
        headers: { "X-CSRF-Token": "test-csrf" },
        body: expect.objectContaining({
          linkedin_username: "alex-rivera",
          instagram_username: "alex.rivera",
          career: "Engineer",
          industry: "Technology",
          profile: "Private notes",
          role: "mentor",
        }),
      }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(screen.getByRole("button", { name: "Add One Member" }));
    expect(screen.getByLabelText("LinkedIn Username")).toHaveValue("");
    expect(screen.getByLabelText("Profile notes")).toHaveValue("");
  });

  it.each([
    "https://linkedin.com/in/alex",
    "@alex",
    "alex rivera",
    "alex?x",
    "alex#bio",
  ])("rejects invalid social usernames: %s", async (username) => {
    renderPage();
    const user = await openForm();
    await fillForm(user);
    await user.click(
      screen.getByRole("button", { name: /Social & Professional/ }),
    );
    await user.type(screen.getByLabelText("LinkedIn Username"), username);
    await user.type(screen.getByLabelText("Instagram Username"), username);
    const details = screen.getByRole("button", {
      name: /Social & Professional/,
    });
    await user.click(details);
    expect(details).toHaveAttribute("aria-expanded", "false");
    submitForm();
    expect(details).toHaveAttribute("aria-expanded", "true");
    expect(
      await screen.findAllByText(
        "Please enter a username without a URL, @, or spaces",
      ),
    ).toHaveLength(2);
    expect(create).not.toHaveBeenCalled();
    expect(screen.getByLabelText("LinkedIn Username")).toHaveValue(username);
  });

  it("shows a list error while keeping the add form available", async () => {
    vi.mocked(list).mockRejectedValue(new Error("Service unavailable"));
    renderPage();
    await waitFor(() => expect(list).toHaveBeenCalled());
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Unable to load members",
    );
    await openForm();
    expect(screen.getByLabelText(/^Email/)).toBeInTheDocument();
  });

  it("validates Discord fields together and rejects future joined dates", async () => {
    renderPage();
    const user = await openForm();
    await fillForm(user);
    await user.click(
      screen.getByRole("button", { name: /Social & Professional/ }),
    );
    await user.type(screen.getByLabelText("Discord ID"), "123");
    const future = new Date(Date.now() + 86400000).toISOString().slice(0, 16);
    fireEvent.change(screen.getByLabelText("Joined At"), {
      target: { value: future },
    });
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Add" }),
    );
    expect(
      await screen.findByText("Discord username is required with an ID"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Joined at must not be in the future"),
    ).toBeInTheDocument();
    expect(create).not.toHaveBeenCalled();
  });
  it("does not create a member when the session check fails", async () => {
    vi.mocked(currentSession).mockResolvedValueOnce({
      data: undefined,
      error: { error: "service unavailable" },
      request: new Request("http://localhost/api/auth/session"),
      response: new Response(null, { status: 503 }),
    });
    renderPage();
    const user = await openForm();
    await fillForm(user);
    submitForm();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Unable to check your session. Please try again.",
    );
    expect(create).not.toHaveBeenCalled();
  });

  it("keeps the form open and preserves values on duplicate email", async () => {
    vi.mocked(create).mockResolvedValue({
      data: undefined,
      error: { error: "already exists" },
      request: new Request("http://localhost/api/members"),
      response: new Response(null, { status: 409 }),
    });
    renderPage();
    const user = await openForm();
    await fillForm(user);
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Add" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "A member with this email or social account already exists.",
    );
    expect(screen.getByLabelText(/^Email/)).toHaveValue(member.email);
  });
  it("blocks same-tick submissions and dismissal until the write finishes", async () => {
    const pending = deferred<ReturnType<typeof savedMember>>();
    vi.mocked(create).mockReturnValue(pending.promise);
    renderPage();
    const user = await openForm();
    await fillForm(user);
    submitForm();
    submitForm();
    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Add" }),
    ).toBeDisabled();
    expect(screen.getByRole("button", { name: "Back" })).toBeDisabled();
    await user.keyboard("{Escape}");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    vi.mocked(list).mockResolvedValue(memberList([member]));
    await act(async () => pending.resolve(savedMember()));
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(
      screen.getByRole("button", { name: "Add One Member" }),
    ).toBeInTheDocument();
  });

  it("does not automatically retry an uncertain write", async () => {
    vi.mocked(create).mockResolvedValue({
      data: undefined,
      error: { error: "upstream timeout" },
      request: new Request("http://localhost/api/members"),
      response: new Response(null, { status: 504 }),
    });
    renderPage(3);
    const user = await openForm();
    await fillForm(user);
    submitForm();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not confirm whether the member was added",
    );
    expect(screen.getByLabelText(/^Email/)).toHaveValue(member.email);
    expect(create).toHaveBeenCalledTimes(1);
    expect(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Add" }),
    ).toBeEnabled();
    vi.mocked(create).mockResolvedValue(savedMember());
    vi.mocked(list).mockResolvedValue(memberList([member]));
    submitForm();
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(create).toHaveBeenCalledTimes(2);
  });
});

describe("edit member", () => {
  it.each([
    [401, "Your session expired"],
    [403, "You do not have permission"],
    [404, "This member could not be found"],
  ])("explains member detail errors for HTTP %s", async (status, message) => {
    vi.mocked(list).mockResolvedValue(memberList([member]));
    vi.mocked(details).mockResolvedValueOnce({
      data: undefined,
      error: { error: "not found" },
      request: new Request("http://localhost/api/members/1"),
      response: new Response(null, { status }),
    });
    renderPage();
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit member 1" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(message);
    expect(
      screen.queryByRole("button", { name: "Save changes" }),
    ).not.toBeInTheDocument();
  });

  it("preserves the exact joined timestamp when editing other fields", async () => {
    vi.mocked(list).mockResolvedValue(memberList([member]));
    const original = "2020-01-02T03:04:05.123456Z";
    vi.mocked(details).mockResolvedValueOnce({
      ...savedMember(),
      data: {
        ...member,
        joined_at: original,
        linkedin_username: null,
        instagram_username: null,
        career: null,
        industry: null,
        profile: null,
        role: null,
      },
    });
    vi.mocked(update).mockResolvedValue(savedMember());
    renderPage();
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit member 1" }),
    );
    await screen.findByLabelText(/^Email/);
    submitForm();
    await waitFor(() =>
      expect(update).toHaveBeenCalledWith(
        expect.objectContaining({
          body: expect.objectContaining({ joined_at: original }),
        }),
      ),
    );
  });

  it("keeps editing unavailable until full details load and allows retry", async () => {
    vi.mocked(list).mockResolvedValue(memberList([member]));
    vi.mocked(details).mockRejectedValueOnce(new Error("Offline"));
    renderPage();
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Edit member 1" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Unable to load member details",
    );
    expect(
      screen.queryByRole("button", { name: "Save changes" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByLabelText("Profile notes")).toHaveValue(
      "Existing notes",
    );
    expect(update).not.toHaveBeenCalled();
  });

  it("saves cleared optional fields and a changed joined date", async () => {
    vi.mocked(list).mockResolvedValue(memberList([member]));
    vi.mocked(update).mockResolvedValue(savedMember());
    renderPage();
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Edit member 1" }),
    );
    await screen.findByLabelText(/^Email/);
    await user.click(
      screen.getByRole("button", { name: /Social & Professional/ }),
    );
    await user.clear(screen.getByLabelText("LinkedIn Username"));
    await user.clear(screen.getByLabelText("Career"));
    await user.clear(screen.getByLabelText("Profile notes"));
    await user.selectOptions(screen.getByLabelText("Role"), "");
    fireEvent.change(screen.getByLabelText("Joined At"), {
      target: { value: "2020-01-02T03:04" },
    });
    submitForm();
    await waitFor(() =>
      expect(update).toHaveBeenCalledWith(
        expect.objectContaining({
          body: expect.objectContaining({
            linkedin_username: null,
            career: null,
            profile: null,
            role: null,
            joined_at: new Date("2020-01-02T03:04").toISOString(),
          }),
        }),
      ),
    );
  });

  it("prefills, validates, saves, and refreshes the row", async () => {
    vi.mocked(list).mockResolvedValue(memberList([member]));
    const changed = { ...member, first_name: "Updated" };
    vi.mocked(update).mockResolvedValue({ ...savedMember(), data: changed });
    renderPage();
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Edit member 1" }),
    );
    await screen.findByLabelText(/^Email/);
    expect(screen.getByLabelText(/^Email/)).toHaveValue(member.email);
    expect(screen.getByLabelText(/^First Name/)).toHaveValue("First");
    expect(screen.getByLabelText("Joined At")).not.toHaveAttribute("readonly");
    expect(screen.getByLabelText("Profile notes")).toHaveValue(
      "Existing notes",
    );
    await user.clear(screen.getByLabelText(/^First Name/));
    submitForm();
    expect(
      await screen.findByText("First name is required"),
    ).toBeInTheDocument();
    expect(update).not.toHaveBeenCalled();
    await user.type(screen.getByLabelText(/^First Name/), "Updated");
    await user.click(
      screen.getByRole("button", { name: /Social & Professional/ }),
    );
    expect(screen.getByLabelText("LinkedIn Username")).toHaveValue("alex");
    await user.clear(screen.getByLabelText("Career"));
    await user.type(screen.getByLabelText("Career"), "Designer");
    await user.clear(screen.getByLabelText("Profile notes"));
    await user.type(screen.getByLabelText("Profile notes"), "Updated notes");
    await user.selectOptions(screen.getByLabelText("Role"), "board");
    vi.mocked(list).mockResolvedValue(memberList([changed]));
    submitForm();
    await waitFor(() =>
      expect(update).toHaveBeenCalledWith({
        headers: { "X-CSRF-Token": "test-csrf" },
        path: { member_id: 1 },
        body: {
          email: member.email,
          first_name: "Updated",
          last_name: "Last",
          discord_id: null,
          discord_username: null,
          joined_at: null,
          linkedin_username: "alex",
          instagram_username: null,
          career: "Designer",
          industry: "Technology",
          profile: "Updated notes",
          role: "board",
        },
      }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Updated")).toBeInTheDocument();
    expect(list).toHaveBeenCalledTimes(2);
  });

  it("clears stale paired Discord errors when either value changes", async () => {
    vi.mocked(list).mockResolvedValue(memberList([member]));
    renderPage();
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Edit member 1" }),
    );
    await screen.findByLabelText(/^Email/);
    await user.click(
      screen.getByRole("button", { name: /Social & Professional/ }),
    );
    await user.type(screen.getByLabelText("Discord ID"), "123");
    submitForm();
    expect(
      await screen.findByText("Discord username is required with an ID"),
    ).toBeInTheDocument();
    await user.clear(screen.getByLabelText("Discord ID"));
    expect(
      screen.queryByText("Discord username is required with an ID"),
    ).not.toBeInTheDocument();
    await user.type(screen.getByLabelText("Discord Username"), "testuser");
    submitForm();
    expect(
      await screen.findByText("Discord ID is required with a username"),
    ).toBeInTheDocument();
    await user.clear(screen.getByLabelText("Discord Username"));
    expect(
      screen.queryByText("Discord ID is required with a username"),
    ).not.toBeInTheDocument();
    expect(update).not.toHaveBeenCalled();
  });

  it("keeps the refreshed row when the subsequent list refresh fails", async () => {
    vi.mocked(list)
      .mockResolvedValueOnce(memberList([member]))
      .mockRejectedValue(new Error("offline"));
    vi.mocked(update).mockResolvedValue({
      ...savedMember(),
      data: { ...member, first_name: "Saved" },
    });
    renderPage();
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Edit member 1" }),
    );
    await screen.findByLabelText(/^Email/);
    await user.clear(screen.getByLabelText(/^First Name/));
    await user.type(screen.getByLabelText(/^First Name/), "Saved");
    submitForm();
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Saved")).toBeInTheDocument();
  });

  it("does not retry uncertain updates and retains entered values", async () => {
    vi.mocked(list).mockResolvedValue(memberList([member]));
    vi.mocked(update).mockResolvedValue({
      data: undefined,
      error: { error: "upstream timeout" },
      request: new Request("http://localhost/api/members/1"),
      response: new Response(null, { status: 504 }),
    });
    renderPage(3);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Edit member 1" }),
    );
    await screen.findByLabelText(/^Email/);
    await user.clear(screen.getByLabelText(/^First Name/));
    await user.type(screen.getByLabelText(/^First Name/), "Keep this");
    submitForm();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not confirm whether the member was updated",
    );
    expect(screen.getByLabelText(/^First Name/)).toHaveValue("Keep this");
    expect(update).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Save changes" })).toBeEnabled();
  });

  it("preserves values on errors and blocks duplicate pending saves", async () => {
    vi.mocked(list).mockResolvedValue(memberList([member]));
    const pending = deferred<{
      data: undefined;
      error: { error: string };
      request: Request;
      response: Response;
    }>();
    vi.mocked(update).mockReturnValue(pending.promise);
    renderPage(3);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Edit member 1" }),
    );
    await screen.findByLabelText(/^Email/);
    submitForm();
    submitForm();
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    await user.keyboard("{Escape}");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await act(async () =>
      pending.resolve({
        data: undefined,
        request: new Request("http://localhost/api/members/1"),
        response: new Response(null, { status: 409 }),
        error: { error: "already exists" },
      }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "already exists",
    );
    expect(screen.getByLabelText(/^Email/)).toHaveValue(member.email);
    expect(update).toHaveBeenCalledTimes(1);
  });
});

describe("member list states", () => {
  it("distinguishes loading from an empty result", async () => {
    const pending = deferred<ReturnType<typeof memberList>>();
    vi.mocked(list).mockReturnValueOnce(pending.promise);
    renderPage();
    expect(screen.getByText("Loading members…")).toBeInTheDocument();
    expect(
      screen.queryByText("Members will appear here"),
    ).not.toBeInTheDocument();
    await act(async () => pending.resolve(memberList([])));
    expect(await screen.findByText("0 members")).toBeInTheDocument();
    expect(screen.getByText("Members will appear here")).toBeInTheDocument();
  });
  it("shows failed refreshes with cached rows and allows retry", async () => {
    vi.mocked(list).mockResolvedValueOnce(memberList([member]));
    const client = renderPage();
    expect(await screen.findByText(member.email)).toBeInTheDocument();
    vi.mocked(list).mockRejectedValueOnce(new Error("Offline"));
    await act(async () => {
      await client.invalidateQueries();
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Unable to refresh members",
    );
    expect(screen.getByText(member.email)).toBeInTheDocument();
    vi.mocked(list).mockResolvedValueOnce(memberList([]));
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("0 members")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
  it("refetches after creation while the initial request is pending", async () => {
    const pending = deferred<ReturnType<typeof memberList>>();
    vi.mocked(list)
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValue(memberList([member]));
    vi.mocked(create).mockResolvedValue(savedMember());
    renderPage();
    const signal = vi.mocked(list).mock.calls[0][0]?.signal;
    const user = await openForm();
    await fillForm(user);
    submitForm();
    expect(await screen.findByText("1 member")).toBeInTheDocument();
    expect(signal?.aborted).toBe(true);
    await act(async () => pending.resolve(memberList([])));
    expect(screen.getByText("1 member")).toBeInTheDocument();
    expect(screen.getByText(member.email)).toBeInTheDocument();
  });
});

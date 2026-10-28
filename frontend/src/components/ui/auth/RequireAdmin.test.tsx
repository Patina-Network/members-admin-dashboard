import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SessionBody } from "@/lib/api/generated/types.gen";

import RequireAdmin from "@/components/ui/auth/RequireAdmin";
import { currentSession } from "@/lib/api/generated/sdk.gen";

vi.mock("@/lib/api/generated/sdk.gen", () => ({
  currentSession: vi.fn(),
  logout: vi.fn(),
}));

function renderDatabase() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MantineProvider>
        <MemoryRouter initialEntries={["/database"]}>
          <Routes>
            <Route path="/login" element={<div>Sign in page</div>} />
            <Route
              path="/database"
              element={
                <RequireAdmin>
                  <div>Protected member data</div>
                </RequireAdmin>
              }
            />
          </Routes>
        </MemoryRouter>
      </MantineProvider>
    </QueryClientProvider>,
  );
  return client;
}

function session(user: SessionBody["user"]) {
  vi.mocked(currentSession).mockResolvedValue({
    data: { user, csrfToken: user ? "csrf" : null, loginEnabled: true },
    request: new Request("http://localhost/api/auth/session"),
    response: new Response(),
  });
}

describe("Database access", () => {
  beforeEach(() => vi.resetAllMocks());

  it("waits for the session before rendering protected content", () => {
    vi.mocked(currentSession).mockReturnValue(new Promise<never>(() => {}));
    renderDatabase();
    expect(screen.getByLabelText("Checking admin access")).toBeInTheDocument();
    expect(screen.queryByText("Protected member data")).not.toBeInTheDocument();
  });

  it("redirects a signed-out user to login", async () => {
    session(null);
    renderDatabase();
    expect(await screen.findByText("Sign in page")).toBeInTheDocument();
    expect(screen.queryByText("Protected member data")).not.toBeInTheDocument();
  });

  it("denies an authenticated user without allowlisted access", async () => {
    session({ id: "user", name: "User", isAdmin: false });
    renderDatabase();
    expect(await screen.findByText("Access denied")).toBeInTheDocument();
    expect(screen.queryByText("Protected member data")).not.toBeInTheDocument();
  });

  it("renders the database for admins", async () => {
    session({ id: "admin", name: "Admin", isAdmin: true });
    renderDatabase();
    expect(
      await screen.findByText("Protected member data"),
    ).toBeInTheDocument();
  });

  it("fails closed when the session request fails", async () => {
    vi.mocked(currentSession).mockRejectedValue(new Error("Unavailable"));
    renderDatabase();
    expect(
      await screen.findByText("Unable to check your session."),
    ).toBeInTheDocument();
    expect(screen.queryByText("Protected member data")).not.toBeInTheDocument();
  });
});

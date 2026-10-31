import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, expect, it, vi } from "vitest";

import AuthControls from "@/components/ui/auth/AuthControls";
import { currentSession } from "@/lib/api/generated/sdk.gen";

vi.mock("@/lib/api/generated/sdk.gen", () => ({
  currentSession: vi.fn(),
  logout: vi.fn(),
}));

function renderControls() {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MantineProvider>
        <MemoryRouter>
          <AuthControls />
        </MemoryRouter>
      </MantineProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => vi.resetAllMocks());

it("links signed-out users to the login page", async () => {
  vi.mocked(currentSession).mockResolvedValue({
    data: { user: null, csrfToken: null },
    request: new Request("http://localhost/api/auth/session"),
    response: new Response(),
  });

  renderControls();

  expect(await screen.findByRole("link", { name: "Sign in" })).toHaveAttribute(
    "href",
    "/login",
  );
});

it("shows the signed-in user and logout control", async () => {
  vi.mocked(currentSession).mockResolvedValue({
    data: {
      user: { id: "admin", name: "Angela", isAdmin: true },
      csrfToken: "csrf",
    },
    request: new Request("http://localhost/api/auth/session"),
    response: new Response(),
  });

  renderControls();

  expect(await screen.findByText("Angela")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Logout" })).toBeInTheDocument();
  expect(
    screen.queryByRole("link", { name: "Sign in" }),
  ).not.toBeInTheDocument();
});

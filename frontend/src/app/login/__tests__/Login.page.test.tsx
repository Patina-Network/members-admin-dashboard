import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, expect, it, vi } from "vitest";

import LoginPage from "@/app/login/Login.page";
import { currentSession } from "@/lib/api/generated/sdk.gen";

vi.mock("@/lib/api/generated/sdk.gen", () => ({
  currentSession: vi.fn(),
  logout: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(currentSession).mockResolvedValue({
    data: { user: null, csrfToken: null },
    request: new Request("http://localhost/api/auth/session"),
    response: new Response(),
  });
});

function renderLogin(error: string) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MantineProvider>
        <MemoryRouter
          initialEntries={[`/login?error=${encodeURIComponent(error)}`]}
        >
          <LoginPage />
        </MemoryRouter>
      </MantineProvider>
    </QueryClientProvider>,
  );
}

it.each([
  "__proto__",
  "constructor",
  "toString",
  "hasOwnProperty",
  "unknown",
  "",
])("ignores undefined error key %s without breaking sign-in", async (error) => {
  renderLogin(error);

  expect(
    await screen.findByRole("link", { name: "Sign in with Microsoft" }),
  ).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

it.each([
  [
    "unavailable",
    "Microsoft sign-in is currently unavailable. Please try again later.",
  ],
  ["cancelled", "Sign-in was cancelled. You can try again."],
  [
    "invalid_callback",
    "Your sign-in request expired or was invalid. Please try again.",
  ],
  [
    "login_failed",
    "We couldn't complete your Microsoft sign-in. Please try again.",
  ],
  [
    "forbidden",
    "You need admin access. Contact an administrator if you need access.",
  ],
])("shows the defined message for %s", async (error, message) => {
  renderLogin(error);

  expect(await screen.findByRole("alert")).toHaveTextContent(message);
});

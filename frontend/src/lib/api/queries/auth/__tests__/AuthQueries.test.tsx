import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import { type ReactNode } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import { client } from "@/lib/api/generated/client.gen";
import { useAuthQuery } from "@/lib/api/queries/auth";

let queryClient: QueryClient;
const fetchMock = vi.fn<typeof fetch>();
const signedOut = { user: null, csrfToken: null };

beforeEach(() => {
  vi.useFakeTimers();
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  client.setConfig({ baseUrl: "http://localhost" });
  queryClient = new QueryClient();
});

afterEach(() => {
  queryClient.clear();
  client.setConfig({ baseUrl: "" });
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

async function startQuery() {
  const hook = renderHook(() => useAuthQuery(), {
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  });
  await act(() => vi.advanceTimersByTimeAsync(1));

  return hook;
}

it("recovers from a network error before showing an error", async () => {
  fetchMock
    .mockRejectedValueOnce(new TypeError("Failed to fetch"))
    .mockResolvedValueOnce(Response.json(signedOut));

  const { result } = await startQuery();

  expect(result.current.isPending).toBe(true);
  expect(fetchMock).toHaveBeenCalledTimes(1);

  await act(() => vi.advanceTimersByTimeAsync(501));

  expect(result.current.data).toEqual(signedOut);
  expect(result.current.isError).toBe(false);
  expect(fetchMock).toHaveBeenCalledTimes(2);
});

it.each([408, 429, 500, 502, 503, 504])(
  "stops after two retries for HTTP %i",
  async (status) => {
    fetchMock.mockImplementation(async () =>
      Response.json({ error: "Unavailable" }, { status }),
    );

    const { result } = await startQuery();
    await act(() => vi.advanceTimersByTimeAsync(1502));

    expect(result.current.isError).toBe(true);
    expect(fetchMock).toHaveBeenCalledTimes(3);

    await act(() => vi.advanceTimersByTimeAsync(5000));
    expect(fetchMock).toHaveBeenCalledTimes(3);
  },
);

it.each([400, 401, 403, 404])("does not retry HTTP %i", async (status) => {
  fetchMock.mockResolvedValue(Response.json({ error: "Denied" }, { status }));

  const { result } = await startQuery();
  await act(() => vi.advanceTimersByTimeAsync(2000));

  expect(result.current.isError).toBe(true);
  expect(fetchMock).toHaveBeenCalledTimes(1);
});

it("does not retry an aborted request", async () => {
  fetchMock.mockRejectedValue(new DOMException("Aborted", "AbortError"));

  const { result } = await startQuery();
  await act(() => vi.advanceTimersByTimeAsync(2000));

  expect(result.current.isError).toBe(true);
  expect(fetchMock).toHaveBeenCalledTimes(1);
});

it("cancels a pending retry when the query is unmounted", async () => {
  fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));

  const { unmount } = await startQuery();
  unmount();
  await act(() => vi.advanceTimersByTimeAsync(2000));

  expect(fetchMock).toHaveBeenCalledTimes(1);
});

it("accepts a signed-out response without retries", async () => {
  fetchMock.mockResolvedValue(Response.json(signedOut));

  const { result } = await startQuery();

  expect(result.current.data?.user).toBeNull();
  expect(result.current.isSuccess).toBe(true);
  expect(fetchMock).toHaveBeenCalledTimes(1);
});

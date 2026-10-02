import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RulesPage } from "./screens/Rules";
import "@testing-library/jest-dom/vitest";
import { describe, expect, it, vi, beforeEach } from "vitest";

function mockFetch(routes: Record<string, unknown>) {
  globalThis.fetch = vi.fn(async (url: unknown) => {
    const path = String(url).split("?")[0];
    const body = routes[path] ?? {};
    return { ok: true, status: 200, text: async () => JSON.stringify(body) } as Response;
  });
}

function renderRules() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <RulesPage />
    </QueryClientProvider>
  );
}

describe("RulesPage learn flow", () => {
  beforeEach(() => {
    mockFetch({
      "/api/v2/rules": { rules: [] },
      "/api/v2/rules/learn": { suggestions: [{ host: "example.com", folder: "tech", count: 3 }] },
    });
  });

  it("shows suggestions and creates them", async () => {
    const user = userEvent.setup();
    renderRules();
    expect(await screen.findByText(/example\.com/)).toBeInTheDocument();
    await user.click(screen.getByText("Create all suggestions"));
    await waitFor(() => {
      const calls = (globalThis.fetch as unknown as { mock: { calls: unknown[][] } }).mock.calls;
      expect(calls.some((c) => String(c[0]).endsWith("/api/v2/rules/learn") && (c[1] as RequestInit)?.method === "POST")).toBe(true);
    });
  });
});

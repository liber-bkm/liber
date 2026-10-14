import { fireEvent, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SavedSearches } from "./components/Layout";
import "@testing-library/jest-dom/vitest";
import { describe, expect, it, vi } from "vitest";

describe("SavedSearches sidebar", () => {
  it("lists presets and applies on click", async () => {
    globalThis.fetch = vi.fn(async (url: unknown) => {
      const path = String(url);
      if (path.endsWith("/api/v2/searches")) {
        return {
          ok: true,
          status: 200,
          text: async () =>
            JSON.stringify({ searches: [{ name: "rust", query: "tag:rust", scope: "", sort: "newest", deep: true }] }),
        } as Response;
      }
      return { ok: true, status: 200, text: async () => JSON.stringify({}) } as Response;
    });

    const applied: string[] = [];
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <SavedSearches onApply={(s) => applied.push(s.name)} />
      </QueryClientProvider>
    );

    const row = await screen.findByText("rust");
    expect(row).toBeInTheDocument();
    expect(screen.getByText("deep")).toBeInTheDocument();
    fireEvent.click(row);
    expect(applied).toEqual(["rust"]);
  });

  it("renders nothing without presets", async () => {
    globalThis.fetch = vi.fn(async () => {
      return {
        ok: true,
        status: 200,
        text: async () => JSON.stringify({ searches: [] }),
      } as Response;
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={qc}>
        <SavedSearches onApply={() => {}} />
      </QueryClientProvider>
    );

    await new Promise((r) => setTimeout(r, 50));
    expect(container.innerHTML).not.toContain("Saved searches");
  });
});

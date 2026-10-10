import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Library } from "./screens/Library";
import "@testing-library/jest-dom/vitest";
import { describe, expect, it, vi } from "vitest";

function bookmark(i: number) {
  return {
    uuid: "u-" + String(i).padStart(4, "0"),
    short_id: i + 1,
    url: "https://example.com/" + i,
    title: "Item " + i,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    has_markdown: false,
    has_archive: false,
  };
}

describe("Library virtualization", () => {
  it("renders result rows through the window virtualizer", async () => {
    window.scrollTo = () => {};
    window.scrollBy = () => {};
    const items = [0, 1, 2].map(bookmark);
    globalThis.fetch = vi.fn(async () => {
      return {
        ok: true,
        status: 200,
        text: async () =>
          JSON.stringify({ total: items.length, page: 1, per_page: 500, bookmarks: items }),
      } as Response;
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={qc}>
        <Library folder="f" tag={null} onOpen={() => {}} />
      </QueryClientProvider>
    );

    await waitFor(() => {
      expect(container.innerHTML).toContain("Item 0");
    });
    expect(container.innerHTML).toContain("Item 2");
    const rows = container.querySelectorAll("[data-index]");
    expect(rows.length).toBe(items.length);
    for (const el of [...rows]) {
      expect((el as HTMLElement).style.position).toBe("absolute");
      expect((el as HTMLElement).style.transform).toMatch(/^translateY\(/);
    }
    await waitFor(() => {
      expect(screen.getByText("3 results")).toBeInTheDocument();
    });
  });
});

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

  it("shows a small favicon before the title in table view", async () => {
    window.scrollTo = () => {};
    window.scrollBy = () => {};
    const items = [0, 1].map(bookmark);
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
        <Library folder={null} tag={null} onOpen={() => {}} />
      </QueryClientProvider>
    );

    await waitFor(() => {
      expect(container.innerHTML).toContain("Item 0");
    });
    const toggle = container.querySelector('button[title="Table view"]');
    if (!toggle) throw new Error("table toggle missing");
    fireEvent.click(toggle);

    await waitFor(() => {
      const imgs = container.querySelectorAll('button img[alt=""]');
      expect(imgs.length).toBe(items.length);
    });
    const imgs = container.querySelectorAll('button img[alt=""]');
    for (const img of [...imgs]) {
      expect(img.getAttribute("src")).toContain("/api/v2/favicons/example.com");
      expect(img.getAttribute("class")).toContain("h-5");
    }
  });

  it("expands a deep-search snippet on click without opening", async () => {
    window.scrollTo = () => {};
    window.scrollBy = () => {};
    const items = [{ ...bookmark(0), snippet: "match with <b>wombat</b> inside" }];
    globalThis.fetch = vi.fn(async () => {
      return {
        ok: true,
        status: 200,
        text: async () =>
          JSON.stringify({ total: items.length, page: 1, per_page: 500, bookmarks: items }),
      } as Response;
    });

    const opened: string[] = [];
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={qc}>
        <Library folder={null} tag={null} onOpen={(b) => opened.push(b.uuid)} />
      </QueryClientProvider>
    );

    await waitFor(() => {
      expect(container.innerHTML).toContain("wombat");
    });
    const snippet = container.querySelector('span[title="Show full match"]');
    if (!snippet) throw new Error("snippet missing");
    expect(snippet.getAttribute("class")).toContain("line-clamp-2");
    fireEvent.click(snippet);
    const expanded = container.querySelector('span[title="Show less"]');
    if (!expanded) throw new Error("snippet did not expand");
    expect(expanded.getAttribute("class")).not.toContain("line-clamp-2");
    expect(opened).toEqual([]);
  });
});

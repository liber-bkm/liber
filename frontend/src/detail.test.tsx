import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AddDialog } from "./components/Detail";
import "@testing-library/jest-dom/vitest";
import { describe, expect, it, vi } from "vitest";

function mockTaxonomy() {
  globalThis.fetch = vi.fn(async (url: unknown) => {
    const path = String(url);
    if (path.endsWith("/api/v2/tags")) {
      return {
        ok: true,
        status: 200,
        text: async () => JSON.stringify({ tags: [{ name: "rust", count: 2 }, { name: "ruby", count: 1 }] }),
      } as Response;
    }
    if (path.endsWith("/api/v2/folders")) {
      return {
        ok: true,
        status: 200,
        text: async () => JSON.stringify({ folders: [{ name: "read", count: 3 }] }),
      } as Response;
    }
    return { ok: true, status: 200, text: async () => JSON.stringify({}) } as Response;
  });
}

describe("AddDialog taxonomy autocomplete", () => {
  it("suggests matching tags and offers folders", async () => {
    mockTaxonomy();
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={qc}>
        <AddDialog onClose={() => {}} onAdded={() => {}} />
      </QueryClientProvider>
    );

    await waitFor(() => {
      expect(container.querySelector("datalist#liber-folder-add option[value='read']")).not.toBeNull();
    });

    const boxes = screen.getAllByRole("textbox");
    const tagsInput = boxes[2] as HTMLInputElement;
    fireEvent.change(tagsInput, { target: { value: "ru" } });

    const suggest = await screen.findByText("rust");
    expect(suggest).toBeInTheDocument();
    fireEvent.click(suggest);
    expect(tagsInput.value).toContain("rust");
  });
});

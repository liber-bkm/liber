import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AddDialog, DetailDrawer } from "./components/Detail";
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

  it("saves several URLs with a per-URL report", async () => {
    globalThis.fetch = vi.fn(async (url: unknown, init?: RequestInit) => {
      const path = String(url);
      if (path.endsWith("/api/v2/tags")) {
        return { ok: true, status: 200, text: async () => JSON.stringify({ tags: [] }) } as Response;
      }
      if (path.endsWith("/api/v2/folders")) {
        return { ok: true, status: 200, text: async () => JSON.stringify({ folders: [] }) } as Response;
      }
      if (path.endsWith("/api/v2/bookmarks")) {
        const payload = JSON.parse(String(init?.body)) as { url: string };
        if (payload.url.includes("dup")) {
          return {
            ok: false,
            status: 409,
            text: async () =>
              JSON.stringify({ error: "duplicate", existing: { uuid: "e-1", title: "Old", url: payload.url } }),
          } as Response;
        }
        return {
          ok: true,
          status: 201,
          text: async () =>
            JSON.stringify({ uuid: "n-1", url: payload.url, title: "New", created_at: new Date().toISOString(), updated_at: new Date().toISOString(), has_markdown: false, has_archive: false }),
        } as Response;
      }
      return { ok: true, status: 200, text: async () => JSON.stringify({}) } as Response;
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <AddDialog onClose={() => {}} onAdded={() => {}} />
      </QueryClientProvider>
    );

    const boxes = screen.getAllByRole("textbox");
    fireEvent.change(boxes[0], { target: { value: "https://example.com/new\nhttps://example.com/dup" } });

    const saveAll = await screen.findByText("Save 2 bookmarks");
    fireEvent.click(saveAll);

    expect(await screen.findByText("Done")).toBeInTheDocument();
    expect(screen.getByText("added")).toBeInTheDocument();
    expect(screen.getByText("duplicate")).toBeInTheDocument();
    expect(screen.getByText(/already saved as Old/)).toBeInTheDocument();
  });
});

function drawerBookmark(attachments: { name: string }[]) {
  return {
    uuid: "d-0001",
    short_id: 1,
    url: "https://example.com/doc",
    title: "Doc",
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    has_markdown: false,
    has_archive: false,
    attachments,
  };
}

describe("DetailDrawer attachments tab", () => {
  it("lists files with download links", async () => {
    const item = drawerBookmark([{ name: "a.pdf" }]);
    globalThis.fetch = vi.fn(async (url: unknown) => {
      return {
        ok: true,
        status: 200,
        text: async () => JSON.stringify(item),
      } as Response;
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <DetailDrawer uuid="d-0001" onClose={() => {}} onChanged={() => {}} />
      </QueryClientProvider>
    );

    fireEvent.click(await screen.findByText("attachments"));
    const link = await screen.findByText("a.pdf");
    expect(link.closest("a")?.getAttribute("href")).toContain("/api/v2/bookmarks/d-0001/attachments/a.pdf");
  });

  it("uploads from the edit form", async () => {
    let uploaded: { name: string }[] = [];
    const calls: string[] = [];
    globalThis.fetch = vi.fn(async (url: unknown, init?: RequestInit) => {
      const path = String(url);
      calls.push(`${init?.method ?? "GET"} ${path}`);
      if (path.endsWith("/api/v2/bookmarks/d-0001/attachments") && init?.method === "POST") {
        const payload = JSON.parse(String(init?.body)) as { name: string };
        uploaded = [{ name: payload.name }];
        return { ok: true, status: 200, text: async () => JSON.stringify({ name: payload.name }) } as Response;
      }
      return {
        ok: true,
        status: 200,
        text: async () => JSON.stringify(drawerBookmark(uploaded)),
      } as Response;
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={qc}>
        <DetailDrawer uuid="d-0001" onClose={() => {}} onChanged={() => {}} />
      </QueryClientProvider>
    );
    const view = within(container);

    fireEvent.click(await view.findByText("Edit"));
    const picker = container.querySelector(
      'input[type="file"]'
    ) as HTMLInputElement | null;
    if (!picker) throw new Error("edit upload input missing");
    fireEvent.change(picker, { target: { files: [new File(["hello"], "note.txt", { type: "text/plain" })] } });

    await waitFor(() => {
      expect(calls.some((c) => c.startsWith("POST") && c.endsWith("/api/v2/bookmarks/d-0001/attachments"))).toBe(true);
    });
    expect(await view.findByText("note.txt")).toBeInTheDocument();
  });
});

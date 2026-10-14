import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { CheckPage } from "./screens/Check";
import "@testing-library/jest-dom/vitest";
import { describe, expect, it, vi } from "vitest";

describe("CheckPage run-apply flow", () => {
  it("runs a check and applies selected rows", async () => {
    const calls: { url: string; init?: RequestInit }[] = [];
    globalThis.fetch = vi.fn(async (url: unknown, init?: RequestInit) => {
      calls.push({ url: String(url), init });
      const path = String(url);
      if (path.endsWith("/api/v2/check/run")) {
        return {
          ok: true,
          status: 200,
          text: async () =>
            JSON.stringify({
              checked: 2,
              ok: 0,
              rows: [
                { uuid: "aaa", title: "Moved", url: "http://x/old", status: "moved", detail: "301", target: "http://x/new" },
                { uuid: "bbb", title: "Dead", url: "http://x/gone", status: "dead", detail: "404" },
              ],
            }),
        } as Response;
      }
      return { ok: true, status: 200, text: async () => JSON.stringify({ updated: 1, quarantined: 1, skipped: [] }) } as Response;
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <CheckPage />
      </QueryClientProvider>
    );
    const user = userEvent.setup();
    await user.click(screen.getByText("Run check"));
    expect(await screen.findByText("Moved")).toBeInTheDocument();

    const boxes = screen.getAllByRole("checkbox");
    for (const box of boxes) await user.click(box);
    await user.click(screen.getByText("Apply selected"));

    await waitFor(() => {
      const apply = calls.find((c) => c.url.endsWith("/api/v2/check/apply"));
      expect(apply).toBeDefined();
      const payload = JSON.parse(String(apply!.init?.body)) as {
        updates: { uuid: string; url: string }[];
        quarantine: string[];
      };
      expect(payload.updates).toEqual([{ uuid: "aaa", url: "http://x/new", retitle: false }]);
      expect(payload.quarantine).toEqual(["bbb"]);
    });
  });

  it("sends the worker count when set", async () => {
    const calls: { url: string; init?: RequestInit }[] = [];
    globalThis.fetch = vi.fn(async (url: unknown, init?: RequestInit) => {
      calls.push({ url: String(url), init });
      return {
        ok: true,
        status: 200,
        text: async () => JSON.stringify({ checked: 0, ok: 0, rows: [] }),
      } as Response;
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={qc}>
        <CheckPage />
      </QueryClientProvider>
    );

    const workers = container.querySelector('input[placeholder="Workers"]');
    if (!workers) throw new Error("workers input missing");
    fireEvent.change(workers, { target: { value: "4" } });
    fireEvent.click(screen.getByText("Run check"));

    await waitFor(() => {
      const run = calls.find((c) => c.url.endsWith("/api/v2/check/run"));
      expect(run).toBeDefined();
      expect(JSON.parse(String(run!.init?.body))).toMatchObject({ workers: 4 });
    });
  });
});

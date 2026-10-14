import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SettingsPage } from "./screens/Settings";
import "@testing-library/jest-dom/vitest";
import { describe, expect, it, vi } from "vitest";

describe("Settings library status", () => {
  it("shows counts plus health hints", async () => {
    globalThis.fetch = vi.fn(async (url: unknown) => {
      const path = String(url);
      if (path.endsWith("/api/v2/settings")) {
        return {
          ok: true,
          status: 200,
          text: async () =>
            JSON.stringify({
              device_id: "dev1",
              active_profile: "default",
              archive_backend: "auto",
              maintenance_status: { bookmarks: 12, oplog_entries: 34, quarantine: 2, short_id_gaps: 1 },
            }),
        } as Response;
      }
      if (path.endsWith("/api/v2/profiles")) {
        return {
          ok: true,
          status: 200,
          text: async () => JSON.stringify({ active: "default", profiles: [] }),
        } as Response;
      }
      return { ok: true, status: 200, text: async () => JSON.stringify({}) } as Response;
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <SettingsPage />
      </QueryClientProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Library status")).toBeInTheDocument();
    });
    expect(await screen.findByText("12 bookmarks")).toBeInTheDocument();
    expect(screen.getByText("34 oplog entries")).toBeInTheDocument();
    expect(screen.getByText("2 quarantined")).toBeInTheDocument();
    expect(screen.getByText("1 short-id gaps")).toBeInTheDocument();
    expect(screen.getByText(/Review the quarantine folder/)).toBeInTheDocument();
  });

  it("refreshes missing icons through the refresh endpoint", async () => {
    const calls: { url: string; init?: RequestInit }[] = [];
    globalThis.fetch = vi.fn(async (url: unknown, init?: RequestInit) => {
      calls.push({ url: String(url), init });
      const path = String(url);
      if (path.endsWith("/api/v2/settings")) {
        return {
          ok: true,
          status: 200,
          text: async () =>
            JSON.stringify({
              device_id: "dev1",
              active_profile: "default",
              archive_backend: "auto",
              maintenance_status: { bookmarks: 1, oplog_entries: 1, quarantine: 0, short_id_gaps: 0 },
            }),
        } as Response;
      }
      if (path.endsWith("/api/v2/profiles")) {
        return {
          ok: true,
          status: 200,
          text: async () => JSON.stringify({ active: "default", profiles: [] }),
        } as Response;
      }
      if (path.endsWith("/api/v2/favicons/refresh")) {
        return {
          ok: true,
          status: 200,
          text: async () => JSON.stringify({ hosts: 2, fetched: 1, cached: 1, failed: [] }),
        } as Response;
      }
      return { ok: true, status: 200, text: async () => JSON.stringify({}) } as Response;
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <SettingsPage />
      </QueryClientProvider>
    );

    expect((await screen.findAllByText("Site icons")).length).toBeGreaterThan(0);
    const buttons = screen.getAllByText("Refresh icons");
    const button = buttons[buttons.length - 1];
    fireEvent.click(button);

    await waitFor(() => {
      const call = calls.find((c) => c.url.endsWith("/api/v2/favicons/refresh"));
      expect(call).toBeDefined();
      expect(String(call!.init?.body)).toContain("refresh_cached");
    });
    expect((await screen.findAllByText(/2 hosts, 1 fetched, 1 already cached/)).length).toBeGreaterThan(0);
  });

  it("shows app plus frontend versions with docs link", async () => {
    globalThis.fetch = vi.fn(async (url: unknown) => {
      const path = String(url);
      if (path.endsWith("/api/v2/settings")) {
        return {
          ok: true,
          status: 200,
          text: async () =>
            JSON.stringify({
              version: "9.9.9",
              device_id: "dev1",
              active_profile: "default",
              archive_backend: "auto",
              maintenance_status: { bookmarks: 0, oplog_entries: 0, quarantine: 0, short_id_gaps: 0 },
            }),
        } as Response;
      }
      if (path.endsWith("/api/v2/profiles")) {
        return {
          ok: true,
          status: 200,
          text: async () => JSON.stringify({ active: "default", profiles: [] }),
        } as Response;
      }
      return { ok: true, status: 200, text: async () => JSON.stringify({}) } as Response;
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={qc}>
        <SettingsPage />
      </QueryClientProvider>
    );
    const view = within(container);

    expect((await screen.findAllByText("About")).length).toBeGreaterThan(0);
    expect((await screen.findAllByText(/app v9\.9\.9/)).length).toBeGreaterThan(0);
    expect((await screen.findAllByText(/frontend v/)).length).toBeGreaterThan(0);
    expect((await screen.findAllByText(/versions differ/)).length).toBeGreaterThan(0);
    const docs = view.getByText("liber-bkm.github.io");
    expect(docs.closest("a")?.getAttribute("href")).toBe("https://liber-bkm.github.io");
  });

  it("exchanges sync bundles through export plus import", async () => {
    const calls: { url: string; init?: RequestInit }[] = [];
    globalThis.fetch = vi.fn(async (url: unknown, init?: RequestInit) => {
      calls.push({ url: String(url), init });
      const path = String(url);
      if (path.endsWith("/api/v2/settings")) {
        return {
          ok: true,
          status: 200,
          text: async () =>
            JSON.stringify({
              version: "0.11.3",
              device_id: "dev1",
              active_profile: "default",
              archive_backend: "auto",
              maintenance_status: { bookmarks: 0, oplog_entries: 0, quarantine: 0, short_id_gaps: 0 },
            }),
        } as Response;
      }
      if (path.endsWith("/api/v2/profiles")) {
        return {
          ok: true,
          status: 200,
          text: async () => JSON.stringify({ active: "default", profiles: [] }),
        } as Response;
      }
      if (path.endsWith("/api/v2/sync/export")) {
        return {
          ok: true,
          status: 200,
          text: async () => JSON.stringify({ entries: [{ op: "upsert" }], count: 1 }),
        } as Response;
      }
      if (path.endsWith("/api/v2/sync/import")) {
        return {
          ok: true,
          status: 200,
          text: async () =>
            JSON.stringify({ merged: 0, inserted: 1, deduped: 0, deleted: 0, rules: 0, renumbered: 0, short_ids_assigned: 0, short_ids_compacted: 0, artifacts: 0, skipped: 0 }),
        } as Response;
      }
      return { ok: true, status: 200, text: async () => JSON.stringify({}) } as Response;
    });
    window.URL.createObjectURL = vi.fn(() => "blob:sync") as unknown as typeof URL.createObjectURL;
    window.URL.revokeObjectURL = vi.fn() as unknown as typeof URL.revokeObjectURL;

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { container } = render(
      <QueryClientProvider client={qc}>
        <SettingsPage />
      </QueryClientProvider>
    );
    const view = within(container);

    fireEvent.click(await view.findByText("Export bundle"));
    await waitFor(() => {
      expect(calls.some((c) => c.url.endsWith("/api/v2/sync/export"))).toBe(true);
    });
    expect((await screen.findAllByText(/Exported 1 oplog entr/)).length).toBeGreaterThan(0);

    const picker = container.querySelector('input[type="file"][accept=".json,application/json"]');
    if (!picker) throw new Error("bundle picker missing");
    fireEvent.change(picker, { target: { files: [new File(['[{"op":"upsert"}]'], "bundle.json", { type: "application/json" })] } });
    fireEvent.click(view.getByText("Import bundle"));
    await waitFor(() => {
      const call = calls.find((c) => c.url.endsWith("/api/v2/sync/import"));
      expect(call).toBeDefined();
      expect(JSON.parse(String(call!.init?.body))).toEqual({ entries: [{ op: "upsert" }] });
    });
    expect((await screen.findAllByText(/1 inserted/)).length).toBeGreaterThan(0);
  });
});

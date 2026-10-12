import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
});

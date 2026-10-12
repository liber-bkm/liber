import { render, screen, waitFor } from "@testing-library/react";
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
});

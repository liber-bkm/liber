import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Play } from "lucide-react";
import { displayId, type CheckRow, type CheckUpdate } from "../api";
import { checkApply, checkRun } from "../tauri";
import { Badge, Button, Empty, Input, Spinner } from "../components/ui";

export function CheckPage() {
  const [report, setReport] = useState<{ checked: number; ok: number; rows: CheckRow[] } | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [stale, setStale] = useState("");
  const [retitleAll, setRetitleAll] = useState(false);
  const [armedDelete, setArmedDelete] = useState(false);
  const [error, setError] = useState("");
  const qc = useQueryClient();

  const run = useMutation({
    mutationFn: () => checkRun(undefined, stale.trim() ? { stale: stale.trim() } : undefined),
    onSuccess: (r) => {
      setReport(r);
      setSelected(new Set());
      setArmedDelete(false);
    },
    onError: (e: Error) => setError(e.message),
  });

  const apply = useMutation({
    mutationFn: (input: { updates: CheckUpdate[]; quarantine: string[]; del?: string[]; confirm?: boolean }) =>
      checkApply(input.updates, input.quarantine, input.del ? { delete: input.del, confirm: input.confirm } : undefined),
    onSuccess: (r) => {
      if (r.confirm_required) {
        setError(r.hint ?? "Delete needs confirmation.");
        return;
      }
      qc.invalidateQueries({ queryKey: ["bookmarks"] });
      run.mutate();
    },
    onError: (e: Error) => setError(e.message),
  });

  function bulkInput(del: string[] = [], confirm = false) {
    const rows = report?.rows ?? [];
    const updates = rows
      .filter((r) => r.status === "moved" && selected.has(r.uuid) && r.target)
      .map((r) => ({ uuid: r.uuid, url: r.target!, retitle: retitleAll }));
    const quarantine = rows
      .filter((r) => (r.status === "dead" || r.status === "uncertain") && selected.has(r.uuid))
      .map((r) => r.uuid);
    return { updates, quarantine, del, confirm };
  }

  function single(input: { updates: CheckUpdate[]; quarantine: string[]; del?: string[]; confirm?: boolean }) {
    setError("");
    apply.mutate(input);
  }

  function toggle(uuid: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(uuid)) next.delete(uuid);
      else next.add(uuid);
      return next;
    });
  }

  const moved = report?.rows.filter((r) => r.status === "moved") ?? [];
  const dead = report?.rows.filter((r) => r.status === "dead") ?? [];
  const uncertain = report?.rows.filter((r) => r.status === "uncertain") ?? [];
  const deletable = (report?.rows ?? []).filter(
    (r) => (r.status === "dead" || r.status === "uncertain") && selected.has(r.uuid)
  );

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-2">
        <h1 className="text-lg font-semibold">Link check</h1>
        <div className="flex items-center gap-2">
          <Input
            value={stale}
            placeholder="Stale, e.g. 7d"
            onChange={(e) => setStale(e.target.value)}
          />
          <Button onClick={() => { setError(""); run.mutate(); }} disabled={run.isPending}>
            <Play className="h-4 w-4" /> {run.isPending ? "Checking..." : report ? "Check again" : "Run check"}
          </Button>
        </div>
      </div>
      {error && <p className="text-sm text-red-600">{error}</p>}
      {run.isPending && (
        <div className="flex justify-center py-8">
          <Spinner />
        </div>
      )}
      {report && (
        <p className="text-sm text-neutral-500">
          {report.ok} ok of {report.checked} checked. Select rows, then apply: moved URLs update, dead links quarantine.
        </p>
      )}
      {!report && !run.isPending && <Empty title="No check run yet" hint="Run a check to find moved and dead links." />}
      {(['moved', 'dead', 'uncertain'] as const).map((status) => {
        const rows = status === "moved" ? moved : status === "dead" ? dead : uncertain;
        if (rows.length === 0) return null;
        return (
          <section key={status}>
            <h2 className="mb-1 text-sm font-semibold capitalize">
              {status} <span className="font-normal text-neutral-400">({rows.length})</span>
            </h2>
            <div className="flex flex-col gap-1">
              {rows.map((r) => (
                <div
                  key={r.uuid}
                  className={`flex items-start gap-2 rounded-xl border px-3 py-2 dark:border-neutral-800 ${selected.has(r.uuid) ? "border-accent-500 bg-accent-50/50 dark:bg-accent-700/10" : "border-neutral-200 bg-white dark:bg-neutral-900"}`}
                >
                  <input
                    type="checkbox"
                    checked={selected.has(r.uuid)}
                    onChange={() => toggle(r.uuid)}
                    className="mt-1 h-4 w-4 accent-[#2549a8]"
                  />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">{r.title}</p>
                    <p className="truncate text-xs text-neutral-400">
                      [{displayId(r)}] {r.detail}
                      {r.target ? ` → ${r.target}` : ""}
                    </p>
                    <div className="mt-1 flex flex-wrap gap-1">
                      {status === "moved" && r.target && (
                        <>
                          <Button variant="outline" onClick={() => single({ updates: [{ uuid: r.uuid, url: r.target! }], quarantine: [] })}>
                            Update
                          </Button>
                          <Button variant="outline" onClick={() => single({ updates: [{ uuid: r.uuid, url: r.target!, retitle: true }], quarantine: [] })}>
                            Update + title
                          </Button>
                        </>
                      )}
                      {status !== "moved" && (
                        <>
                          <Button variant="outline" onClick={() => single({ updates: [], quarantine: [r.uuid] })}>
                            Quarantine
                          </Button>
                          <Button
                            variant="outline"
                            onClick={() => {
                              if (window.confirm(`Delete ${r.title}?`)) {
                                single({ updates: [], quarantine: [], del: [r.uuid], confirm: true });
                              }
                            }}
                          >
                            Delete
                          </Button>
                        </>
                      )}
                    </div>
                  </div>
                  <span className="ml-auto shrink-0">
                    <Badge tone={status === "dead" ? "red" : status === "moved" ? "amber" : "neutral"}>{status}</Badge>
                  </span>
                </div>
              ))}
            </div>
          </section>
        );
      })}
      {selected.size > 0 && (
        <div className="fixed bottom-4 left-1/2 z-40 flex -translate-x-1/2 flex-wrap items-center gap-2 rounded-2xl border border-neutral-200 bg-white px-3 py-2 shadow-lg dark:border-neutral-700 dark:bg-neutral-900">
          <span className="text-sm text-neutral-500">{selected.size} selected</span>
          <label className="flex items-center gap-1 text-xs text-neutral-500">
            <input type="checkbox" checked={retitleAll} onChange={(e) => setRetitleAll(e.target.checked)} />
            Refresh titles
          </label>
          <Button onClick={() => { setError(""); setArmedDelete(false); apply.mutate(bulkInput()); }} disabled={apply.isPending}>
            {apply.isPending ? "Applying..." : "Apply selected"}
          </Button>
          {deletable.length > 0 && (
            <Button
              variant="outline"
              onClick={() => {
                setError("");
                if (armedDelete) {
                  apply.mutate(bulkInput(deletable.map((r) => r.uuid), true));
                  setArmedDelete(false);
                } else {
                  setArmedDelete(true);
                }
              }}
              disabled={apply.isPending}
            >
              {armedDelete ? `Sure? Delete ${deletable.length}` : `Delete ${deletable.length}`}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}

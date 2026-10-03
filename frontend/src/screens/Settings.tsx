import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, RefreshCw, Upload } from "lucide-react";
import { exportSite, fetchSettings, importLibrary, runReindex, setSetting } from "../api";
import { Button, Field, Input, Spinner } from "../components/ui";

const BACKENDS = ["auto", "builtin", "browser", "single-file", "monolith"];

export function SettingsPage() {
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [prune, setPrune] = useState(false);
  const [compactIds, setCompactIds] = useState(false);
  const [importText, setImportText] = useState("");
  const qc = useQueryClient();
  const settings = useQuery({ queryKey: ["settings"], queryFn: fetchSettings });

  const save = useMutation({
    mutationFn: ({ key, value }: { key: string; value: string }) => setSetting(key, value),
    onSuccess: () => {
      setNotice("Saved.");
      qc.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (e: Error) => setError(e.message),
  });

  const reindex = useMutation({
    mutationFn: () => runReindex(prune, compactIds),
    onSuccess: (r) => {
      const compacted = r.short_ids_compacted ? `, ${r.short_ids_compacted} ids compacted` : "";
      setNotice(`Reindexed: ${r.adopted} adopted, ${r.indexed} indexed, ${r.pruned} pruned${compacted}.`);
      qc.invalidateQueries({ queryKey: ["bookmarks"] });
    },
    onError: (e: Error) => setError(e.message),
  });

  const doImport = useMutation({
    mutationFn: () => importLibrary(importText, false),
    onSuccess: (r) => {
      setNotice(`Imported ${r.added}, skipped ${r.skipped_dup} duplicates.`);
      setImportText("");
      qc.invalidateQueries({ queryKey: ["bookmarks"] });
    },
    onError: (e: Error) => setError(e.message),
  });

  const doExportSite = useMutation({
    mutationFn: exportSite,
    onSuccess: (r) => setNotice(`Site exported to ${r.index}.`),
    onError: (e: Error) => setError(e.message),
  });

  const get = (key: string) => String(settings.data?.[key] ?? "");

  return (
    <div className="flex max-w-2xl flex-col gap-5">
      <h1 className="text-lg font-semibold">Settings</h1>
      {error && <p className="text-sm text-red-600">{error}</p>}
      {notice && <p className="text-sm text-green-700 dark:text-green-400">{notice}</p>}
      {settings.isLoading && <Spinner />}

      <section className="flex flex-col gap-3 rounded-2xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900">
        <h2 className="text-sm font-semibold">Archive backend</h2>
        <Field label="Backend">
          <select
            value={get("archive_backend") || "auto"}
            onChange={(e) => { setError(""); setNotice(""); save.mutate({ key: "archive_backend", value: e.target.value }); }}
            className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
          >
            {BACKENDS.map((b) => (
              <option key={b} value={b}>
                {b}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Browser path (for browser backend)">
          <SettingInput settingKey="browser_path" value={get("browser_path")} onSave={(v) => save.mutate({ key: "browser_path", value: v })} />
        </Field>
      </section>

      <section className="flex flex-col gap-3 rounded-2xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900">
        <h2 className="text-sm font-semibold">Maintenance</h2>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={prune} onChange={(e) => setPrune(e.target.checked)} />
          Prune entries with missing files
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={compactIds} disabled={!prune} onChange={(e) => setCompactIds(e.target.checked)} />
          Close short-id gaps (renumbers to 1..N)
        </label>
        <div>
          <Button variant="outline" onClick={() => { setError(""); setNotice(""); reindex.mutate(); }} disabled={reindex.isPending}>
            <RefreshCw className="h-4 w-4" /> {reindex.isPending ? "Reindexing..." : "Run reindex"}
          </Button>
        </div>
      </section>

      <section className="flex flex-col gap-3 rounded-2xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900">
        <h2 className="text-sm font-semibold">Import / export</h2>
        <div>
          <p className="mb-1 text-xs font-medium uppercase tracking-wide text-neutral-500">Netscape bookmark file</p>
          <textarea
            value={importText}
            onChange={(e) => setImportText(e.target.value)}
            placeholder="Paste bookmark file contents..."
            rows={3}
            className="w-full rounded-lg border border-neutral-300 px-3 py-1.5 font-mono text-xs dark:border-neutral-700 dark:bg-neutral-900"
          />
          <div className="mt-2">
            <Button
              variant="outline"
              onClick={() => { setError(""); setNotice(""); doImport.mutate(); }}
              disabled={doImport.isPending || !importText.trim()}
            >
              <Upload className="h-4 w-4" /> {doImport.isPending ? "Importing..." : "Import"}
            </Button>
          </div>
        </div>
        <div className="flex gap-2">
          <a href="/api/v2/library/export-bookmarks" download>
            <Button variant="outline">
              <Download className="h-4 w-4" /> Bookmarks file
            </Button>
          </a>
          <Button
            variant="outline"
            onClick={() => { setError(""); setNotice(""); doExportSite.mutate(); }}
            disabled={doExportSite.isPending}
          >
            <Download className="h-4 w-4" /> Static site
          </Button>
        </div>
      </section>
    </div>
  );
}

function SettingInput({ settingKey, value, onSave }: { settingKey: string; value: string; onSave: (v: string) => void }) {
  const [text, setText] = useState(value);
  const [last, setLast] = useState(value);
  if (value !== last) {
    setLast(value);
    setText(value);
  }
  return (
    <div className="flex gap-2">
      <Input value={text} placeholder={`path or command (${settingKey})`} onChange={(e) => setText(e.target.value)} />
      <Button variant="outline" onClick={() => onSave(text)}>
        Save
      </Button>
    </div>
  );
}

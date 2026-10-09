import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, RefreshCw, Upload } from "lucide-react";
import { backupLibraryContent, backupUrl, deleteProfile, exportBookmarksContent, exportBookmarksUrl, exportSite, fetchProfiles, fetchSettings, getRemoteBase, importLibrary, isTauri, restoreBackup, runReindex, setRemote, setSetting, switchProfile, syncCommit, useRemote } from "../tauri";
import { Button, Field, Input, Spinner } from "../components/ui";

const BACKENDS = ["auto", "builtin", "browser", "single-file", "monolith"];

export function SettingsPage() {
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [prune, setPrune] = useState(false);
  const [compactIds, setCompactIds] = useState(false);
  const [importText, setImportText] = useState("");
  const [importArchive, setImportArchive] = useState(false);
  const [remoteUrl, setRemoteUrl] = useState(getRemoteBase());
  const [remoteToken, setRemoteToken] = useState("");
  const [remoteOn, setRemoteOn] = useState(useRemote());
  const [newProfile, setNewProfile] = useState("");
  const [armedDelete, setArmedDelete] = useState("");
  const [armedRestore, setArmedRestore] = useState(false);
  const [restoreFile, setRestoreFile] = useState<File | null>(null);
  const [syncPush, setSyncPush] = useState(false);
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
    mutationFn: () => importLibrary(importText, false, importArchive),
    onSuccess: (r) => {
      const warn = r.warnings?.length ? `, ${r.warnings.length} warning(s)` : "";
      setNotice(`Imported ${r.added}, skipped ${r.skipped_dup} duplicates${warn}.`);
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

  const doRestore = useMutation({
    mutationFn: async (confirm: boolean) => {
      if (!restoreFile) throw new Error("choose a backup file first");
      const buf = await restoreFile.arrayBuffer();
      const bytes = new Uint8Array(buf);
      let bin = "";
      for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
      return restoreBackup(btoa(bin), confirm);
    },
    onSuccess: (r) => {
      if (r.confirm_required) {
        setError(r.hint ?? "Restore needs confirmation.");
        return;
      }
      setNotice(`Restored backup from ${r.restored_from ?? "unknown"}. Reindexed ${r.indexed ?? 0} entries.`);
      setRestoreFile(null);
      setArmedRestore(false);
      qc.invalidateQueries();
    },
    onError: (e: Error) => { setError(e.message); setArmedRestore(false); },
  });

  const profiles = useQuery({ queryKey: ["profiles", remoteOn], queryFn: fetchProfiles });

  const doSwitch = useMutation({
    mutationFn: (name: string) => switchProfile(name),
    onSuccess: (r) => {
      setNotice(`Switched to profile ${r.active}.`);
      qc.invalidateQueries();
    },
    onError: (e: Error) => setError(e.message),
  });

  const doDelete = useMutation({
    mutationFn: (name: string) => deleteProfile(name),
    onSuccess: (r) => {
      setNotice(`Deleted profile ${r.name} (files left on disk).`);
      setArmedDelete("");
      qc.invalidateQueries();
    },
    onError: (e: Error) => { setError(e.message); setArmedDelete(""); },
  });

  const doSnapshot = useMutation({
    mutationFn: () => syncCommit(syncPush),
    onSuccess: (r) => {
      if (r.error) setError(r.error);
      else setNotice(r.output);
    },
    onError: (e: Error) => setError(e.message),
  });

  const get = (key: string) => String(settings.data?.[key] ?? "");
  const maintenance = settings.data?.maintenance_status as unknown as
    | { bookmarks: number; oplog_entries: number; quarantine: number; short_id_gaps: number }
    | undefined;

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
        <Field label="Single-file command">
          <SettingInput settingKey="singlefile_cmd" value={get("singlefile_cmd")} onSave={(v) => save.mutate({ key: "singlefile_cmd", value: v })} />
        </Field>
        <Field label="Monolith command">
          <SettingInput settingKey="monolith_cmd" value={get("monolith_cmd")} onSave={(v) => save.mutate({ key: "monolith_cmd", value: v })} />
        </Field>
      </section>

      <section className="flex flex-col gap-3 rounded-2xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900">
        <h2 className="text-sm font-semibold">Tools</h2>
        <Field label="Browser command (open in browser)">
          <SettingInput settingKey="browser_cmd" value={get("browser_cmd")} onSave={(v) => save.mutate({ key: "browser_cmd", value: v })} />
        </Field>
        <Field label="Editor command">
          <SettingInput settingKey="editor_cmd" value={get("editor_cmd")} onSave={(v) => save.mutate({ key: "editor_cmd", value: v })} />
        </Field>
        <Field label="Device id">
          <SettingInput settingKey="device_id" value={get("device_id")} onSave={(v) => save.mutate({ key: "device_id", value: v })} />
        </Field>
      </section>

      <section className="flex flex-col gap-3 rounded-2xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900">
        <h2 className="text-sm font-semibold">Library paths</h2>
        <p className="text-xs text-neutral-500">
          Saving a path switches to it immediately. Move your files first, then run reindex so orphans are adopted.
        </p>
        <Field label="Base directory (switches library immediately)">
          <SettingInput settingKey="base_dir" value={get("base_dir")} onSave={(v) => { setError(""); setNotice(""); save.mutate({ key: "base_dir", value: v }); qc.invalidateQueries(); }} />
        </Field>
        <Field label="HTML directory override (unset means default)">
          <SettingInput settingKey="html_dir" value={get("html_dir")} onSave={(v) => save.mutate({ key: "html_dir", value: v })} />
        </Field>
        <Field label="Markdown directory override (unset means default)">
          <SettingInput settingKey="markdown_dir" value={get("markdown_dir")} onSave={(v) => save.mutate({ key: "markdown_dir", value: v })} />
        </Field>
        <Field label="Archive directory override (unset means default)">
          <SettingInput settingKey="archive_dir" value={get("archive_dir")} onSave={(v) => save.mutate({ key: "archive_dir", value: v })} />
        </Field>
        <Field label="Attachment directory override (unset means default)">
          <SettingInput settingKey="attachment_dir" value={get("attachment_dir")} onSave={(v) => save.mutate({ key: "attachment_dir", value: v })} />
        </Field>
      </section>

      <section className="flex flex-col gap-3 rounded-2xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900">
        <h2 className="text-sm font-semibold">Remote server</h2>
        <p className="text-xs text-neutral-500">
          Talk to a liber-serve on your network instead of the local library. Empty URL means local mode.
        </p>
        <Field label="Server URL (e.g. http://192.168.1.10:8080)">
          <Input value={remoteUrl} placeholder="http://host:port" onChange={(e) => setRemoteUrl(e.target.value)} />
        </Field>
        <Field label="Auth token (when the server requires one)">
          <Input type="password" value={remoteToken} placeholder="token from the server" onChange={(e) => setRemoteToken(e.target.value)} />
        </Field>
        <div>
          <Button
            variant="outline"
            onClick={() => {
              setError("");
              setNotice("");
              setRemote(remoteUrl, remoteToken);
              setRemoteOn(useRemote());
              setRemoteToken("");
              qc.invalidateQueries();
              setNotice(useRemote() ? "Remote mode on." : "Local mode.");
            }}
          >
            Save remote settings
          </Button>
        </div>
        {remoteOn && (
          <p className="text-xs text-neutral-500">Remote mode is on. Clear the URL and save to go back local.</p>
        )}
      </section>

      <section className="flex flex-col gap-3 rounded-2xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900">
        <h2 className="text-sm font-semibold">Profiles</h2>
        <p className="text-xs text-neutral-500">
          Separate libraries under one config. Switching takes effect immediately; deleting only untracks, files stay on disk.
        </p>
        <div className="flex flex-col gap-1">
          {(profiles.data?.profiles ?? []).map((p) => (
            <div key={p.name} className="flex items-center gap-2 text-sm">
              <span className="w-4 text-center">{p.active ? "*" : ""}</span>
              <span className="flex-1 font-mono">{p.name}</span>
              {!p.active && (
                <Button variant="outline" onClick={() => { setError(""); setNotice(""); doSwitch.mutate(p.name); }}>
                  Switch
                </Button>
              )}
              {!p.active && !p.default && (
                <Button
                  variant="outline"
                  onClick={() => {
                    setError("");
                    setNotice("");
                    if (armedDelete === p.name) doDelete.mutate(p.name);
                    else setArmedDelete(p.name);
                  }}
                >
                  {armedDelete === p.name ? "Sure?" : "Delete"}
                </Button>
              )}
            </div>
          ))}
        </div>
        <div className="flex gap-2">
          <Input value={newProfile} placeholder="New profile name" onChange={(e) => setNewProfile(e.target.value)} />
          <Button
            variant="outline"
            disabled={!newProfile.trim() || doSwitch.isPending}
            onClick={() => { setError(""); setNotice(""); doSwitch.mutate(newProfile.trim()); setNewProfile(""); }}
          >
            Create
          </Button>
        </div>
      </section>

      <section className="flex flex-col gap-3 rounded-2xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900">
        <h2 className="text-sm font-semibold">Sync snapshot</h2>
        <p className="text-xs text-neutral-500">
          Commits the library directory with git. In remote mode this commits the server library.
        </p>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={syncPush} onChange={(e) => setSyncPush(e.target.checked)} />
          Push after committing
        </label>
        <div>
          <Button variant="outline" onClick={() => { setError(""); setNotice(""); doSnapshot.mutate(); }} disabled={doSnapshot.isPending}>
            {doSnapshot.isPending ? "Snapshotting..." : "Snapshot now"}
          </Button>
        </div>
      </section>

      <section className="flex flex-col gap-3 rounded-2xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900">
        <h2 className="text-sm font-semibold">Maintenance</h2>
        {maintenance != null && (
          <p className="text-xs text-neutral-500">
            {maintenance.bookmarks} bookmarks, {maintenance.oplog_entries} oplog entries,{" "}
            {maintenance.quarantine} quarantined, {maintenance.short_id_gaps} short-id gaps.
          </p>
        )}
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
          <label className="mt-2 flex items-center gap-2 text-sm text-neutral-600 dark:text-neutral-400">
            <input type="checkbox" checked={importArchive} onChange={(e) => setImportArchive(e.target.checked)} />
            Archive pages at import
          </label>
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
          {isTauri() ? (
            <Button
              variant="outline"
              onClick={async () => {
                setError("");
                setNotice("");
                try {
                  const { content } = await exportBookmarksContent();
                  const blob = new Blob([content], { type: "text/html;charset=utf-8" });
                  const href = URL.createObjectURL(blob);
                  const a = document.createElement("a");
                  a.href = href;
                  a.download = "liber-bookmarks.html";
                  a.click();
                  setTimeout(() => URL.revokeObjectURL(href), 5000);
                } catch (e) {
                  setError(e instanceof Error ? e.message : "export failed");
                }
              }}
            >
              <Download className="h-4 w-4" /> Bookmarks file
            </Button>
          ) : (
            <a href={exportBookmarksUrl()} download>
              <Button variant="outline">
                <Download className="h-4 w-4" /> Bookmarks file
              </Button>
            </a>
          )}
          <Button
            variant="outline"
            onClick={() => { setError(""); setNotice(""); doExportSite.mutate(); }}
            disabled={doExportSite.isPending}
          >
            <Download className="h-4 w-4" /> Static site
          </Button>
        </div>
        <div>
          <p className="mb-1 text-xs font-medium uppercase tracking-wide text-neutral-500">Full backup</p>
          <p className="mb-2 text-xs text-neutral-500">
            Downloads the whole library (database plus content files) as one file. Restoring replaces the library and reindexes it.
          </p>
          <div className="flex flex-wrap gap-2">
            {isTauri() ? (
              <Button
                variant="outline"
                onClick={async () => {
                  setError("");
                  setNotice("");
                  try {
                    const { filename, content } = await backupLibraryContent();
                    const bin = atob(content);
                    const bytes = new Uint8Array(bin.length);
                    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
                    const blob = new Blob([bytes], { type: "application/gzip" });
                    const href = URL.createObjectURL(blob);
                    const a = document.createElement("a");
                    a.href = href;
                    a.download = filename;
                    a.click();
                    setTimeout(() => URL.revokeObjectURL(href), 5000);
                  } catch (e) {
                    setError(e instanceof Error ? e.message : "backup failed");
                  }
                }}
              >
                <Download className="h-4 w-4" /> Download backup
              </Button>
            ) : (
              <a href={backupUrl()} download>
                <Button variant="outline">
                  <Download className="h-4 w-4" /> Download backup
                </Button>
              </a>
            )}
          </div>
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <input
              type="file"
              accept=".tar.gz,.tgz,application/gzip"
              onChange={(e) => { setRestoreFile(e.target.files?.[0] ?? null); setArmedRestore(false); }}
              className="text-xs"
            />
            <Button
              variant="outline"
              onClick={() => {
                setError("");
                setNotice("");
                if (armedRestore) doRestore.mutate(true);
                else setArmedRestore(true);
              }}
              disabled={doRestore.isPending || !restoreFile}
            >
              <Upload className="h-4 w-4" /> {armedRestore ? "Sure? Replace library" : "Restore backup"}
            </Button>
          </div>
          {restoreFile && (
            <p className="mt-1 text-xs text-neutral-500">
              {(restoreFile.size / 1048576).toFixed(1)} MiB selected. Uploads over 256 MiB are refused by the server.
            </p>
          )}
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

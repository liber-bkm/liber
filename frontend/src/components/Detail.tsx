import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Pencil, Trash2, Upload, X } from "lucide-react";
import { displayId, domainOf, getArchiveView, setArchiveView, shortUuid, ApiError, type Bookmark } from "../api";
import { addBookmark, archiveUrl, attachmentUrl, createArchive, deleteArchive, deleteAttachment, deleteBookmark, deleteNotes, downloadAttachment, fetchArchive, fetchBookmark, fetchFolders, fetchNotes, fetchTags, isTauri, openArchiveExternal, openAttachmentExternal, openBookmark, openExternal, saveNotes, updateBookmark, uploadAttachment, useRemote } from "../tauri";
import { Badge, Button, Field, Input, Modal, Spinner } from "./ui";

function useTaxonomyNames() {
  const tags = useQuery({ queryKey: ["tags"], queryFn: fetchTags, staleTime: 60_000 });
  const folders = useQuery({ queryKey: ["folders"], queryFn: fetchFolders, staleTime: 60_000 });
  return {
    tagNames: (tags.data?.tags ?? []).map((t) => t.name),
    folderNames: (folders.data?.folders ?? []).map((f) => f.name),
  };
}

function lastToken(value: string): string {
  const parts = value.split(/[\s,]+/);
  return parts[parts.length - 1] ?? "";
}

function appendToken(value: string, token: string): string {
  const sep = value === "" || /[\s,]$/.test(value) ? "" : " ";
  return `${value}${sep}${token} `;
}

function TagSuggest({ value, names, onPick }: { value: string; names: string[]; onPick: (next: string) => void }) {
  const token = lastToken(value).toLowerCase();
  if (!token) return null;
  const have = new Set(value.split(/[\s,]+/).filter(Boolean).map((t) => t.toLowerCase()));
  const matches = names.filter((n) => n.toLowerCase().startsWith(token) && !have.has(n.toLowerCase())).slice(0, 6);
  if (matches.length === 0) return null;
  return (
    <div className="mt-1 flex flex-wrap gap-1">
      {matches.map((m) => (
        <button
          key={m}
          type="button"
          onClick={() => onPick(appendToken(value, m))}
          className="rounded-full bg-neutral-100 px-2 py-0.5 text-xs text-neutral-600 hover:bg-accent-50 hover:text-accent-700 dark:bg-neutral-800 dark:text-neutral-300"
        >
          {m}
        </button>
      ))}
    </div>
  );
}

type Tab = "details" | "notes" | "archive" | "attachments";

export function DetailDrawer({ uuid, onClose, onChanged }: { uuid: string; onClose: () => void; onChanged: () => void }) {
  const [editing, setEditing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [tab, setTab] = useState<Tab>("details");
  const [openError, setOpenError] = useState("");
  const qc = useQueryClient();
  const detail = useQuery({ queryKey: ["bookmark", uuid], queryFn: () => fetchBookmark(uuid) });

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["bookmarks"] });
    qc.invalidateQueries({ queryKey: ["bookmark", uuid] });
    onChanged();
  };

  const del = useMutation({
    mutationFn: () => deleteBookmark(uuid, true),
    onSuccess: () => {
      invalidate();
      onClose();
    },
  });

  const open = useMutation({
    mutationFn: () => openBookmark(uuid),
    onMutate: () => setOpenError(""),
    onSuccess: async (data) => {
      await openExternal(data.url);
      invalidate();
    },
    onError: (e: Error) => setOpenError(e.message),
  });

  return (
    <div className="fixed inset-0 z-40" onClick={onClose}>
      <div className="absolute inset-0 bg-black/30" />
      <div
        className="absolute right-0 top-0 flex h-full w-full max-w-md flex-col gap-4 overflow-y-auto bg-white px-5 pb-5 pt-[max(1.25rem,env(safe-area-inset-top))] shadow-xl dark:bg-neutral-900"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between">
          <p className="font-mono text-xs text-neutral-400" title={uuid}>
            [{detail.data ? displayId(detail.data) : shortUuid(uuid)}] {uuid}
          </p>
          <button onClick={onClose} className="rounded-lg p-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800">
            <X className="h-4 w-4" />
          </button>
        </div>
        {detail.isLoading && <Spinner />}
        {detail.data && (
          <div className="flex gap-1 border-b border-neutral-200 pb-2 dark:border-neutral-800">
            {(["details", "notes", "archive", "attachments"] as Tab[]).map((t) => (
              <button
                key={t}
                onClick={() => setTab(t)}
                className={`rounded-lg px-2.5 py-1 text-sm capitalize ${tab === t ? "bg-accent-50 font-medium text-accent-700 dark:bg-accent-700/20 dark:text-accent-100" : "text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"}`}
              >
                {t}
              </button>
            ))}
          </div>
        )}
        {detail.data && tab === "details" && !editing && <View bookmark={detail.data} onChanged={invalidate} />}
        {detail.data && tab === "details" && editing && (
          <EditForm
            bookmark={detail.data}
            onDone={() => {
              setEditing(false);
              invalidate();
            }}
          />
        )}
        {detail.data && tab === "notes" && <NotesTab uuid={uuid} hasNotes={detail.data.has_markdown} />}
        {detail.data && tab === "archive" && <ArchiveTab bookmark={detail.data} />}
        {detail.data && tab === "attachments" && <AttachmentsTab bookmark={detail.data} onChanged={invalidate} />}
        {openError && <p className="text-sm text-red-600">{openError}</p>}
        {detail.data && !editing && (
          <div className="flex flex-wrap gap-2">
            <Button onClick={() => open.mutate()}>
              <ExternalLink className="h-4 w-4" /> Open
            </Button>
            <Button variant="outline" onClick={() => setEditing(true)}>
              <Pencil className="h-4 w-4" /> Edit
            </Button>
            {!confirming ? (
              <Button variant="ghost" onClick={() => setConfirming(true)}>
                <Trash2 className="h-4 w-4" /> Delete
              </Button>
            ) : (
              <>
                <Button variant="danger" onClick={() => del.mutate()}>
                  Confirm delete
                </Button>
                <Button variant="ghost" onClick={() => setConfirming(false)}>
                  Keep
                </Button>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function View({ bookmark: b, onChanged }: { bookmark: Bookmark; onChanged: () => void }) {
  const qc = useQueryClient();
  const [uploadError, setUploadError] = useState("");
  const [linkError, setLinkError] = useState("");
  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-lg font-semibold leading-snug">{b.title}</h2>
      {isTauri() ? (
        <>
          <button
            onClick={async () => {
              setLinkError("");
              try {
                await openExternal(b.url);
              } catch (e) {
                setLinkError(e instanceof Error ? e.message : "could not open link");
              }
            }}
            className="truncate text-left text-sm text-accent-600 hover:underline"
          >
            {domainOf(b.url)}
          </button>
          {linkError && <p className="text-xs text-red-600">{linkError}</p>}
        </>
      ) : (
        <a href={b.url} target="_blank" rel="noopener" className="truncate text-sm text-accent-600 hover:underline">
          {domainOf(b.url)}
        </a>
      )}
      {b.description && <p className="text-sm text-neutral-600 dark:text-neutral-300">{b.description}</p>}
      <div className="flex flex-wrap gap-1.5">
        {b.folder && <Badge tone="accent">{b.folder}</Badge>}
        {(b.tags ?? []).map((t) => (
          <Badge key={t}>#{t}</Badge>
        ))}
        {b.has_archive && <Badge tone="green">archived</Badge>}
        {b.has_markdown && <Badge>notes</Badge>}
        {b.check_status && b.check_status !== "ok" && <Badge tone={b.check_status === "dead" ? "red" : "amber"}>{b.check_status}</Badge>}
      </div>
      <div>
        <div className="mb-1 flex items-center justify-between">
          <p className="text-xs font-medium uppercase tracking-wide text-neutral-400">Attachments</p>
          <label className="cursor-pointer text-xs text-accent-600 hover:underline">
            Upload
            <input
              type="file"
              className="hidden"
              onChange={async (e) => {
                const file = e.target.files?.[0];
                if (!file) return;
                setUploadError("");
                try {
                  const dataUrl = await new Promise<string>((resolve, reject) => {
                    const reader = new FileReader();
                    reader.onload = () => resolve(String(reader.result));
                    reader.onerror = reject;
                    reader.readAsDataURL(file);
                  });
                  await uploadAttachment(b.uuid, file.name, dataUrl);
                  qc.invalidateQueries({ queryKey: ["bookmark", b.uuid] });
                  qc.invalidateQueries({ queryKey: ["bookmarks"] });
                  onChanged();
                } catch (err) {
                  setUploadError(err instanceof Error ? err.message : "upload failed");
                }
                e.target.value = "";
              }}
            />
          </label>
        </div>
        {uploadError && <p className="mb-1 text-xs text-red-600">{uploadError}</p>}
        {(b.attachments ?? []).length === 0 && <p className="text-sm text-neutral-400">None yet.</p>}
        {(b.attachments ?? []).map((a) => (
          <AttachmentLink key={a.name} uuid={b.uuid} name={a.name} />
        ))}
      </div>
      <p className="text-xs text-neutral-400">
        Saved {new Date(b.created_at).toLocaleDateString()}
        {b.open_count ? ` · opened ${b.open_count} times` : ""}
      </p>
    </div>
  );
}

function AttachmentLink({ uuid, name }: { uuid: string; name: string }) {
  const [error, setError] = useState("");
  if (!isTauri() || useRemote()) {
    return (
      <a
        href={attachmentUrl(uuid, name)}
        className="block truncate text-sm text-accent-600 hover:underline"
      >
        {name}
      </a>
    );
  }
  return (
    <>
      <button
        onClick={async () => {
          setError("");
          try {
            await openAttachmentExternal(uuid, name);
          } catch (e) {
            setError(e instanceof Error ? e.message : "could not open attachment");
          }
        }}
        className="block truncate text-left text-sm text-accent-600 hover:underline"
      >
        {name}
      </button>
      {error && <p className="text-xs text-red-600">{error}</p>}
    </>
  );
}

function AttachmentManager({ uuid, attachments, onChanged }: { uuid: string; attachments: { name: string }[]; onChanged: () => void }) {
  const qc = useQueryClient();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  async function uploadFiles(files: File[]) {
    setError("");
    setBusy(true);
    try {
      for (const f of files) {
        const bytes = new Uint8Array(await f.arrayBuffer());
        let bin = "";
        const CHUNK = 8192;
        for (let i = 0; i < bytes.length; i += CHUNK) {
          bin += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
        }
        await uploadAttachment(uuid, f.name, `data:application/octet-stream;base64,${btoa(bin)}`);
      }
      await qc.invalidateQueries({ queryKey: ["bookmark", uuid] });
      qc.invalidateQueries({ queryKey: ["bookmarks"] });
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "upload failed");
    } finally {
      setBusy(false);
    }
  }

  async function remove(name: string) {
    setError("");
    setBusy(true);
    try {
      await deleteAttachment(uuid, name);
      await qc.invalidateQueries({ queryKey: ["bookmark", uuid] });
      qc.invalidateQueries({ queryKey: ["bookmarks"] });
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "remove failed");
    } finally {
      setBusy(false);
    }
  }

  async function saveAs(name: string) {
    setError("");
    setBusy(true);
    try {
      const data = await downloadAttachment(uuid, name);
      const bin = atob(data.content);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      const blob = new Blob([bytes], { type: data.mime });
      const href = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = href;
      a.download = data.name;
      a.click();
      setTimeout(() => URL.revokeObjectURL(href), 5000);
    } catch (e) {
      setError(e instanceof Error ? e.message : "download failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-2">
      {error && <p className="text-sm text-red-600">{error}</p>}
      <label className="cursor-pointer text-sm text-accent-600 hover:underline">
        {busy ? "Working..." : "Upload files"}
        <input
          type="file"
          multiple
          className="hidden"
          disabled={busy}
          onChange={(e) => {
            const files = Array.from(e.target.files ?? []);
            e.target.value = "";
            if (files.length > 0) void uploadFiles(files);
          }}
        />
      </label>
      {attachments.length === 0 && <p className="text-sm text-neutral-400">No attachments yet.</p>}
      {attachments.map((a) => (
        <div key={a.name} className="flex items-center gap-2">
          <div className="min-w-0 flex-1">
            <AttachmentLink uuid={uuid} name={a.name} />
          </div>
          {isTauri() && !useRemote() && (
            <button
              onClick={() => void saveAs(a.name)}
              disabled={busy}
              className="shrink-0 rounded-lg px-2 py-0.5 text-xs text-neutral-500 hover:bg-neutral-100 disabled:opacity-50 dark:hover:bg-neutral-800"
            >
              Save
            </button>
          )}
          <button
            onClick={() => void remove(a.name)}
            disabled={busy}
            className="shrink-0 rounded-lg px-2 py-0.5 text-xs text-red-600 hover:bg-red-50 disabled:opacity-50 dark:hover:bg-red-900/20"
          >
            Remove
          </button>
        </div>
      ))}
    </div>
  );
}

function AttachmentsTab({ bookmark: b, onChanged }: { bookmark: Bookmark; onChanged: () => void }) {
  return <AttachmentManager uuid={b.uuid} attachments={b.attachments ?? []} onChanged={onChanged} />;
}

function NotesTab({ uuid, hasNotes }: { uuid: string; hasNotes: boolean }) {
  const qc = useQueryClient();
  const notes = useQuery({ queryKey: ["notes", uuid], queryFn: () => fetchNotes(uuid) });
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState("");
  const shown = text ?? notes.data?.body ?? "";
  const save = useMutation({
    mutationFn: () => saveNotes(uuid, shown),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["notes", uuid] });
      qc.invalidateQueries({ queryKey: ["bookmark", uuid] });
    },
    onError: (e: Error) => setError(e.message),
  });

  return (
    <div className="flex flex-col gap-2">
      {error && <p className="text-sm text-red-600">{error}</p>}
      {notes.isLoading ? (
        <Spinner />
      ) : (
        <>
          {!hasNotes && !notes.data?.body && text === null && (
            <p className="text-sm text-neutral-400">No notes yet. Write the first ones below.</p>
          )}
          <textarea
            value={shown}
            onChange={(e) => setText(e.target.value)}
            rows={14}
            placeholder="Personal notes in markdown..."
            className="w-full rounded-lg border border-neutral-300 px-3 py-2 font-mono text-sm outline-none focus:border-accent-500 dark:border-neutral-700 dark:bg-neutral-900"
          />
          <div>
            <Button onClick={() => save.mutate()} disabled={save.isPending}>
              {save.isPending ? "Saving..." : "Save notes"}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}

function ArchiveTab({ bookmark: b }: { bookmark: Bookmark }) {
  const [view, setView] = useState(getArchiveView());
  const [openError, setOpenError] = useState("");
  const remote = useQuery({
    queryKey: ["archive", b.uuid],
    queryFn: () => fetchArchive(b.uuid),
    enabled: isTauri() && b.has_archive,
  });
  if (!b.has_archive) {
    return <p className="text-sm text-neutral-400">No archived copy. Add one from the edit form below.</p>;
  }
  if (isTauri()) {
    if (remote.isLoading) return <Spinner />;
    if (remote.error) {
      return (
        <p className="text-sm text-red-600">
          {remote.error instanceof Error ? remote.error.message : "archive failed to load"}
        </p>
      );
    }
    return (
      <div className="flex flex-col gap-2">
        <div>
          <Button
            variant="outline"
            onClick={async () => {
              setOpenError("");
              try {
                await openArchiveExternal(b.uuid);
              } catch (e) {
                setOpenError(e instanceof Error ? e.message : "could not open archive");
              }
            }}
          >
            Open in browser
          </Button>
        </div>
        {openError && <p className="text-sm text-red-600">{openError}</p>}
        <iframe
          srcDoc={remote.data?.html ?? ""}
          sandbox=""
          title="Archived page"
          className="h-[60vh] w-full rounded-xl border border-neutral-200 bg-white dark:border-neutral-800"
        />
      </div>
    );
  }
  const url = archiveUrl(b.uuid);
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2 text-sm">
        <span className="text-neutral-500">View:</span>
        {(["embed", "tab"] as const).map((v) => (
          <button
            key={v}
            onClick={() => {
              setView(v);
              setArchiveView(v);
            }}
            className={`rounded-lg px-2 py-0.5 ${view === v ? "bg-accent-50 font-medium text-accent-700 dark:bg-accent-700/20 dark:text-accent-100" : "text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"}`}
          >
            {v === "embed" ? "Inline" : "New tab"}
          </button>
        ))}
        <a href={url} target="_blank" rel="noopener" className="ml-auto text-accent-600 hover:underline">
          Open directly
        </a>
      </div>
      {view === "embed" ? (
        <iframe src={url} sandbox="" title="Archived page" className="h-[60vh] w-full rounded-xl border border-neutral-200 bg-white dark:border-neutral-800" />
      ) : (
        <p className="text-sm text-neutral-500">
          Archives open in a new tab with this preference. The server sends a sandbox policy header on top.
        </p>
      )}
    </div>
  );
}

function EditForm({ bookmark: b, onDone }: { bookmark: Bookmark; onDone: () => void }) {
  const [title, setTitle] = useState(b.title);
  const [url, setUrl] = useState(b.url);
  const [description, setDescription] = useState(b.description ?? "");
  const [tags, setTags] = useState((b.tags ?? []).join(" "));
  const [folder, setFolder] = useState(b.folder ?? "");
  const [error, setError] = useState("");
  const { tagNames, folderNames } = useTaxonomyNames();
  const save = useMutation({
    mutationFn: () =>
      updateBookmark(b.uuid, {
        title,
        url,
        description,
        tags: tags.split(/[\s,]+/).filter(Boolean),
        folder,
      }),
    onSuccess: onDone,
    onError: (e: Error) => setError(e.message),
  });
  return (
    <div className="flex flex-col gap-3">
      {error && <p className="text-sm text-red-600">{error}</p>}
      <Field label="Title">
        <Input value={title} onChange={(e) => setTitle(e.target.value)} />
      </Field>
      <Field label="URL">
        <Input value={url} onChange={(e) => setUrl(e.target.value)} />
      </Field>
      <Field label="Description">
        <Input value={description} onChange={(e) => setDescription(e.target.value)} />
      </Field>
      <Field label="Tags (space separated)">
        <Input value={tags} onChange={(e) => setTags(e.target.value)} />
        <TagSuggest value={tags} names={tagNames} onPick={setTags} />
      </Field>
      <Field label="Folder">
        <Input value={folder} onChange={(e) => setFolder(e.target.value)} list="liber-folder-edit" />
        <datalist id="liber-folder-edit">
          {folderNames.map((f) => (
            <option key={f} value={f} />
          ))}
        </datalist>
      </Field>
      <ArtifactSection bookmark={b} />
      <div>
        <Button onClick={() => save.mutate()} disabled={save.isPending}>
          {save.isPending ? "Saving..." : "Save"}
        </Button>
      </div>
    </div>
  );
}

const BACKENDS = ["auto", "builtin", "browser", "single-file", "monolith"];

function ArtifactSection({ bookmark: b }: { bookmark: Bookmark }) {
  const qc = useQueryClient();
  const [error, setError] = useState("");
  const [backend, setBackend] = useState("auto");
  const [busy, setBusy] = useState(false);

  async function refresh() {
    await qc.invalidateQueries({ queryKey: ["bookmark", b.uuid] });
    qc.invalidateQueries({ queryKey: ["bookmarks"] });
  }

  async function run(fn: () => Promise<unknown>) {
    setError("");
    setBusy(true);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-2 rounded-xl border border-neutral-200 p-3 dark:border-neutral-800">
      <p className="text-xs font-medium uppercase tracking-wide text-neutral-500">Artifacts</p>
      {error && <p className="text-sm text-red-600">{error}</p>}
      <div className="flex flex-wrap items-center gap-2 text-sm">
        <span className="w-20 text-neutral-500">Notes</span>
        {b.has_markdown ? (
          <Button variant="outline" onClick={() => run(() => deleteNotes(b.uuid))} disabled={busy}>
            Remove notes
          </Button>
        ) : (
          <Button variant="outline" onClick={() => run(() => saveNotes(b.uuid, ""))} disabled={busy}>
            Start notes
          </Button>
        )}
      </div>
      <div className="flex flex-wrap items-center gap-2 text-sm">
        <span className="w-20 text-neutral-500">Archive</span>
        {b.has_archive ? (
          <>
            <Button variant="outline" onClick={() => run(() => createArchive(b.uuid, backend))} disabled={busy}>
              Re-archive
            </Button>
            <Button variant="ghost" onClick={() => run(() => deleteArchive(b.uuid))} disabled={busy}>
              Remove
            </Button>
          </>
        ) : (
          <>
            <select
              value={backend}
              onChange={(e) => setBackend(e.target.value)}
              className="rounded-lg border border-neutral-300 bg-white px-2 py-1 text-sm dark:border-neutral-700 dark:bg-neutral-900"
            >
              {BACKENDS.map((v) => (
                <option key={v} value={v}>
                  {v}
                </option>
              ))}
            </select>
            <Button variant="outline" onClick={() => run(() => createArchive(b.uuid, backend))} disabled={busy}>
              Archive now
            </Button>
          </>
        )}
      </div>
      <div className="flex flex-col gap-1 text-sm">
        <span className="w-20 text-neutral-500">Files</span>
        <AttachmentManager
          uuid={b.uuid}
          attachments={b.attachments ?? []}
          onChanged={() => void refresh()}
        />
      </div>
    </div>
  );
}

export function parseAddUrls(text: string): string[] {
  return text.split(/[\s,;]+/).map((s) => s.trim()).filter(Boolean);
}

type MultiResult = { url: string; status: "added" | "duplicate" | "failed"; detail: string };

export function AddDialog({ onClose, onAdded, initialUrl = "", initialTitle = "", initialTags = "", initialFolder = "" }: { onClose: () => void; onAdded: (b: Bookmark) => void; initialUrl?: string; initialTitle?: string; initialTags?: string; initialFolder?: string }) {
  const [url, setUrl] = useState(initialUrl);
  const [title, setTitle] = useState(initialTitle);
  const [tags, setTags] = useState(initialTags);
  const [folder, setFolder] = useState(initialFolder);
  const [markdown, setMarkdown] = useState(false);
  const [archive, setArchive] = useState(false);
  const [files, setFiles] = useState<File[]>([]);
  const [error, setError] = useState("");
  const [existing, setExisting] = useState<Bookmark | null>(null);
  const [saving, setSaving] = useState(false);
  const [results, setResults] = useState<MultiResult[] | null>(null);
  const { tagNames, folderNames } = useTaxonomyNames();
  const qc = useQueryClient();
  const urls = parseAddUrls(url);
  const isMulti = urls.length > 1;

  async function fileToAttachment(f: File): Promise<{ name: string; content: string }> {
    const bytes = new Uint8Array(await f.arrayBuffer());
    let bin = "";
    const CHUNK = 8192;
    for (let i = 0; i < bytes.length; i += CHUNK) {
      bin += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
    }
    return { name: f.name, content: btoa(bin) };
  }

  async function save(confirmed: boolean) {
    setSaving(true);
    setError("");
    try {
      const attachments = await Promise.all(files.map(fileToAttachment));
      const b = await addBookmark({
        url,
        title: title || undefined,
        tags: tags.split(/[\s,]+/).filter(Boolean),
        folder: folder || undefined,
        markdown,
        archive,
        confirm_dup: confirmed,
        attachments: attachments.length ? attachments : undefined,
      });
      qc.invalidateQueries({ queryKey: ["bookmarks"] });
      qc.invalidateQueries({ queryKey: ["tags"] });
      qc.invalidateQueries({ queryKey: ["folders"] });
      onAdded(b);
    } catch (e) {
      if (e instanceof ApiError && e.status === 409) {
        const body = e.body as { existing?: Bookmark };
        if (body.existing) {
          setExisting(body.existing);
          return;
        }
      }
      setError(e instanceof Error ? e.message : "add failed");
    } finally {
      setSaving(false);
    }
  }

  async function saveMulti() {
    setSaving(true);
    setError("");
    const out: MultiResult[] = [];
    const sharedTags = tags.split(/[\s,]+/).filter(Boolean);
    for (const u of urls) {
      try {
        await addBookmark({
          url: u,
          tags: sharedTags,
          folder: folder || undefined,
          archive,
        });
        out.push({ url: u, status: "added", detail: "saved" });
      } catch (e) {
        if (e instanceof ApiError && e.status === 409) {
          const body = e.body as { existing?: Bookmark };
          out.push({ url: u, status: "duplicate", detail: body.existing ? `already saved as ${body.existing.title}` : "already saved" });
        } else {
          out.push({ url: u, status: "failed", detail: e instanceof Error ? e.message : "add failed" });
        }
      }
      setResults([...out]);
    }
    qc.invalidateQueries({ queryKey: ["bookmarks"] });
    qc.invalidateQueries({ queryKey: ["tags"] });
    qc.invalidateQueries({ queryKey: ["folders"] });
    setSaving(false);
  }

  return (
    <Modal onClose={onClose}>
      <h2 className="mb-3 text-base font-semibold">Add bookmark</h2>
      {error && <p className="mb-2 text-sm text-red-600">{error}</p>}
      {results ? (
        <div className="flex flex-col gap-2">
          {results.map((r) => (
            <div key={r.url} className="flex items-center gap-2 text-sm">
              <Badge tone={r.status === "added" ? "green" : r.status === "duplicate" ? "amber" : "red"}>{r.status}</Badge>
              <span className="min-w-0 flex-1 truncate font-mono text-xs">{r.url}</span>
              <span className="max-w-48 truncate text-xs text-neutral-500">{r.detail}</span>
            </div>
          ))}
          {saving && <p className="text-xs text-neutral-500">Working through the list...</p>}
          {!saving && (
            <div>
              <Button variant="outline" onClick={onClose}>
                Done
              </Button>
            </div>
          )}
        </div>
      ) : existing ? (
        <div className="flex flex-col gap-3">
          <p className="text-sm text-neutral-600 dark:text-neutral-300">
            Already in your library as <span className="font-medium">{existing.title}</span>
            {existing.folder ? ` in ${existing.folder}` : ""}.
          </p>
          <div className="flex gap-2">
            <Button onClick={() => save(true)} disabled={saving}>
              Add anyway
            </Button>
            <Button variant="outline" onClick={() => existing && onAdded(existing)}>
              Open existing instead
            </Button>
          </div>
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          <Field label={isMulti ? `URLs (one per line, ${urls.length})` : "URL"}>
            <textarea
              value={url}
              placeholder="https://example.com"
              onChange={(e) => setUrl(e.target.value)}
              autoFocus
              rows={isMulti ? 4 : 2}
              className="w-full rounded-lg border border-neutral-300 bg-white px-3 py-1.5 text-sm outline-none focus:border-accent-500 focus:ring-1 focus:ring-accent-500 dark:border-neutral-700 dark:bg-neutral-900"
            />
          </Field>
          {!isMulti && (
            <Field label="Title (optional)">
              <Input value={title} onChange={(e) => setTitle(e.target.value)} />
            </Field>
          )}
          <div className="grid grid-cols-2 gap-3">
            <Field label="Tags">
              <Input value={tags} onChange={(e) => setTags(e.target.value)} />
              <TagSuggest value={tags} names={tagNames} onPick={setTags} />
            </Field>
            <Field label="Folder">
              <Input value={folder} onChange={(e) => setFolder(e.target.value)} list="liber-folder-add" />
              <datalist id="liber-folder-add">
                {folderNames.map((f) => (
                  <option key={f} value={f} />
                ))}
              </datalist>
            </Field>
          </div>
          {!isMulti && (
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={markdown} onChange={(e) => setMarkdown(e.target.checked)} />
              Start with notes
            </label>
          )}
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={archive} onChange={(e) => setArchive(e.target.checked)} />
            Save archived copy
          </label>
          {!isMulti && (
            <div>
              <p className="mb-1 text-xs font-medium uppercase tracking-wide text-neutral-500">Attachments</p>
              <input
                type="file"
                multiple
                onChange={(e) => setFiles(Array.from(e.target.files ?? []))}
                className="text-xs"
              />
              {files.length > 0 && (
                <ul className="mt-1 flex flex-col gap-1">
                  {files.map((f, i) => (
                    <li key={`${f.name}-${i}`} className="flex items-center gap-2 text-xs text-neutral-500">
                      <span className="flex-1 truncate">{f.name}</span>
                      <button
                        type="button"
                        className="text-red-600"
                        onClick={() => setFiles(files.filter((_, j) => j !== i))}
                      >
                        Remove
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
          <div>
            {isMulti ? (
              <Button onClick={() => void saveMulti()} disabled={saving}>
                {saving ? "Saving..." : `Save ${urls.length} bookmarks`}
              </Button>
            ) : (
              <Button onClick={() => save(false)} disabled={saving || !url.trim()}>
                {saving ? "Saving..." : "Save bookmark"}
              </Button>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}

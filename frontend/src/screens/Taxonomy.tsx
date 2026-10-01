import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Pencil, Trash2 } from "lucide-react";
import { deleteFolder, deleteTag, fetchFolders, fetchTags, renameFolder, renameTag } from "../api";
import { Badge, Button, Empty, Input, Modal, Spinner } from "../components/ui";

function refresh(qc: ReturnType<typeof useQueryClient>) {
  qc.invalidateQueries({ queryKey: ["tags"] });
  qc.invalidateQueries({ queryKey: ["folders"] });
  qc.invalidateQueries({ queryKey: ["bookmarks"] });
}

export function TagsPage() {
  const [renaming, setRenaming] = useState<string | null>(null);
  const [next, setNext] = useState("");
  const [deleting, setDeleting] = useState<{ tag: string; count: number } | null>(null);
  const [error, setError] = useState("");
  const qc = useQueryClient();
  const tags = useQuery({ queryKey: ["tags"], queryFn: fetchTags });

  const rename = useMutation({
    mutationFn: (to: string) => renameTag(renaming!, to),
    onSuccess: () => {
      setRenaming(null);
      setNext("");
      refresh(qc);
    },
    onError: (e: Error) => setError(e.message),
  });

  const remove = useMutation({
    mutationFn: () => deleteTag(deleting!.tag, true),
    onSuccess: () => {
      setDeleting(null);
      refresh(qc);
    },
    onError: (e: Error) => setError(e.message),
  });

  return (
    <div className="flex flex-col gap-3">
      <h1 className="text-lg font-semibold">Tags</h1>
      {error && <p className="text-sm text-red-600">{error}</p>}
      {tags.isLoading && <Spinner />}
      {tags.data && tags.data.tags.length === 0 && <Empty title="No tags yet" hint="Tags appear when you add them to bookmarks." />}
      <div className="flex flex-col gap-1">
        {(tags.data?.tags ?? []).map((t) => (
          <div
            key={t.name}
            className="flex items-center gap-2 rounded-xl border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-800 dark:bg-neutral-900"
          >
            <Badge>#{t.name}</Badge>
            <span className="text-xs text-neutral-400">{t.count} bookmark{t.count === 1 ? "" : "s"}</span>
            <span className="ml-auto flex gap-1">
              <button
                onClick={() => { setRenaming(t.name); setNext(t.name); setError(""); }}
                className="rounded-lg p-1.5 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                title="Rename (merges onto existing)"
              >
                <Pencil className="h-4 w-4" />
              </button>
              <button
                onClick={() => { setDeleting({ tag: t.name, count: t.count }); setError(""); }}
                className="rounded-lg p-1.5 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                title="Delete"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </span>
          </div>
        ))}
      </div>
      {renaming && (
        <Modal onClose={() => setRenaming(null)}>
          <h2 className="mb-3 text-base font-semibold">Rename #{renaming}</h2>
          <p className="mb-2 text-sm text-neutral-500">Renaming onto an existing tag merges them.</p>
          <Input value={next} onChange={(e) => setNext(e.target.value)} autoFocus />
          <div className="mt-3 flex gap-2">
            <Button onClick={() => rename.mutate(next)} disabled={rename.isPending || !next.trim()}>
              {rename.isPending ? "Renaming..." : "Rename"}
            </Button>
            <Button variant="ghost" onClick={() => setRenaming(null)}>
              Cancel
            </Button>
          </div>
        </Modal>
      )}
      {deleting && (
        <Modal onClose={() => setDeleting(null)}>
          <h2 className="mb-3 text-base font-semibold">Delete #{deleting.tag}?</h2>
          <p className="mb-3 text-sm text-neutral-500">
            Removes the tag from {deleting.count} bookmark{deleting.count === 1 ? "" : "s"}. Bookmarks stay.
          </p>
          <div className="flex gap-2">
            <Button variant="danger" onClick={() => remove.mutate()} disabled={remove.isPending}>
              {remove.isPending ? "Deleting..." : "Delete tag"}
            </Button>
            <Button variant="ghost" onClick={() => setDeleting(null)}>
              Cancel
            </Button>
          </div>
        </Modal>
      )}
    </div>
  );
}

export function FoldersPage() {
  const [renaming, setRenaming] = useState<string | null>(null);
  const [next, setNext] = useState("");
  const [deleting, setDeleting] = useState<{ folder: string; count: number } | null>(null);
  const [error, setError] = useState("");
  const qc = useQueryClient();
  const folders = useQuery({ queryKey: ["folders"], queryFn: fetchFolders });

  const rename = useMutation({
    mutationFn: (to: string) => renameFolder(renaming!, to),
    onSuccess: () => {
      setRenaming(null);
      setNext("");
      refresh(qc);
    },
    onError: (e: Error) => setError(e.message),
  });

  const remove = useMutation({
    mutationFn: () => deleteFolder(deleting!.folder),
    onSuccess: () => {
      setDeleting(null);
      refresh(qc);
    },
    onError: (e: Error) => setError(e.message),
  });

  return (
    <div className="flex flex-col gap-3">
      <h1 className="text-lg font-semibold">Folders</h1>
      {error && <p className="text-sm text-red-600">{error}</p>}
      {folders.isLoading && <Spinner />}
      {folders.data && folders.data.folders.length === 0 && <Empty title="No folders yet" hint="Everything lives at the root." />}
      <div className="flex flex-col gap-1">
        {(folders.data?.folders ?? []).map((f) => (
          <div
            key={f.name}
            className="flex items-center gap-2 rounded-xl border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-800 dark:bg-neutral-900"
          >
            <span className="text-sm font-medium">{f.name}</span>
            <span className="text-xs text-neutral-400">{f.count} bookmark{f.count === 1 ? "" : "s"}</span>
            <span className="ml-auto flex gap-1">
              <button
                onClick={() => { setRenaming(f.name); setNext(f.name); setError(""); }}
                className="rounded-lg p-1.5 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                title="Rename (moves subfolders too)"
              >
                <Pencil className="h-4 w-4" />
              </button>
              <button
                onClick={() => { setDeleting({ folder: f.name, count: f.count }); setError(""); }}
                className="rounded-lg p-1.5 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                title="Move to root"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </span>
          </div>
        ))}
      </div>
      {renaming && (
        <Modal onClose={() => setRenaming(null)}>
          <h2 className="mb-3 text-base font-semibold">Rename {renaming}</h2>
          <p className="mb-2 text-sm text-neutral-500">Subfolders move along. Files relocate on disk.</p>
          <Input value={next} onChange={(e) => setNext(e.target.value)} autoFocus />
          <div className="mt-3 flex gap-2">
            <Button onClick={() => rename.mutate(next)} disabled={rename.isPending || !next.trim()}>
              {rename.isPending ? "Renaming..." : "Rename"}
            </Button>
            <Button variant="ghost" onClick={() => setRenaming(null)}>
              Cancel
            </Button>
          </div>
        </Modal>
      )}
      {deleting && (
        <Modal onClose={() => setDeleting(null)}>
          <h2 className="mb-3 text-base font-semibold">Remove folder {deleting.folder}?</h2>
          <p className="mb-3 text-sm text-neutral-500">
            Moves {deleting.count} bookmark{deleting.count === 1 ? "" : "s"} to the root. Nothing is deleted.
          </p>
          <div className="flex gap-2">
            <Button variant="danger" onClick={() => remove.mutate()} disabled={remove.isPending}>
              {remove.isPending ? "Moving..." : "Move to root"}
            </Button>
            <Button variant="ghost" onClick={() => setDeleting(null)}>
              Cancel
            </Button>
          </div>
        </Modal>
      )}
    </div>
  );
}

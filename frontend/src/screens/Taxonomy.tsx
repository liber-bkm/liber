import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Pencil, Trash2 } from "lucide-react";
import { deleteFolder, deleteTag, fetchFolders, fetchTags, renameFolder, renameTag } from "../tauri";
import { Button, Empty, Input, Modal, Spinner } from "../components/ui";
import { PathTree } from "../components/PathTree";

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
  const tagItems = useMemo(() => tags.data?.tags ?? [], [tags.data]);

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
      <div className="rounded-xl border border-neutral-200 bg-white px-2 py-2 dark:border-neutral-800 dark:bg-neutral-900">
        <PathTree
          items={tagItems}
          storageKey="liber-tags-page-expanded"
          labelPrefix="#"
          caseInsensitive
          renderActions={(t) => (
            <>
              <button
                onClick={() => { setRenaming(t.path); setNext(t.path); setError(""); }}
                className="rounded-lg p-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                title="Rename (moves sub-tags too)"
              >
                <Pencil className="h-4 w-4" />
              </button>
              <button
                onClick={() => { setDeleting({ tag: t.path, count: t.total }); setError(""); }}
                className="rounded-lg p-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                title="Delete tag and sub-tags"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </>
          )}
        />
      </div>
      {renaming && (
        <Modal onClose={() => setRenaming(null)}>
          <h2 className="mb-3 text-base font-semibold">Rename #{renaming}</h2>
          <p className="mb-2 text-sm text-neutral-500">
            Sub-tags move along. Renaming onto an existing tag merges them.
          </p>
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
            Removes #{deleting.tag} and its sub-tags from {deleting.count} bookmark
            {deleting.count === 1 ? "" : "s"}. Bookmarks stay.
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
  const folderItems = useMemo(() => folders.data?.folders ?? [], [folders.data]);

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

  const deleteTarget = deleting
    ? deleting.folder.includes("/")
      ? deleting.folder.slice(0, deleting.folder.lastIndexOf("/"))
      : "the root"
    : "the root";

  return (
    <div className="flex flex-col gap-3">
      <h1 className="text-lg font-semibold">Folders</h1>
      {error && <p className="text-sm text-red-600">{error}</p>}
      {folders.isLoading && <Spinner />}
      {folders.data && folders.data.folders.length === 0 && <Empty title="No folders yet" hint="Everything lives at the root." />}
      <div className="rounded-xl border border-neutral-200 bg-white px-2 py-2 dark:border-neutral-800 dark:bg-neutral-900">
        <PathTree
          items={folderItems}
          storageKey="liber-folders-page-expanded"
          renderActions={(f) =>
            f.isRoot ? null : (
              <>
                <button
                  onClick={() => { setRenaming(f.path); setNext(f.path); setError(""); }}
                  className="rounded-lg p-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                  title="Rename (moves subfolders too)"
                >
                  <Pencil className="h-4 w-4" />
                </button>
                <button
                  onClick={() => { setDeleting({ folder: f.path, count: f.total }); setError(""); }}
                  className="rounded-lg p-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                  title="Move up one level"
                >
                  <Trash2 className="h-4 w-4" />
                </button>
              </>
            )
          }
        />
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
            Moves {deleting.count} bookmark{deleting.count === 1 ? "" : "s"} and their subfolders up
            to {deleteTarget}. Nothing is deleted.
          </p>
          <div className="flex gap-2">
            <Button variant="danger" onClick={() => remove.mutate()} disabled={remove.isPending}>
              {remove.isPending ? "Moving..." : "Move up"}
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

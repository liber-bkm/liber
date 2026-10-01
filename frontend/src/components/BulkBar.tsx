import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Pencil, Trash2, X } from "lucide-react";
import { bulkOp } from "../api";
import { Button, Input, Modal } from "./ui";

export function BulkBar({ ids, onDone, onClear }: { ids: string[]; onDone: () => void; onClear: () => void }) {
  const [mode, setMode] = useState<"none" | "tags" | "folder" | "delete">("none");
  const [tags, setTags] = useState("");
  const [folder, setFolder] = useState("");
  const [error, setError] = useState("");
  const qc = useQueryClient();

  const done = () => {
    qc.invalidateQueries({ queryKey: ["bookmarks"] });
    qc.invalidateQueries({ queryKey: ["tags"] });
    qc.invalidateQueries({ queryKey: ["folders"] });
    onDone();
  };

  const run = useMutation({
    mutationFn: async () => {
      if (mode === "tags") {
        return bulkOp(ids, "set_tags", { tags: tags.split(/[\s,]+/).filter(Boolean) });
      }
      if (mode === "folder") {
        return bulkOp(ids, "move_folder", { folder });
      }
      return bulkOp(ids, "delete", { confirm: true });
    },
    onSuccess: () => {
      setMode("none");
      done();
    },
    onError: (e: Error) => setError(e.message),
  });

  if (ids.length === 0) return null;

  return (
    <>
      <div className="fixed bottom-4 left-1/2 z-40 flex -translate-x-1/2 items-center gap-2 rounded-2xl border border-neutral-200 bg-white px-3 py-2 shadow-lg dark:border-neutral-700 dark:bg-neutral-900">
        <span className="text-sm text-neutral-500">
          {ids.length} selected
        </span>
        <Button variant="outline" onClick={() => { setMode("tags"); setError(""); }}>
          <Pencil className="h-4 w-4" /> Tags
        </Button>
        <Button variant="outline" onClick={() => { setMode("folder"); setError(""); }}>
          <Pencil className="h-4 w-4" /> Folder
        </Button>
        <Button variant="ghost" onClick={() => { setMode("delete"); setError(""); }}>
          <Trash2 className="h-4 w-4" /> Delete
        </Button>
        <button onClick={onClear} className="rounded-lg p-1.5 text-neutral-400 hover:bg-neutral-100 dark:hover:bg-neutral-800">
          <X className="h-4 w-4" />
        </button>
      </div>
      {mode !== "none" && (
        <Modal onClose={() => setMode("none")}>
          <h2 className="mb-3 text-base font-semibold">
            {mode === "tags" && `Set tags on ${ids.length} bookmark${ids.length === 1 ? "" : "s"}`}
            {mode === "folder" && `Move ${ids.length} bookmark${ids.length === 1 ? "" : "s"}`}
            {mode === "delete" && `Delete ${ids.length} bookmark${ids.length === 1 ? "" : "s"}?`}
          </h2>
          {error && <p className="mb-2 text-sm text-red-600">{error}</p>}
          {mode === "tags" && (
            <Input value={tags} placeholder="space separated (empty clears)" onChange={(e) => setTags(e.target.value)} autoFocus />
          )}
          {mode === "folder" && (
            <Input value={folder} placeholder="folder path (empty for root)" onChange={(e) => setFolder(e.target.value)} autoFocus />
          )}
          {mode === "delete" && (
            <p className="text-sm text-neutral-500">This removes the bookmarks and their files. This cannot be undone.</p>
          )}
          <div className="mt-3 flex gap-2">
            <Button variant={mode === "delete" ? "danger" : "primary"} onClick={() => run.mutate()} disabled={run.isPending}>
              {run.isPending ? "Working..." : mode === "delete" ? "Delete" : "Apply"}
            </Button>
            <Button variant="ghost" onClick={() => setMode("none")}>
              Cancel
            </Button>
          </div>
        </Modal>
      )}
    </>
  );
}

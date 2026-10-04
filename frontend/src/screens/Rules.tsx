import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Pencil, Play, Sparkles, Trash2 } from "lucide-react";
import { addRule, applyRules, deleteRule, editRule, fetchRules, learnCreate, learnSuggestions } from "../tauri";
import { Button, Empty, Field, Input, Modal, Spinner } from "../components/ui";

function refresh(qc: ReturnType<typeof useQueryClient>) {
  qc.invalidateQueries({ queryKey: ["rules"] });
  qc.invalidateQueries({ queryKey: ["bookmarks"] });
}

export function RulesPage() {
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);
  const [pattern, setPattern] = useState("");
  const [tags, setTags] = useState("");
  const [folder, setFolder] = useState("");
  const [reapply, setReapply] = useState(true);
  const [error, setError] = useState("");
  const qc = useQueryClient();
  const rules = useQuery({ queryKey: ["rules"], queryFn: fetchRules });
  const suggestions = useQuery({ queryKey: ["learn"], queryFn: () => learnSuggestions(2) });

  const save = useMutation({
    mutationFn: () => {
      const input = {
        pattern,
        tags: tags.split(/[\s,]+/).filter(Boolean),
        folder: folder || undefined,
      };
      return editing ? editRule(editing, { ...input, reapply }) : addRule(input);
    },
    onSuccess: () => {
      setAdding(false);
      setEditing(null);
      setPattern("");
      setTags("");
      setFolder("");
      refresh(qc);
    },
    onError: (e: Error) => setError(e.message),
  });

  const remove = useMutation({
    mutationFn: (id: string) => deleteRule(id),
    onSuccess: () => refresh(qc),
    onError: (e: Error) => setError(e.message),
  });

  const apply = useMutation({
    mutationFn: (id?: string) => applyRules(id),
    onSuccess: () => refresh(qc),
    onError: (e: Error) => setError(e.message),
  });

  const learn = useMutation({
    mutationFn: () => learnCreate(2),
    onSuccess: () => {
      refresh(qc);
      qc.invalidateQueries({ queryKey: ["learn"] });
    },
    onError: (e: Error) => setError(e.message),
  });

  function startAdd() {
    setEditing(null);
    setPattern("");
    setTags("");
    setFolder("");
    setReapply(false);
    setError("");
    setAdding(true);
  }

  function startEdit(id: string, p: string, t: string[], f?: string) {
    setEditing(id);
    setPattern(p);
    setTags(t.join(" "));
    setFolder(f ?? "");
    setReapply(true);
    setError("");
    setAdding(true);
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">Automation rules</h1>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => apply.mutate(undefined)}>
            <Play className="h-4 w-4" /> Apply all
          </Button>
          <Button onClick={startAdd}>New rule</Button>
        </div>
      </div>
      {error && <p className="text-sm text-red-600">{error}</p>}
      {rules.isLoading && <Spinner />}
      {rules.data && rules.data.rules.length === 0 && (
        <Empty title="No rules yet" hint="Rules file new bookmarks into folders and tags automatically." />
      )}
      <div className="flex flex-col gap-1">
        {(rules.data?.rules ?? []).map((r) => (
          <div
            key={r.id}
            className="flex items-center gap-2 rounded-xl border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-800 dark:bg-neutral-900"
          >
            <div className="min-w-0">
              <p className="truncate text-sm font-medium">{r.description}</p>
              <p className="text-xs text-neutral-400">applied to {r.applied_count} bookmark{r.applied_count === 1 ? "" : "s"}</p>
            </div>
            <span className="ml-auto flex gap-1">
              <button onClick={() => apply.mutate(r.id)} className="rounded-lg p-1.5 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800" title="Apply">
                <Play className="h-4 w-4" />
              </button>
              <button onClick={() => startEdit(r.id, r.pattern, r.tags, r.folder)} className="rounded-lg p-1.5 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800" title="Edit">
                <Pencil className="h-4 w-4" />
              </button>
              <button onClick={() => remove.mutate(r.id)} className="rounded-lg p-1.5 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800" title="Delete">
                <Trash2 className="h-4 w-4" />
              </button>
            </span>
          </div>
        ))}
      </div>

      <div className="rounded-2xl border border-neutral-200 bg-white p-4 dark:border-neutral-800 dark:bg-neutral-900">
        <h2 className="mb-1 flex items-center gap-1.5 text-sm font-semibold">
          <Sparkles className="h-4 w-4" /> Suggested rules
        </h2>
        {(suggestions.data?.suggestions ?? []).length === 0 && (
          <p className="text-sm text-neutral-400">No host appears in one folder often enough.</p>
        )}
        {(suggestions.data?.suggestions ?? []).map((s) => (
          <p key={s.host} className="text-sm text-neutral-500">
            {s.count} bookmarks with host {s.host} live in {s.folder || "/"}
          </p>
        ))}
        {(suggestions.data?.suggestions ?? []).length > 0 && (
          <div className="mt-2">
            <Button variant="outline" onClick={() => learn.mutate()} disabled={learn.isPending}>
              {learn.isPending ? "Creating..." : "Create all suggestions"}
            </Button>
          </div>
        )}
      </div>

      {adding && (
        <Modal onClose={() => setAdding(false)}>
          <h2 className="mb-3 text-base font-semibold">{editing ? "Edit rule" : "New rule"}</h2>
          {error && <p className="mb-2 text-sm text-red-600">{error}</p>}
          <div className="flex flex-col gap-3">
            <Field label="Match (host:example.com, title:word, or substring)">
              <Input value={pattern} onChange={(e) => setPattern(e.target.value)} autoFocus />
            </Field>
            <div className="grid grid-cols-2 gap-3">
              <Field label="Folder">
                <Input value={folder} onChange={(e) => setFolder(e.target.value)} />
              </Field>
              <Field label="Tags">
                <Input value={tags} onChange={(e) => setTags(e.target.value)} />
              </Field>
            </div>
            {editing && (
              <label className="flex items-center gap-2 text-sm">
                <input type="checkbox" checked={reapply} onChange={(e) => setReapply(e.target.checked)} />
                Reapply to existing bookmarks
              </label>
            )}
            <div>
              <Button onClick={() => save.mutate()} disabled={save.isPending || !pattern.trim()}>
                {save.isPending ? "Saving..." : editing ? "Save" : "Create rule"}
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </div>
  );
}

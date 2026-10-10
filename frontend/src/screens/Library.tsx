import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDownWideNarrow, LayoutGrid, Search, Table2 } from "lucide-react";
import { ageOf, displayId, domainOf, type Bookmark } from "../api";
import { faviconHost, faviconUrl, fetchFaviconContent, isTauri, listBookmarks } from "../tauri";
import { Badge, Empty, Input, Spinner } from "../components/ui";
import { BulkBar } from "../components/BulkBar";

const SORTS = [
  { value: "newest", label: "Newest" },
  { value: "", label: "Relevance" },
  { value: "oldest", label: "Oldest" },
  { value: "visited", label: "Most visited" },
  { value: "title", label: "Title" },
];

const SCOPES = [
  { value: "", label: "All fields" },
  { value: "title", label: "Title" },
  { value: "url", label: "URL" },
  { value: "tag", label: "Tags" },
  { value: "folder", label: "Folder" },
  { value: "desc", label: "Notes" },
];

export function Library({
  folder,
  tag,
  onOpen,
}: {
  folder: string | null;
  tag: string | null;
  onOpen: (b: Bookmark) => void;
}) {
  const [q, setQ] = useState("");
  const [debounced, setDebounced] = useState("");
  const [sort, setSort] = useState("newest");
  const [scope, setScope] = useState("");
  const [table, setTable] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const qc = useQueryClient();

  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(q), 250);
    return () => window.clearTimeout(t);
  }, [q]);

  useEffect(() => {
    setSelected(new Set());
  }, [debounced, sort, folder, tag, scope]);

  function toggle(uuid: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(uuid)) next.delete(uuid);
      else next.add(uuid);
      return next;
    });
  }

  const query = useQuery({
    queryKey: ["bookmarks", debounced, sort, folder, tag, scope],
    queryFn: () =>
      listBookmarks({
        q: debounced || undefined,
        sort: sort || undefined,
        folder: folder ?? undefined,
        tag: tag ?? undefined,
        per_page: 100,
        scope: scope || undefined,
      }),
  });

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
          <Input
            value={q}
            placeholder="Search bookmarks... (title:foo tag:bar)"
            className="pl-9"
            onChange={(e) => setQ(e.target.value)}
          />
        </div>
        <div className="relative">
          <select
            value={scope}
            onChange={(e) => setScope(e.target.value)}
            title="Restrict unscoped words to these fields (field: prefixes always win)"
            className="appearance-none rounded-lg border border-neutral-300 bg-white py-1.5 pl-3 pr-3 text-sm dark:border-neutral-700 dark:bg-neutral-900"
          >
            {SCOPES.map((s) => (
              <option key={s.value} value={s.value}>
                {s.label}
              </option>
            ))}
          </select>
        </div>
        <div className="relative">
          <ArrowDownWideNarrow className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
          <select
            value={sort}
            onChange={(e) => setSort(e.target.value)}
            className="appearance-none rounded-lg border border-neutral-300 bg-white py-1.5 pl-8 pr-3 text-sm dark:border-neutral-700 dark:bg-neutral-900"
          >
            {SORTS.map((s) => (
              <option key={s.value} value={s.value}>
                {s.label}
              </option>
            ))}
          </select>
        </div>
        <button
          onClick={() => setTable(!table)}
          className="rounded-lg border border-neutral-300 p-1.5 text-neutral-500 hover:bg-neutral-100 dark:border-neutral-700 dark:hover:bg-neutral-800"
          title={table ? "Card view" : "Table view"}
        >
          {table ? <LayoutGrid className="h-4 w-4" /> : <Table2 className="h-4 w-4" />}
        </button>
      </div>

      {(folder || tag) && (
        <div className="flex items-center gap-2 text-sm text-neutral-500">
          {folder && <Badge tone="accent">{folder}</Badge>}
          {tag && <Badge tone="accent">#{tag}</Badge>}
          {query.data && <span>{query.data.total} result{query.data.total === 1 ? "" : "s"}</span>}
        </div>
      )}

      {query.isLoading && (
        <div className="flex justify-center py-16">
          <Spinner />
        </div>
      )}
      {query.data && query.data.bookmarks.length === 0 && (
        <Empty title="No bookmarks found" hint="Try a different search or add a new bookmark." />
      )}
      {query.data && query.data.bookmarks.length > 0 && !table && (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {query.data.bookmarks.map((b) => (
            <Card key={b.uuid} bookmark={b} selected={selected.has(b.uuid)} onToggle={() => toggle(b.uuid)} onOpen={() => onOpen(b)} />
          ))}
        </div>
      )}
      {query.data && query.data.bookmarks.length > 0 && table && (
        <Rows bookmarks={query.data.bookmarks} selected={selected} onToggle={toggle} onOpen={onOpen} />
      )}
      <BulkBar
        ids={[...selected]}
        onClear={() => setSelected(new Set())}
        onDone={() => {
          setSelected(new Set());
          qc.invalidateQueries({ queryKey: ["bookmarks"] });
        }}
      />
    </div>
  );
}

function Avatar({ title, url }: { title: string; url: string }) {
  const letter = (title || url || "?").slice(0, 1).toUpperCase();
  return (
    <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent-50 text-sm font-semibold text-accent-700 dark:bg-accent-700/20 dark:text-accent-100">
      {letter}
    </div>
  );
}

function StatusDot({ b }: { b: Bookmark }) {
  if (!b.check_status || b.check_status === "ok") return null;
  const tone = b.check_status === "dead" ? "red" : b.check_status === "moved" ? "amber" : "neutral";
  return <Badge tone={tone as "red" | "amber" | "neutral"}>{b.check_status}</Badge>;
}

function Card({ bookmark: b, selected, onToggle, onOpen }: { bookmark: Bookmark; selected: boolean; onToggle: () => void; onOpen: () => void }) {
  return (
    <div
      className={`relative flex flex-col gap-2 rounded-2xl border bg-white p-4 text-left shadow-sm transition-shadow hover:shadow-md dark:bg-neutral-900 ${selected ? "border-accent-500" : "border-neutral-200 dark:border-neutral-800"}`}
    >
      <input
        type="checkbox"
        checked={selected}
        onChange={onToggle}
        onClick={(e) => e.stopPropagation()}
        className="absolute right-3 top-3 h-4 w-4 accent-[#2549a8]"
        title="Select"
      />
      <button onClick={onOpen} className="flex flex-col gap-2 text-left">
        <div className="flex items-start gap-2.5">
          <Avatar title={b.title} url={b.url} />
          <div className="min-w-0 pr-5">
            <p className="truncate text-sm font-medium">
              <span className="mr-1 font-mono text-xs font-normal text-neutral-400">{displayId(b)}</span>
              {b.title}
            </p>
            <p className="truncate text-xs text-neutral-400">{domainOf(b.url)}</p>
          </div>
        </div>
        {b.description && <p className="line-clamp-2 text-sm text-neutral-500">{b.description}</p>}
        <div className="mt-auto flex flex-wrap items-center gap-1.5 pt-1">
          {b.folder && <span className="text-xs text-neutral-400">{b.folder}</span>}
          {(b.tags ?? []).slice(0, 3).map((t) => (
            <Badge key={t}>#{t}</Badge>
          ))}
          <span className="ml-auto flex items-center gap-1.5 text-xs text-neutral-400">
            <StatusDot b={b} />
            {b.has_archive && <Badge tone="green">arc</Badge>}
            {ageOf(b.created_at)}
          </span>
        </div>
      </button>
    </div>
  );
}

function Rows({ bookmarks, selected, onToggle, onOpen }: { bookmarks: Bookmark[]; selected: Set<string>; onToggle: (uuid: string) => void; onOpen: (b: Bookmark) => void }) {
  return (
    <div className="overflow-hidden rounded-2xl border border-neutral-200 bg-white dark:border-neutral-800 dark:bg-neutral-900">
      {bookmarks.map((b) => (
        <div
          key={b.uuid}
          className={`flex w-full items-center gap-3 border-b border-neutral-100 px-4 py-2 text-left text-sm last:border-0 hover:bg-neutral-50 dark:border-neutral-800 dark:hover:bg-neutral-800/50 ${selected.has(b.uuid) ? "bg-accent-50 dark:bg-accent-700/10" : ""}`}
        >
          <input type="checkbox" checked={selected.has(b.uuid)} onChange={() => onToggle(b.uuid)} className="h-4 w-4 shrink-0 accent-[#2549a8]" title="Select" />
          <button onClick={() => onOpen(b)} className="flex min-w-0 flex-1 items-center gap-3 text-left">
            <span className="w-16 shrink-0 font-mono text-xs text-neutral-400">{displayId(b)}</span>
            <span className="min-w-0 flex-1 truncate font-medium">{b.title}</span>
            <span className="hidden max-w-48 truncate text-xs text-neutral-400 sm:block">{domainOf(b.url)}</span>
            <StatusDot b={b} />
            <span className="w-16 shrink-0 text-right text-xs text-neutral-400">{ageOf(b.created_at)}</span>
          </button>
        </div>
      ))}
    </div>
  );
}

import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowDownWideNarrow, LayoutGrid, Search, Table2 } from "lucide-react";
import { ageOf, domainOf, fetchBookmarks, shortUuid, type Bookmark } from "../api";
import { Badge, Empty, Input, Spinner } from "../components/ui";

const SORTS = [
  { value: "", label: "Relevance" },
  { value: "newest", label: "Newest" },
  { value: "oldest", label: "Oldest" },
  { value: "visited", label: "Most visited" },
  { value: "title", label: "Title" },
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
  const [sort, setSort] = useState("");
  const [table, setTable] = useState(false);

  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(q), 250);
    return () => window.clearTimeout(t);
  }, [q]);

  const query = useQuery({
    queryKey: ["bookmarks", debounced, sort, folder, tag],
    queryFn: () =>
      fetchBookmarks({
        q: debounced || undefined,
        sort: sort || undefined,
        folder: folder ?? undefined,
        tag: tag ?? undefined,
        per_page: 100,
      }),
  });

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
          <Input
            value={q}
            placeholder="Search bookmarks..."
            className="pl-9"
            onChange={(e) => setQ(e.target.value)}
          />
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
            <Card key={b.uuid} bookmark={b} onOpen={() => onOpen(b)} />
          ))}
        </div>
      )}
      {query.data && query.data.bookmarks.length > 0 && table && (
        <Rows bookmarks={query.data.bookmarks} onOpen={onOpen} />
      )}
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

function Card({ bookmark: b, onOpen }: { bookmark: Bookmark; onOpen: () => void }) {
  return (
    <button
      onClick={onOpen}
      className="flex flex-col gap-2 rounded-2xl border border-neutral-200 bg-white p-4 text-left shadow-sm transition-shadow hover:shadow-md dark:border-neutral-800 dark:bg-neutral-900"
    >
      <div className="flex items-start gap-2.5">
        <Avatar title={b.title} url={b.url} />
        <div className="min-w-0">
          <p className="truncate text-sm font-medium">{b.title}</p>
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
  );
}

function Rows({ bookmarks, onOpen }: { bookmarks: Bookmark[]; onOpen: (b: Bookmark) => void }) {
  return (
    <div className="overflow-hidden rounded-2xl border border-neutral-200 bg-white dark:border-neutral-800 dark:bg-neutral-900">
      {bookmarks.map((b) => (
        <button
          key={b.uuid}
          onClick={() => onOpen(b)}
          className="flex w-full items-center gap-3 border-b border-neutral-100 px-4 py-2 text-left text-sm last:border-0 hover:bg-neutral-50 dark:border-neutral-800 dark:hover:bg-neutral-800/50"
        >
          <span className="w-16 shrink-0 font-mono text-xs text-neutral-400">{shortUuid(b.uuid)}</span>
          <span className="min-w-0 flex-1 truncate font-medium">{b.title}</span>
          <span className="hidden max-w-48 truncate text-xs text-neutral-400 sm:block">{domainOf(b.url)}</span>
          <StatusDot b={b} />
          <span className="w-16 shrink-0 text-right text-xs text-neutral-400">{ageOf(b.created_at)}</span>
        </button>
      ))}
    </div>
  );
}

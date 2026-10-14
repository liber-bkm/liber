import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useWindowVirtualizer } from "@tanstack/react-virtual";
import { ArrowDownWideNarrow, BookmarkPlus, LayoutGrid, Search, Table2 } from "lucide-react";
import { ageOf, displayId, domainOf, type Bookmark } from "../api";
import { addSearch, faviconHost, faviconUrl, fetchFaviconContent, isTauri, listBookmarks } from "../tauri";
import { Badge, Button, Empty, Input, Spinner } from "../components/ui";
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

export interface SearchPreset {
  q: string;
  scope: string;
  sort: string;
  deep: boolean;
  nonce: number;
}

export function Library({
  folder,
  tag,
  onOpen,
  preset,
  onAdd,
  onGoSettings,
}: {
  folder: string | null;
  tag: string | null;
  onOpen: (b: Bookmark) => void;
  preset?: SearchPreset | null;
  onAdd?: () => void;
  onGoSettings?: () => void;
}) {
  const [q, setQ] = useState("");
  const [debounced, setDebounced] = useState("");
  const [sort, setSort] = useState("newest");
  const [scope, setScope] = useState("");
  const [deep, setDeep] = useState(false);
  const [table, setTable] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [saveError, setSaveError] = useState("");
  const qc = useQueryClient();

  useEffect(() => {
    if (!preset) return;
    setQ(preset.q);
    setScope(preset.scope);
    setSort(preset.sort);
    setDeep(preset.deep);
  }, [preset]);

  const savePreset = useMutation({
    mutationFn: (name: string) => addSearch({ name, query: debounced, scope, sort, deep: deep || undefined }),
    onSuccess: () => {
      setSaveError("");
      qc.invalidateQueries({ queryKey: ["searches"] });
    },
    onError: (e: Error) => setSaveError(e.message),
  });

  const canSave = debounced.trim() !== "" || scope !== "" || (sort !== "" && sort !== "newest") || deep;

  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(q), 250);
    return () => window.clearTimeout(t);
  }, [q]);

  useEffect(() => {
    setSelected(new Set());
  }, [debounced, sort, folder, tag, scope, deep]);

  function toggle(uuid: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(uuid)) next.delete(uuid);
      else next.add(uuid);
      return next;
    });
  }

  const query = useQuery({
    queryKey: ["bookmarks", debounced, sort, folder, tag, scope, deep],
    queryFn: () =>
      listBookmarks({
        q: debounced || undefined,
        sort: sort || undefined,
        folder: folder ?? undefined,
        tag: tag ?? undefined,
        per_page: 500,
        scope: scope || undefined,
        deep: deep || undefined,
      }),
  });
  const bookmarks = query.data?.bookmarks ?? [];
  const isFreshLibrary =
    query.data != null &&
    query.data.total === 0 &&
    debounced.trim() === "" &&
    folder == null &&
    tag == null;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-0 grow basis-52">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
          <Input
            value={q}
            placeholder="Search bookmarks..."
            title="Search, supports field:value prefixes like title:foo tag:bar"
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
        <label
          className="flex items-center gap-1.5 rounded-lg border border-neutral-300 px-2.5 py-1.5 text-sm text-neutral-500 dark:border-neutral-700"
          title="Include saved page content and notes in search (slower). Matches show highlighted snippets; click a snippet to expand it."
        >
          <input type="checkbox" checked={deep} onChange={(e) => setDeep(e.target.checked)} className="h-4 w-4 accent-[#2549a8]" />
          Deep
        </label>
        {canSave && (
          <button
            onClick={() => {
              const name = window.prompt("Name this search");
              if (name && name.trim()) savePreset.mutate(name.trim());
            }}
            disabled={savePreset.isPending}
            className="rounded-lg border border-neutral-300 p-1.5 text-neutral-500 hover:bg-neutral-100 dark:border-neutral-700 dark:hover:bg-neutral-800"
            title="Save current search"
          >
            <BookmarkPlus className="h-4 w-4" />
          </button>
        )}
      </div>
      {saveError && <p className="text-sm text-red-600">{saveError}</p>}

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
      {query.data && query.data.bookmarks.length === 0 && !isFreshLibrary && (
        <Empty title="No bookmarks found" hint="Try a different search or add a new bookmark." />
      )}
      {isFreshLibrary && (
        <div className="flex flex-col items-center gap-3 rounded-2xl border border-neutral-200 bg-white px-6 py-12 text-center dark:border-neutral-800 dark:bg-neutral-900">
          <p className="text-base font-semibold">Welcome to liber</p>
          <p className="max-w-sm text-sm text-neutral-500">
            Your library is empty. Add your first bookmark, import from your browser, or pick how pages are archived.
          </p>
          <div className="flex flex-wrap justify-center gap-2">
            {onAdd && (
              <Button onClick={onAdd}>Add your first bookmark</Button>
            )}
            {onGoSettings && (
              <Button variant="outline" onClick={onGoSettings}>
                Import or settings
              </Button>
            )}
          </div>
          <p className="max-w-sm text-xs text-neutral-400">
            Tip: capture from any browser with the bookmarklet, and set the archive backend under Settings.
          </p>
        </div>
      )}
      {bookmarks.length > 0 && !table && (
        <CardGrid bookmarks={bookmarks} selected={selected} onToggle={toggle} onOpen={onOpen} />
      )}
      {bookmarks.length > 0 && table && (
        <RowList bookmarks={bookmarks} selected={selected} onToggle={toggle} onOpen={onOpen} />
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

function useColumns() {
  const get = () => {
    if (typeof window.matchMedia !== "function") return 1;
    return window.matchMedia("(min-width: 1024px)").matches
      ? 3
      : window.matchMedia("(min-width: 640px)").matches
        ? 2
        : 1;
  };
  const [cols, setCols] = useState(get);
  useEffect(() => {
    const onResize = () => setCols(get());
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return cols;
}

function chunk<T>(items: T[], size: number): T[][] {
  const rows: T[][] = [];
  for (let i = 0; i < items.length; i += size) rows.push(items.slice(i, i + size));
  return rows;
}

function CardGrid({ bookmarks, selected, onToggle, onOpen }: { bookmarks: Bookmark[]; selected: Set<string>; onToggle: (uuid: string) => void; onOpen: (b: Bookmark) => void }) {
  const columns = useColumns();
  const rows = useMemo(() => chunk(bookmarks, columns), [bookmarks, columns]);
  const virtualizer = useWindowVirtualizer({
    count: rows.length,
    estimateSize: () => 240,
    overscan: 2,
  });
  return (
    <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
      {virtualizer.getVirtualItems().map((vi) => (
        <div
          key={vi.key}
          data-index={vi.index}
          ref={virtualizer.measureElement}
          style={{ position: "absolute", top: 0, left: 0, width: "100%", transform: `translateY(${vi.start}px)` }}
        >
          <div className="grid grid-cols-1 gap-3 pb-3 sm:grid-cols-2 lg:grid-cols-3">
            {rows[vi.index].map((b) => (
              <Card key={b.uuid} bookmark={b} selected={selected.has(b.uuid)} onToggle={() => onToggle(b.uuid)} onOpen={() => onOpen(b)} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function Avatar({ title, url, size = "md" }: { title: string; url: string; size?: "sm" | "md" }) {
  const letter = (title || url || "?").slice(0, 1).toUpperCase();
  const cls =
    size === "sm"
      ? "h-5 w-5 rounded-md text-xs"
      : "h-9 w-9 rounded-lg text-sm";
  return (
    <div className={`flex shrink-0 items-center justify-center bg-accent-50 font-semibold text-accent-700 dark:bg-accent-700/20 dark:text-accent-100 ${cls}`}>
      {letter}
    </div>
  );
}

function Favicon({ title, url, size = "md" }: { title: string; url: string; size?: "sm" | "md" }) {
  const host = faviconHost(url);
  const [src, setSrc] = useState<string | null>(!isTauri() && host ? faviconUrl(host) : null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (isTauri() && host) {
      fetchFaviconContent(host)
        .then((r) => setSrc(`data:${r.mime};base64,${r.content}`))
        .catch(() => setFailed(true));
    }
  }, [host]);
  if (!src || failed) return <Avatar title={title} url={url} size={size} />;
  return (
    <img
      src={src}
      alt=""
      loading="lazy"
      decoding="async"
      onError={() => setFailed(true)}
      className={
        size === "sm"
          ? "h-5 w-5 shrink-0 rounded-md object-contain"
          : "h-9 w-9 shrink-0 rounded-lg object-contain"
      }
    />
  );
}

function Snippet({ html }: { html?: string }) {
  const [expanded, setExpanded] = useState(false);
  if (!html) return null;
  return (
    <span
      onClick={(e) => {
        e.stopPropagation();
        setExpanded((v) => !v);
      }}
      title={expanded ? "Show less" : "Show full match"}
      className={`cursor-pointer text-xs text-neutral-500 [&_b]:text-accent-700 ${expanded ? "" : "line-clamp-2"}`}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

function RowSnippet({ html }: { html: string }) {
  const [expanded, setExpanded] = useState(false);
  return (
    <span
      onClick={(e) => {
        e.stopPropagation();
        setExpanded((v) => !v);
      }}
      title={expanded ? "Show less" : "Show full match"}
      className={`hidden max-w-64 flex-1 cursor-pointer text-xs text-neutral-400 lg:block [&_b]:text-accent-700 ${expanded ? "whitespace-normal" : "truncate"}`}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

function StatusDot({ b }: { b: Bookmark }) {  if (!b.check_status || b.check_status === "ok") return null;
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
          <Favicon title={b.title} url={b.url} />
          <div className="min-w-0 pr-5">
            <p className="truncate text-sm font-medium">
              <span className="mr-1 font-mono text-xs font-normal text-neutral-400">{displayId(b)}</span>
              {b.title}
            </p>
            <p className="truncate text-xs text-neutral-400">{domainOf(b.url)}</p>
          </div>
        </div>
        {b.description && <p className="line-clamp-2 text-sm text-neutral-500">{b.description}</p>}
        <Snippet html={b.snippet} />
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

function RowList({ bookmarks, selected, onToggle, onOpen }: { bookmarks: Bookmark[]; selected: Set<string>; onToggle: (uuid: string) => void; onOpen: (b: Bookmark) => void }) {
  const virtualizer = useWindowVirtualizer({
    count: bookmarks.length,
    estimateSize: () => 56,
    overscan: 5,
  });
  return (
    <div className="overflow-hidden rounded-2xl border border-neutral-200 bg-white dark:border-neutral-800 dark:bg-neutral-900">
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualizer.getVirtualItems().map((vi) => {
          const b = bookmarks[vi.index];
          return (
            <div
              key={b.uuid}
              data-index={vi.index}
              ref={virtualizer.measureElement}
              style={{ position: "absolute", top: 0, left: 0, width: "100%", transform: `translateY(${vi.start}px)` }}
              className={`flex w-full items-center gap-3 border-b border-neutral-100 bg-white px-4 py-2 text-left text-sm last:border-0 hover:bg-neutral-50 dark:border-neutral-800 dark:bg-neutral-900 dark:hover:bg-neutral-800/50 ${selected.has(b.uuid) ? "bg-accent-50 dark:bg-accent-700/10" : ""}`}
            >
              <input type="checkbox" checked={selected.has(b.uuid)} onChange={() => onToggle(b.uuid)} className="h-4 w-4 shrink-0 accent-[#2549a8]" title="Select" />
              <button onClick={() => onOpen(b)} className="flex min-w-0 flex-1 items-center gap-3 text-left">
                <span className="w-16 shrink-0 font-mono text-xs text-neutral-400">{displayId(b)}</span>
                <Favicon title={b.title} url={b.url} size="sm" />
                <span className="min-w-0 flex-1 truncate font-medium">{b.title}</span>
                {b.snippet && <RowSnippet html={b.snippet} />}
                <span className="hidden max-w-48 truncate text-xs text-neutral-400 sm:block">{domainOf(b.url)}</span>
                <StatusDot b={b} />
                <span className="w-16 shrink-0 text-right text-xs text-neutral-400">{ageOf(b.created_at)}</span>
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}

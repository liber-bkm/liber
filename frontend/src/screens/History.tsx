import { useQuery } from "@tanstack/react-query";
import { ageOf, displayId, domainOf, type Bookmark } from "../api";
import { fetchHistory } from "../tauri";
import { Empty, Spinner } from "../components/ui";

export function HistoryPage({ onOpen }: { onOpen: (b: Bookmark) => void }) {
  const history = useQuery({ queryKey: ["history"], queryFn: fetchHistory });

  return (
    <div className="flex flex-col gap-3">
      <h1 className="text-lg font-semibold">History</h1>
      {history.isLoading && (
        <div className="flex justify-center py-8">
          <Spinner />
        </div>
      )}
      {history.data && history.data.bookmarks.length === 0 && (
        <Empty title="Nothing opened yet" hint="Open a bookmark and it will appear here." />
      )}
      <div className="flex flex-col gap-1">
        {(history.data?.bookmarks ?? []).map((b) => (
          <button
            key={b.uuid}
            onClick={() => onOpen(b)}
            className="flex items-center gap-3 rounded-xl border border-neutral-200 bg-white px-3 py-2 text-left text-sm hover:shadow-sm dark:border-neutral-800 dark:bg-neutral-900"
          >
            <span className="w-16 shrink-0 font-mono text-xs text-neutral-400">{displayId(b)}</span>
            <span className="min-w-0 flex-1">
              <span className="block truncate font-medium">{b.title}</span>
              <span className="block truncate text-xs text-neutral-400">{domainOf(b.url)}</span>
            </span>
            <span className="shrink-0 text-xs text-neutral-400">
              {b.open_count ? `${b.open_count}x · ` : ""}
              {b.last_opened_at ? ageOf(b.last_opened_at) : ""}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

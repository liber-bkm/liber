import type { Bookmark, BookmarkList, ListParams } from "./api";
import { fetchBookmarks as restList } from "./api";

export function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

interface TauriListResponse {
  total: number;
  page: number;
  per_page: number;
  bookmarks: {
    uuid: string;
    url: string;
    title: string;
    description: string;
    tags: string[];
    folder: string;
    created_at: string;
    updated_at: string;
    has_markdown: boolean;
    has_archive: boolean;
    open_count: number;
  }[];
}

function toBookmarkList(r: TauriListResponse): BookmarkList {
  return {
    total: r.total,
    page: r.page,
    per_page: r.per_page,
    bookmarks: r.bookmarks.map(
      (b): Bookmark => ({
        uuid: b.uuid,
        url: b.url,
        title: b.title,
        description: b.description,
        tags: b.tags,
        folder: b.folder,
        created_at: b.created_at,
        updated_at: b.updated_at,
        has_markdown: b.has_markdown,
        has_archive: b.has_archive,
        open_count: b.open_count,
      })
    ),
  };
}

export async function listBookmarks(params: ListParams = {}): Promise<BookmarkList> {
  if (!isTauri()) return restList(params);
  const { invoke } = await import("@tauri-apps/api/core");
  const r = await invoke<TauriListResponse>("list_bookmarks", {
    q: params.q ?? null,
    sort: params.sort ?? null,
    tag: params.tag ?? null,
    folder: params.folder ?? null,
    page: params.page ?? null,
    perPage: params.per_page ?? null,
  });
  return toBookmarkList(r);
}

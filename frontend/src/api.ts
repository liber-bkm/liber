export interface BookmarkList {
  total: number;
  bookmarks: unknown[];
}

export async function fetchBookmarks(): Promise<BookmarkList> {
  const r = await fetch("/api/v2/bookmarks");
  if (!r.ok) throw new Error("request failed");
  return r.json();
}

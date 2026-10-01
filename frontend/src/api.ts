export interface Attachment {
  name: string;
}

export interface Bookmark {
  uuid: string;
  url: string;
  title: string;
  description?: string;
  tags?: string[];
  folder?: string;
  created_at: string;
  updated_at: string;
  has_markdown: boolean;
  has_archive: boolean;
  attachments?: Attachment[];
  open_count?: number;
  last_opened_at?: string;
  check_status?: string;
}

export interface BookmarkList {
  total: number;
  bookmarks: unknown[];
}

export async function fetchBookmarks(): Promise<BookmarkList> {
  const r = await fetch("/api/v2/bookmarks");
  if (!r.ok) throw new Error("request failed");
  return r.json();
}

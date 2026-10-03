import type {
  AddInput,
  Bookmark,
  BookmarkList,
  FolderCount,
  ListParams,
  TagCount,
  UpdateInput,
} from "./api";
import {
  addBookmark as restAdd,
  ApiError,
  deleteBookmark as restDelete,
  deleteFolder as restDeleteFolder,
  deleteTag as restDeleteTag,
  fetchBookmark as restGet,
  fetchBookmarks as restList,
  fetchFolders as restFolders,
  fetchTags as restTags,
  openBookmark as restOpen,
  renameFolder as restRenameFolder,
  renameTag as restRenameTag,
  updateBookmark as restUpdate,
} from "./api";

export function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

async function invokeCmd<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    if (e instanceof Error) throw e;
    throw new Error(typeof e === "string" ? e : "command failed");
  }
}

interface TauriBookmarkPayload {
  uuid: string;
  short_id?: number | null;
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
  attachments?: string[];
}

interface TauriListResponse {
  total: number;
  page: number;
  per_page: number;
  bookmarks: TauriBookmarkPayload[];
}

function toBookmark(b: TauriBookmarkPayload): Bookmark {
  return {
    uuid: b.uuid,
    short_id: b.short_id ?? undefined,
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
    attachments: (b.attachments ?? []).map((name) => ({ name })),
  };
}

function toBookmarkList(r: TauriListResponse): BookmarkList {
  return {
    total: r.total,
    page: r.page,
    per_page: r.per_page,
    bookmarks: r.bookmarks.map(toBookmark),
  };
}

export async function listBookmarks(params: ListParams = {}): Promise<BookmarkList> {
  if (!isTauri()) return restList(params);
  const r = await invokeCmd<TauriListResponse>("list_bookmarks", {
    q: params.q ?? null,
    sort: params.sort ?? null,
    tag: params.tag ?? null,
    folder: params.folder ?? null,
    page: params.page ?? null,
    per_page: params.per_page ?? null,
  });
  return toBookmarkList(r);
}

export async function fetchBookmark(uuid: string): Promise<Bookmark> {
  if (!isTauri()) return restGet(uuid);
  const b = await invokeCmd<TauriBookmarkPayload>("get_bookmark", { id: uuid });
  return toBookmark(b);
}

export async function addBookmark(input: AddInput): Promise<Bookmark> {
  if (!isTauri()) return restAdd(input);
  try {
    const r = await invokeCmd<{ status: string; bookmark: TauriBookmarkPayload }>(
      "add_bookmark",
      {
        url: input.url,
        title: input.title ?? null,
        description: input.description ?? null,
        tags: input.tags ?? null,
        folder: input.folder ?? null,
        markdown: input.markdown ?? null,
        confirm_dup: input.confirm_dup ?? null,
      }
    );
    return toBookmark(r.bookmark);
  } catch (e) {
    const msg = e instanceof Error ? e.message : "";
    try {
      const parsed = JSON.parse(msg) as {
        error?: string;
        existing?: TauriBookmarkPayload;
        hint?: string;
      };
      if (parsed.error === "duplicate" && parsed.existing) {
        throw new ApiError(409, {
          error: "duplicate",
          existing: toBookmark(parsed.existing),
          hint: parsed.hint ?? "",
        });
      }
    } catch (parseErr) {
      if (parseErr instanceof ApiError) throw parseErr;
    }
    throw e;
  }
}

export async function updateBookmark(uuid: string, input: UpdateInput): Promise<Bookmark> {
  if (!isTauri()) return restUpdate(uuid, input);
  const b = await invokeCmd<TauriBookmarkPayload>("update_bookmark", {
    id: uuid,
    title: input.title ?? null,
    description: input.description ?? null,
    tags: input.tags ?? null,
    folder: input.folder ?? null,
    url: input.url ?? null,
  });
  return toBookmark(b);
}

export async function deleteBookmark(
  uuid: string,
  confirm: boolean
): Promise<{ deleted?: string; confirm_required?: boolean; bookmark?: Bookmark }> {
  if (!isTauri()) return restDelete(uuid, confirm);
  const r = await invokeCmd<{ deleted?: string; confirm_required?: boolean; bookmark?: TauriBookmarkPayload }>(
    "delete_bookmark",
    { id: uuid, confirm }
  );
  return {
    deleted: r.deleted,
    confirm_required: r.confirm_required,
    bookmark: r.bookmark ? toBookmark(r.bookmark) : undefined,
  };
}

export async function openBookmark(uuid: string): Promise<{ url: string }> {
  if (!isTauri()) return restOpen(uuid);
  return invokeCmd<{ url: string }>("open_bookmark", { id: uuid });
}

export async function openExternal(url: string): Promise<void> {
  if (!isTauri()) {
    window.open(url, "_blank", "noopener");
    return;
  }
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}

export async function fetchTags(): Promise<{ tags: TagCount[] }> {
  if (!isTauri()) return restTags();
  return invokeCmd<{ tags: TagCount[] }>("list_tags");
}

export async function fetchFolders(): Promise<{ folders: FolderCount[] }> {
  if (!isTauri()) return restFolders();
  return invokeCmd<{ folders: FolderCount[] }>("list_folders");
}

export async function renameTag(old: string, next: string): Promise<{ renamed: number }> {
  if (!isTauri()) return restRenameTag(old, next);
  return invokeCmd<{ renamed: number }>("rename_tag", { old, newTag: next });
}

export async function deleteTag(
  tag: string,
  confirm: boolean
): Promise<{ deleted?: number; confirm_required?: boolean; count?: number }> {
  if (!isTauri()) return restDeleteTag(tag, confirm);
  return invokeCmd("delete_tag", { tag, confirm });
}

export async function renameFolder(old: string, next: string): Promise<{ renamed: number }> {
  if (!isTauri()) return restRenameFolder(old, next);
  return invokeCmd<{ renamed: number }>("rename_folder", { old, newFolder: next });
}

export async function deleteFolder(folder: string): Promise<{ moved_to_root: number }> {
  if (!isTauri()) return restDeleteFolder(folder);
  return invokeCmd<{ moved_to_root: number }>("delete_folder", { folder });
}

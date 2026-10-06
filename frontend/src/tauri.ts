import type {
  AddInput,
  Bookmark,
  BookmarkList,
  BulkResult,
  CheckReport,
  FolderCount,
  ListParams,
  ReindexReport,
  Rule,
  Settings,
  Suggestion,
  TagCount,
  UpdateInput,
} from "./api";
import {
  addBookmark as restAdd,
  addRule as restAddRule,
  ApiError,
  getRemoteBase,
  useRemote,
  applyRules as restApplyRules,
  bulkOp as restBulk,
  checkApply as restCheckApply,
  checkRun as restCheckRun,
  createArchive as restCreateArchive,
  deleteArchive as restDeleteArchive,
  deleteAttachment as restDeleteAttachment,
  deleteBookmark as restDelete,
  deleteFolder as restDeleteFolder,
  deleteNotes as restDeleteNotes,
  deleteRule as restDeleteRule,
  deleteTag as restDeleteTag,
  editRule as restEditRule,
  exportSite as restExportSite,
  fetchBookmark as restGet,
  fetchBookmarks as restList,
  fetchFolders as restFolders,
  fetchHistory as restHistory,
  fetchNotes as restNotes,
  fetchRules as restRules,
  fetchSettings as restSettings,
  fetchTags as restTags,
  importLibrary as restImport,
  learnCreate as restLearnCreate,
  learnSuggestions as restLearn,
  openBookmark as restOpen,
  renameFolder as restRenameFolder,
  renameTag as restRenameTag,
  runReindex as restReindex,
  saveNotes as restSaveNotes,
  setSetting as restSetSetting,
  updateBookmark as restUpdate,
  uploadAttachment as restUpload,
} from "./api";

export function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export { getRemoteBase, getRemoteToken, setRemote, useRemote } from "./api";

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
  last_opened_at?: string | null;
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
    last_opened_at: b.last_opened_at ?? undefined,
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
  if (!isTauri() || useRemote()) return restList(params);
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
  if (!isTauri() || useRemote()) return restGet(uuid);
  const b = await invokeCmd<TauriBookmarkPayload>("get_bookmark", { id: uuid });
  return toBookmark(b);
}

export async function addBookmark(input: AddInput): Promise<Bookmark> {
  if (!isTauri() || useRemote()) return restAdd(input);
  try {
    const r = await invokeCmd<{
      status: string;
      bookmark: TauriBookmarkPayload;
      warnings?: string[];
    }>("add_bookmark", {
      url: input.url,
      title: input.title ?? null,
      description: input.description ?? null,
      tags: input.tags ?? null,
      folder: input.folder ?? null,
      markdown: input.markdown ?? null,
      archive: input.archive ?? null,
      confirm_dup: input.confirm_dup ?? null,
    });
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
  if (!isTauri() || useRemote()) return restUpdate(uuid, input);
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
  if (!isTauri() || useRemote()) return restDelete(uuid, confirm);
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
  if (!isTauri() || useRemote()) return restOpen(uuid);
  return invokeCmd<{ url: string }>("open_bookmark", { id: uuid });
}

export async function subscribeSharedUrls(
  cb: (url: string) => void
): Promise<(() => void) | undefined> {
  if (!isTauri()) return undefined;
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const emit = (urls: unknown) => {
    const list = Array.isArray(urls) ? urls : [urls];
    for (const raw of list) {
      if (typeof raw !== "string") continue;
      try {
        const shared = new URL(raw).searchParams.get("url");
        if (shared) cb(shared);
      } catch {
        continue;
      }
    }
  };
  try {
    const current = await invoke<string[] | null>("plugin:deep-link|get_current");
    if (current) emit(current);
  } catch {
    /* plugin event only */
  }
  const unlisten = await listen<string[]>("deep-link://new-url", (event) =>
    emit(event.payload)
  );
  return unlisten;
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
  if (!isTauri() || useRemote()) return restTags();
  return invokeCmd<{ tags: TagCount[] }>("list_tags");
}

export async function fetchFolders(): Promise<{ folders: FolderCount[] }> {
  if (!isTauri() || useRemote()) return restFolders();
  return invokeCmd<{ folders: FolderCount[] }>("list_folders");
}

export async function renameTag(old: string, next: string): Promise<{ renamed: number }> {
  if (!isTauri() || useRemote()) return restRenameTag(old, next);
  return invokeCmd<{ renamed: number }>("rename_tag", { old, newTag: next });
}

export async function deleteTag(
  tag: string,
  confirm: boolean
): Promise<{ deleted?: number; confirm_required?: boolean; count?: number }> {
  if (!isTauri() || useRemote()) return restDeleteTag(tag, confirm);
  return invokeCmd("delete_tag", { tag, confirm });
}

export async function renameFolder(old: string, next: string): Promise<{ renamed: number }> {
  if (!isTauri() || useRemote()) return restRenameFolder(old, next);
  return invokeCmd<{ renamed: number }>("rename_folder", { old, newFolder: next });
}

export async function deleteFolder(folder: string): Promise<{ moved_to_root: number }> {
  if (!isTauri() || useRemote()) return restDeleteFolder(folder);
  return invokeCmd<{ moved_to_root: number }>("delete_folder", { folder });
}

export async function fetchRules(): Promise<{ rules: Rule[] }> {
  if (!isTauri() || useRemote()) return restRules();
  return invokeCmd<{ rules: Rule[] }>("list_rules");
}

export async function addRule(input: {
  pattern: string;
  tags?: string[];
  folder?: string;
}): Promise<{ rule: Rule }> {
  if (!isTauri() || useRemote()) return restAddRule(input);
  return invokeCmd("add_rule", {
    pattern: input.pattern,
    tags: input.tags ?? null,
    folder: input.folder ?? null,
  });
}

export async function editRule(
  id: string,
  input: { pattern?: string; tags?: string[]; folder?: string; reapply?: boolean }
): Promise<{ rule: Rule; reapplied: number }> {
  if (!isTauri() || useRemote()) return restEditRule(id, input);
  return invokeCmd("edit_rule", {
    id,
    pattern: input.pattern ?? null,
    tags: input.tags ?? null,
    folder: input.folder ?? null,
    reapply: input.reapply ?? null,
  });
}

export async function deleteRule(id: string): Promise<{ deleted: string }> {
  if (!isTauri() || useRemote()) return restDeleteRule(id);
  return invokeCmd("delete_rule", { id });
}

export async function applyRules(id?: string): Promise<{ applied: number }> {
  if (!isTauri() || useRemote()) return restApplyRules(id);
  return invokeCmd("apply_rules", { id: id ?? null });
}

export async function learnSuggestions(min = 3): Promise<{ suggestions: Suggestion[] }> {
  if (!isTauri() || useRemote()) return restLearn(min);
  return invokeCmd("learn_suggestions", { min });
}

export async function learnCreate(min = 3): Promise<{ created: number; applied: number }> {
  if (!isTauri() || useRemote()) return restLearnCreate(min);
  return invokeCmd("learn_create", { min });
}

export async function fetchHistory(): Promise<BookmarkList> {
  if (!isTauri() || useRemote()) return restHistory();
  return toBookmarkList(await invokeCmd<TauriListResponse>("fetch_history"));
}

export async function fetchSettings(): Promise<Settings> {
  if (!isTauri() || useRemote()) return restSettings();
  return invokeCmd<Settings>("fetch_settings");
}

export async function setSetting(key: string, value: string): Promise<{ ok: boolean }> {
  if (!isTauri() || useRemote()) return restSetSetting(key, value);
  return invokeCmd("set_setting", { key, value });
}

export async function bulkOp(
  ids: string[],
  op: string,
  extra?: { tags?: string[]; folder?: string; confirm?: boolean }
): Promise<BulkResult> {
  if (!isTauri() || useRemote()) return restBulk(ids, op, extra);
  return invokeCmd("bulk_op", {
    ids,
    op,
    tags: extra?.tags ?? null,
    folder: extra?.folder ?? null,
    confirm: extra?.confirm ?? null,
  });
}

export async function checkRun(spec?: string): Promise<CheckReport> {
  if (!isTauri() || useRemote()) return restCheckRun(spec);
  return invokeCmd("check_run", { spec: spec ?? null });
}

export async function checkApply(
  updates: { uuid: string; url: string }[],
  quarantine: string[]
): Promise<{ updated: number; quarantined: number; skipped: unknown[] }> {
  if (!isTauri() || useRemote()) return restCheckApply(updates, quarantine);
  return invokeCmd("check_apply", { updates, quarantine });
}

export async function runReindex(prune: boolean, compact_ids = false): Promise<ReindexReport> {
  if (!isTauri() || useRemote()) return restReindex(prune, compact_ids);
  return invokeCmd("run_reindex", { prune, compact_ids });
}

export async function fetchNotes(uuid: string): Promise<{ body: string | null }> {
  if (!isTauri() || useRemote()) return restNotes(uuid);
  return invokeCmd("fetch_notes", { id: uuid });
}

export async function saveNotes(uuid: string, body: string): Promise<{ ok: boolean }> {
  if (!isTauri() || useRemote()) return restSaveNotes(uuid, body);
  return invokeCmd("save_notes", { id: uuid, body });
}

export async function deleteNotes(uuid: string): Promise<{ ok: boolean }> {
  if (!isTauri() || useRemote()) return restDeleteNotes(uuid);
  return invokeCmd("delete_notes", { id: uuid });
}

export async function fetchArchive(uuid: string): Promise<{ html: string }> {
  if (!isTauri()) throw new Error("fetchArchive is Tauri-only; use archiveUrl on web");
  return invokeCmd("fetch_archive", { id: uuid });
}

export function archiveUrl(uuid: string): string {
  const path = `/api/v2/bookmarks/${uuid}/archive`;
  const base = getRemoteBase();
  return base ? `${base}${path}` : path;
}

export function attachmentUrl(uuid: string, name: string): string {
  const path = `/api/v2/bookmarks/${uuid}/attachments/${encodeURIComponent(name)}`;
  const base = getRemoteBase();
  return base ? `${base}${path}` : path;
}

export function exportBookmarksUrl(): string {
  const path = "/api/v2/library/export-bookmarks";
  const base = getRemoteBase();
  return base ? `${base}${path}` : path;
}

export async function createArchive(
  uuid: string,
  backend?: string
): Promise<{ ok: boolean; warnings: string[] }> {
  if (!isTauri() || useRemote()) return restCreateArchive(uuid, backend);
  return invokeCmd("create_archive", { id: uuid, backend: backend ?? null });
}

export async function deleteArchive(uuid: string): Promise<{ ok: boolean }> {
  if (!isTauri() || useRemote()) return restDeleteArchive(uuid);
  return invokeCmd("delete_archive", { id: uuid });
}

export async function uploadAttachment(
  uuid: string,
  name: string,
  dataUrl: string
): Promise<{ name: string }> {
  if (!isTauri() || useRemote()) return restUpload(uuid, name, dataUrl);
  const base64 = dataUrl.includes(",") ? dataUrl.split(",").slice(1).join(",") : dataUrl;
  return invokeCmd("upload_attachment", { id: uuid, name, content: base64 });
}

export async function deleteAttachment(
  uuid: string,
  name: string
): Promise<{ detached: string }> {
  if (!isTauri() || useRemote()) return restDeleteAttachment(uuid, name);
  return invokeCmd("delete_attachment", { id: uuid, name });
}

export async function downloadAttachment(
  uuid: string,
  name: string
): Promise<{ name: string; mime: string; content: string }> {
  if (!isTauri()) throw new Error("downloadAttachment is Tauri-only; use the download link on web");
  return invokeCmd("download_attachment", { id: uuid, name });
}

export async function openArchiveExternal(uuid: string): Promise<{ opened: boolean }> {
  if (!isTauri()) throw new Error("openArchiveExternal is Tauri-only");
  return invokeCmd("open_archive_external", { id: uuid });
}

export async function openAttachmentExternal(
  uuid: string,
  name: string
): Promise<{ opened: boolean }> {
  if (!isTauri()) throw new Error("openAttachmentExternal is Tauri-only");
  return invokeCmd("open_attachment_external", { id: uuid, name });
}

export async function importLibrary(
  content: string,
  markdown = false
): Promise<{ added: number; skipped_dup: number; skipped_bad: number }> {
  if (!isTauri() || useRemote()) return restImport(content, markdown);
  return invokeCmd("import_library", { content, markdown });
}

export async function exportSite(): Promise<{ index: string }> {
  if (!isTauri() || useRemote()) return restExportSite();
  return invokeCmd("export_site", {});
}

export async function exportBookmarksContent(): Promise<{ content: string }> {
  if (!isTauri()) throw new Error("exportBookmarksContent is Tauri-only; use the download link on web");
  return invokeCmd("export_bookmarks", {});
}

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
  page: number;
  per_page: number;
  bookmarks: Bookmark[];
}

export interface ListParams {
  q?: string;
  sort?: string;
  tag?: string;
  folder?: string;
  page?: number;
  per_page?: number;
}

export interface ExistingBookmark {
  error: string;
  existing: Bookmark;
  hint: string;
}

export class ApiError extends Error {
  status: number;
  body: unknown;
  constructor(status: number, body: unknown) {
    super(typeof body === "object" && body !== null && "error" in body ? String((body as { error: unknown }).error) : `request failed (${status})`);
    this.status = status;
    this.body = body;
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const r = await fetch(path, {
    headers: { "Content-Type": "application/json" },
    ...init,
  });
  if (r.status === 401 && !path.startsWith("/login")) {
    window.location.href = `/login?next=${encodeURIComponent(window.location.pathname)}`;
    throw new ApiError(401, { error: "authentication required" });
  }
  const text = await r.text();
  const body = text ? (JSON.parse(text) as unknown) : null;
  if (!r.ok) throw new ApiError(r.status, body);
  return body as T;
}

export function fetchBookmarks(params: ListParams = {}): Promise<BookmarkList> {
  const q = new URLSearchParams();
  if (params.q) q.set("q", params.q);
  if (params.sort) q.set("sort", params.sort);
  if (params.tag) q.set("tag", params.tag);
  if (params.folder) q.set("folder", params.folder);
  if (params.page) q.set("page", String(params.page));
  if (params.per_page) q.set("per_page", String(params.per_page));
  const suffix = q.toString();
  return request<BookmarkList>(`/api/v2/bookmarks${suffix ? `?${suffix}` : ""}`);
}

export interface AddInput {
  url: string;
  title?: string;
  description?: string;
  tags?: string[];
  folder?: string;
  markdown?: boolean;
  confirm_dup?: boolean;
}

export function addBookmark(input: AddInput): Promise<Bookmark> {
  return request<Bookmark>("/api/v2/bookmarks", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function fetchBookmark(uuid: string): Promise<Bookmark> {
  return request<Bookmark>(`/api/v2/bookmarks/${uuid}`);
}

export interface UpdateInput {
  title?: string;
  description?: string;
  tags?: string[];
  folder?: string;
  url?: string;
}

export function updateBookmark(uuid: string, input: UpdateInput): Promise<Bookmark> {
  return request<Bookmark>(`/api/v2/bookmarks/${uuid}`, {
    method: "PUT",
    body: JSON.stringify(input),
  });
}

export function deleteBookmark(uuid: string, confirm: boolean): Promise<{ deleted?: string; confirm_required?: boolean; bookmark?: Bookmark }> {
  return request(`/api/v2/bookmarks/${uuid}${confirm ? "?confirm=true" : ""}`, {
    method: "DELETE",
  });
}

export function openBookmark(uuid: string): Promise<{ url: string }> {
  return request(`/api/v2/bookmarks/${uuid}/open`, { method: "POST" });
}

export interface TagCount {
  name: string;
  count: number;
}

export interface FolderCount {
  name: string;
  count: number;
}

export function fetchTags(): Promise<{ tags: TagCount[] }> {
  return request("/api/v2/tags");
}

export function fetchFolders(): Promise<{ folders: FolderCount[] }> {
  return request("/api/v2/folders");
}

export function shortUuid(uuid: string): string {
  return uuid.slice(0, 8);
}

export interface BulkResult {
  deleted?: number;
  updated?: number;
  confirm_required?: boolean;
  count?: number;
}

export function bulkOp(ids: string[], op: string, extra?: { tags?: string[]; folder?: string; confirm?: boolean }): Promise<BulkResult> {
  return request<BulkResult>("/api/v2/bulk", {
    method: "POST",
    body: JSON.stringify({ ids, op, ...extra }),
  });
}

export function renameTag(old: string, next: string): Promise<{ renamed: number }> {
  return request("/api/v2/tags/rename", {
    method: "POST",
    body: JSON.stringify({ old, new: next }),
  });
}

export function deleteTag(tag: string, confirm: boolean): Promise<{ deleted?: number; confirm_required?: boolean; count?: number }> {
  return request("/api/v2/tags/delete", {
    method: "POST",
    body: JSON.stringify({ tag, confirm }),
  });
}

export function renameFolder(old: string, next: string): Promise<{ renamed: number }> {
  return request("/api/v2/folders/rename", {
    method: "POST",
    body: JSON.stringify({ old, new: next }),
  });
}

export function deleteFolder(folder: string): Promise<{ moved_to_root: number }> {
  return request("/api/v2/folders/delete", {
    method: "POST",
    body: JSON.stringify({ folder }),
  });
}

export function domainOf(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

export function ageOf(iso: string): string {
  const ms = Date.now() - new Date(iso).getTime();
  const days = Math.floor(ms / 86400000);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months}mo ago`;
  return `${Math.floor(months / 12)}y ago`;
}

export function fetchHistory(): Promise<BookmarkList> {
  return request<BookmarkList>("/api/v2/history?per_page=100");
}

export interface Rule {
  id: string;
  pattern: string;
  tags: string[];
  folder?: string;
  description: string;
  applied_count: number;
}

export function fetchRules(): Promise<{ rules: Rule[] }> {
  return request("/api/v2/rules");
}

export function addRule(input: { pattern: string; tags?: string[]; folder?: string }): Promise<{ rule: Rule }> {
  return request("/api/v2/rules", { method: "POST", body: JSON.stringify(input) });
}

export function editRule(id: string, input: { pattern?: string; tags?: string[]; folder?: string; reapply?: boolean }): Promise<{ rule: Rule; reapplied: number }> {
  return request(`/api/v2/rules/${id}`, { method: "PUT", body: JSON.stringify(input) });
}

export function deleteRule(id: string): Promise<{ deleted: string }> {
  return request(`/api/v2/rules/${id}`, { method: "DELETE" });
}

export function applyRules(id?: string): Promise<{ applied: number }> {
  return request("/api/v2/rules/apply", { method: "POST", body: JSON.stringify({ id }) });
}

export interface Suggestion {
  host: string;
  folder: string;
  count: number;
}

export function learnSuggestions(min = 3): Promise<{ suggestions: Suggestion[] }> {
  return request(`/api/v2/rules/learn?min=${min}`);
}

export function learnCreate(min = 3): Promise<{ created: number; applied: number }> {
  return request("/api/v2/rules/learn", { method: "POST", body: JSON.stringify({ min }) });
}

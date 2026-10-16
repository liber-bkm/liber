// Shared liber API client, no DOM. Works in popup, options, and background.
"use strict";

const browserApi =
  typeof browser !== "undefined" && browser.storage
    ? browser
    : typeof chrome !== "undefined"
      ? chrome
      : undefined;

async function getSettings() {
  const { baseUrl = "", token = "" } = await browserApi.storage.local.get([
    "baseUrl",
    "token",
  ]);
  return { baseUrl: baseUrl.replace(/\/+$/, ""), token };
}

async function deriveBearer(token) {
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(token),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const sig = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode("liber-bearer-v1")
  );
  return Array.from(new Uint8Array(sig))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

class LiberError extends Error {
  constructor(status, body) {
    super(
      body && typeof body === "object" && "error" in body
        ? String(body.error)
        : `request failed (${status})`
    );
    this.status = status;
    this.body = body;
  }
}

async function request(path, init) {
  const { baseUrl, token } = await getSettings();
  if (!baseUrl) {
    throw new LiberError(0, {
      error: "no server configured (open extension options)",
    });
  }
  const headers = { "Content-Type": "application/json" };
  if (token) headers["Authorization"] = `Bearer ${await deriveBearer(token)}`;
  let res;
  try {
    res = await fetch(`${baseUrl}${path}`, { headers, ...init });
  } catch (e) {
    throw new LiberError(0, {
      error: `cannot reach ${baseUrl} (is liber serve running?)`,
    });
  }
  const text = await res.text();
  const body = text ? JSON.parse(text) : null;
  if (!res.ok) throw new LiberError(res.status, body);
  return body;
}

async function testConnection() {
  return request("/api/v2/bookmarks?per_page=1", {});
}

async function addBookmark(input) {
  return request("/api/v2/bookmarks", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

async function fetchTags() {
  return request("/api/v2/tags", {});
}

async function fetchFolders() {
  return request("/api/v2/folders", {});
}

function splitTags(line) {
  return line.split(/[\s,]+/).filter(Boolean);
}

async function fileToAttachment(file) {
  const MAX_BYTES = 24 * 1024 * 1024;
  if (file.size > MAX_BYTES) {
    throw new Error(`${file.name} is over the attachment size limit`);
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  let bin = "";
  const CHUNK = 8192;
  for (let i = 0; i < bytes.length; i += CHUNK) {
    bin += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  }
  return { name: file.name, content: btoa(bin) };
}

if (typeof module !== "undefined") {
  module.exports = { splitTags, LiberError };
}

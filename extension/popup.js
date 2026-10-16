// Popup controller: prefill from the active tab, save through liber.js.
"use strict";

const $ = (id) => document.getElementById(id);

function showError(message) {
  $("error").textContent = message;
}

async function activeTab() {
  const tabs = await browserApi.tabs.query({ active: true, currentWindow: true });
  return tabs[0];
}

async function fillDatalists() {
  try {
    const { tags } = await fetchTags();
    $("tag-list").innerHTML = (tags || [])
      .map((t) => `<option value="${t.name.replace(/"/g, "&quot;")}">`)
      .join("");
  } catch {
    /* suggestions are best effort */
  }
  try {
    const { folders } = await fetchFolders();
    $("folder-list").innerHTML = (folders || [])
      .map((f) => `<option value="${f.name.replace(/"/g, "&quot;")}">`)
      .join("");
  } catch {
    /* suggestions are best effort */
  }
}

function collect(confirmed) {
  return {
    url: $("url").value.trim(),
    title: $("title").value.trim() || undefined,
    tags: splitTags($("tags").value),
    folder: $("folder").value.trim() || undefined,
    markdown: $("markdown").checked || undefined,
    archive: $("archive").checked || undefined,
    confirm_dup: confirmed || undefined,
  };
}

async function collectAttachments() {
  const files = Array.from($("files").files || []);
  const out = [];
  for (const f of files) {
    out.push(await fileToAttachment(f));
  }
  return out;
}

function showDone(title, warnings) {
  $("form").classList.add("hidden");
  $("duplicate").classList.add("hidden");
  $("done-text").textContent = `Saved: ${title}`;
  $("done-warnings").textContent = (warnings || []).join("\n");
  $("done").classList.remove("hidden");
}

function showDuplicate(existing) {
  $("form").classList.add("hidden");
  $("dup-text").textContent = `Already in your library as ${existing.title}.`;
  $("duplicate").classList.remove("hidden");
  $("add-anyway").onclick = async () => {
    await save(true);
  };
  $("open-existing").onclick = async () => {
    const { baseUrl } = await getSettings();
    await browserApi.tabs.create({ url: baseUrl });
    window.close();
  };
}

async function save(confirmed) {
  showError("");
  $("warnings").textContent = "";
  $("save").disabled = true;
  try {
    const input = collect(confirmed);
    if (!input.url) throw new Error("no URL on this tab");
    input.attachments = await collectAttachments();
    if (input.attachments.length === 0) delete input.attachments;
    const saved = await addBookmark(input);
    showDone(saved.title || saved.url, saved.warnings);
  } catch (e) {
    if (e instanceof LiberError && e.status === 409 && e.body && e.body.existing) {
      showDuplicate(e.body.existing);
      return;
    }
    showError(e instanceof Error ? e.message : "save failed");
  } finally {
    $("save").disabled = false;
  }
}

async function openLibrary() {
  const { baseUrl } = await getSettings();
  if (baseUrl) await browserApi.tabs.create({ url: baseUrl });
  window.close();
}

document.addEventListener("DOMContentLoaded", async () => {
  try {
    const tab = await activeTab();
    if (tab && tab.url && !tab.url.startsWith("about:")) {
      $("url").value = tab.url;
      $("title").value = tab.title || "";
    }
  } catch {
    /* prefill is best effort */
  }
  await fillDatalists();
  $("save").onclick = () => save(false);
  $("open-library").onclick = openLibrary;
});

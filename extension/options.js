// Options page: server URL plus token in chrome.storage.local only.
"use strict";

const $ = (id) => document.getElementById(id);

function showError(message) {
  $("error").textContent = message;
  $("ok").textContent = "";
}

function showOk(message) {
  $("ok").textContent = message;
  $("error").textContent = "";
}

document.addEventListener("DOMContentLoaded", async () => {
  const { baseUrl, token } = await getSettings();
  $("baseUrl").value = baseUrl;
  $("token").value = token;

  async function persist() {
    await browserApi.storage.local.set({
      baseUrl: $("baseUrl").value.trim(),
      token: $("token").value,
    });
    if (!$("token").value) {
      await browserApi.storage.local.remove("token");
    }
    try {
      await browserApi.permissions.request({
        origins: [$("baseUrl").value.trim().replace(/\/+$/, "") + "/*"],
      });
    } catch {
      /* optional host grant is best effort */
    }
  }

  $("save").onclick = async () => {
    await persist();
    showOk("Saved.");
  };

  $("test").onclick = async () => {
    showError("");
    try {
      await persist();
      await testConnection();
      showOk("Connected.");
    } catch (e) {
      showError(e instanceof Error ? e.message : "connection failed");
    }
  };
});

// Context menu capture: save the page or link without opening the popup.
"use strict";

importScripts("liber.js");

const MENU_PAGE = "liber-save-page";
const MENU_LINK = "liber-save-link";

async function ensureMenus() {
  await browserApi.contextMenus.removeAll();
  browserApi.contextMenus.create({
    id: MENU_PAGE,
    title: "Save to liber",
    contexts: ["page"],
  });
  browserApi.contextMenus.create({
    id: MENU_LINK,
    title: "Save link to liber",
    contexts: ["link"],
  });
}

browserApi.runtime.onInstalled.addListener(() => {
  ensureMenus();
});
ensureMenus();

browserApi.contextMenus.onClicked.addListener(async (info, tab) => {
  const link = info.menuItemId === MENU_LINK;
  const url = link ? info.linkUrl : tab && tab.url;
  if (!url) return;
  try {
    await addBookmark({
      url,
      title: !link && tab && tab.title ? tab.title : undefined,
    });
  } catch (e) {
    console.warn("liber save failed:", e instanceof Error ? e.message : e);
  }
});

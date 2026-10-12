import { useEffect, useRef, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ThemeProvider } from "./theme";
import { Layout, type View } from "./components/Layout";
import { Library } from "./screens/Library";
import { Login } from "./screens/Login";
import { AddDialog, DetailDrawer } from "./components/Detail";
import { isTauri, subscribeSharedUrls } from "./tauri";
import { FoldersPage, TagsPage } from "./screens/Taxonomy";
import { RulesPage } from "./screens/Rules";
import { CheckPage } from "./screens/Check";
import { HistoryPage } from "./screens/History";
import { SettingsPage } from "./screens/Settings";
import { Palette } from "./components/Palette";
import type { Bookmark } from "./api";
import "./index.css";

const qc = new QueryClient();

export function parseAddQuery(search: string): { url: string; title: string; tags: string; folder: string } | null {
  try {
    const params = new URLSearchParams(search);
    const add = params.get("add");
    if (!add) return null;
    return {
      url: add,
      title: params.get("title") ?? "",
      tags: params.get("tags") ?? "",
      folder: params.get("folder") ?? "",
    };
  } catch {
    return null;
  }
}

function Shell() {
  const [view, setView] = useState<View>("library");
  const [folder, setFolder] = useState<string | null>(null);
  const [tag, setTag] = useState<string | null>(null);
  const [openUuid, setOpenUuid] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [palette, setPalette] = useState(false);
  const [version, setVersion] = useState(0);
  const [shareUrl, setShareUrl] = useState<string | null>(null);
  const [shareTitle, setShareTitle] = useState("");
  const [shareTags, setShareTags] = useState("");
  const [shareFolder, setShareFolder] = useState("");

  function clearShare() {
    setShareUrl(null);
    setShareTitle("");
    setShareTags("");
    setShareFolder("");
  }

  useEffect(() => {
    let off: (() => void) | undefined;
    (async () => {
      off = await subscribeSharedUrls((u) => {
        setShareUrl(u);
        setAdding(true);
      });
    })();
    try {
      const parsed = parseAddQuery(window.location.search);
      if (parsed) {
        setShareUrl(parsed.url);
        setShareTitle(parsed.title);
        setShareTags(parsed.tags);
        setShareFolder(parsed.folder);
        setAdding(true);
        window.history.replaceState(null, "", window.location.pathname);
      }
    } catch {
      /* leave dialog closed on malformed query */
    }
    return () => off?.();
  }, []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette((p) => !p);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const suppressPush = useRef(false);
  const mountedNav = useRef(false);
  const prevNav = useRef({ view, o: false, a: false, p: false });
  const live = useRef({ view, folder, tag, openUuid, adding, palette });
  live.current = { view, folder, tag, openUuid, adding, palette };
  const navKey = JSON.stringify({
    view,
    o: !!openUuid,
    a: adding,
    p: palette,
  });

  useEffect(() => {
    if (!isTauri()) return;
    if (!mountedNav.current) {
      mountedNav.current = true;
      return;
    }
    if (suppressPush.current) {
      suppressPush.current = false;
      prevNav.current = { view, o: !!openUuid, a: adding, p: palette };
      return;
    }
    const prev = prevNav.current;
    const opened = (!prev.o && !!openUuid) || (!prev.a && adding) || (!prev.p && palette);
    prevNav.current = { view, o: !!openUuid, a: adding, p: palette };
    if (prev.view !== view || opened) history.pushState({ tag: "liber-nav" }, "");
  }, [navKey]);

  useEffect(() => {
    if (!isTauri()) return;
    history.replaceState({ tag: "liber-base" }, "");
    const onPop = () => {
      const s = live.current;
      if (s.palette) {
        suppressPush.current = true;
        setPalette(false);
      } else if (s.adding) {
        suppressPush.current = true;
        setAdding(false);
        clearShare();
      } else if (s.openUuid) {
        suppressPush.current = true;
        setOpenUuid(null);
      } else if (s.view !== "library" || s.folder || s.tag) {
        suppressPush.current = true;
        setView("library");
        setFolder(null);
        setTag(null);
      } else {
        suppressPush.current = false;
      }
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);

  return (
    <Layout
      view={view}
      setView={setView}
      onAdd={() => setAdding(true)}
      folder={folder}
      setFolder={setFolder}
      tag={tag}
      setTag={setTag}
    >
      {view === "library" && (
        <Library
          key={version}
          folder={folder}
          tag={tag}
          onOpen={(b: Bookmark) => setOpenUuid(b.uuid)}
        />
      )}
      {view === "history" && <HistoryPage onOpen={(b: Bookmark) => setOpenUuid(b.uuid)} />}
      {view === "tags" && <TagsPage />}
      {view === "folders" && <FoldersPage />}
      {view === "rules" && <RulesPage />}
      {view === "check" && <CheckPage />}
      {view === "settings" && <SettingsPage />}
      {palette && (
        <Palette
          onOpen={(b: Bookmark) => setOpenUuid(b.uuid)}
          onAdd={() => setAdding(true)}
          onNavigate={(v) => setView(v as View)}
          onClose={() => setPalette(false)}
        />
      )}
      {openUuid && (
        <DetailDrawer
          uuid={openUuid}
          onClose={() => setOpenUuid(null)}
          onChanged={() => setVersion((v) => v + 1)}
        />
      )}
      {adding && (
        <AddDialog
          key={`${shareUrl ?? "add"}|${shareTitle}|${shareTags}|${shareFolder}`}
          initialUrl={shareUrl ?? ""}
          initialTitle={shareTitle}
          initialTags={shareTags}
          initialFolder={shareFolder}
          onClose={() => {
            setAdding(false);
            clearShare();
          }}
          onAdded={(b) => {
            setAdding(false);
            clearShare();
            setOpenUuid(b.uuid);
            setVersion((v) => v + 1);
          }}
        />
      )}
    </Layout>
  );
}

function LoggedIn() {
  const [authed, setAuthed] = useState<boolean | null>(null);

  useEffect(() => {
    fetch("/api/v2/bookmarks?per_page=1")
      .then((r) => setAuthed(r.status !== 401))
      .catch(() => setAuthed(true));
  }, []);

  if (authed === null) return null;
  if (!authed && window.location.pathname !== "/login") {
    const next = window.location.pathname + window.location.search;
    window.location.href = `/login?next=${encodeURIComponent(next)}`;
    return null;
  }
  return <Shell />;
}

export default function App() {
  if (window.location.pathname === "/login") return <Login />;
  return (
    <QueryClientProvider client={qc}>
      <ThemeProvider>
        <LoggedIn />
      </ThemeProvider>
    </QueryClientProvider>
  );
}

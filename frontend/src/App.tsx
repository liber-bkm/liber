import { useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ThemeProvider } from "./theme";
import { Layout, type View } from "./components/Layout";
import { Library } from "./screens/Library";
import { Login } from "./screens/Login";
import { AddDialog, DetailDrawer } from "./components/Detail";
import { FoldersPage, TagsPage } from "./screens/Taxonomy";
import { RulesPage } from "./screens/Rules";
import { CheckPage } from "./screens/Check";
import { HistoryPage } from "./screens/History";
import { SettingsPage } from "./screens/Settings";
import { Palette } from "./components/Palette";
import type { Bookmark } from "./api";
import "./index.css";

const qc = new QueryClient();

function Shell() {
  const [view, setView] = useState<View>("library");
  const [folder, setFolder] = useState<string | null>(null);
  const [tag, setTag] = useState<string | null>(null);
  const [openUuid, setOpenUuid] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [palette, setPalette] = useState(false);
  const [version, setVersion] = useState(0);

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
          onClose={() => setAdding(false)}
          onAdded={(b) => {
            setAdding(false);
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
    window.location.href = `/login?next=${encodeURIComponent(window.location.pathname)}`;
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

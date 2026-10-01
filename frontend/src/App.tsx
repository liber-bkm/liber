import { useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ThemeProvider } from "./theme";
import { Layout, type View } from "./components/Layout";
import { Library } from "./screens/Library";
import { FoldersPage, TagsPage } from "./screens/Taxonomy";
import { Login } from "./screens/Login";
import { AddDialog, DetailDrawer } from "./components/Detail";
import { Empty } from "./components/ui";
import type { Bookmark } from "./api";
import "./index.css";

const qc = new QueryClient();

function Shell() {
  const [view, setView] = useState<View>("library");
  const [folder, setFolder] = useState<string | null>(null);
  const [tag, setTag] = useState<string | null>(null);
  const [openUuid, setOpenUuid] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [version, setVersion] = useState(0);

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
      {view === "history" && (
        <Empty title="History" hint="Recently opened bookmarks will appear here." />
      )}
      {view === "tags" && <TagsPage />}
      {view === "folders" && <FoldersPage />}
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

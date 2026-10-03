import { useQuery } from "@tanstack/react-query";
import { BookOpen, CheckCircle2, Clock3, FolderTree, Moon, Plus, Settings as SettingsIcon, Sparkles, Sun, Tags } from "lucide-react";
import { fetchFolders, fetchTags } from "../tauri";
import { useTheme } from "../theme";

export type View = "library" | "history" | "tags" | "folders" | "rules" | "check" | "settings";

export function Layout({
  view,
  setView,
  onAdd,
  folder,
  setFolder,
  tag,
  setTag,
  children,
}: {
  view: View;
  setView: (v: View) => void;
  onAdd: () => void;
  folder: string | null;
  setFolder: (f: string | null) => void;
  tag: string | null;
  setTag: (t: string | null) => void;
  children: React.ReactNode;
}) {
  const { effective, setTheme } = useTheme();
  const tags = useQuery({ queryKey: ["tags"], queryFn: fetchTags });
  const folders = useQuery({ queryKey: ["folders"], queryFn: fetchFolders });

  return (
    <div className="flex min-h-screen bg-neutral-50 text-neutral-900 dark:bg-neutral-950 dark:text-neutral-100">
      <aside className="hidden w-60 shrink-0 flex-col gap-1 border-r border-neutral-200 bg-white p-3 dark:border-neutral-800 dark:bg-neutral-900 md:flex">
        <div className="mb-2 flex items-center justify-between px-2">
          <span className="text-base font-semibold tracking-tight">liber</span>
          <button
            onClick={() => setTheme(effective === "dark" ? "light" : "dark")}
            className="rounded-lg p-1.5 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
            title="Toggle theme"
          >
            {effective === "dark" ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
          </button>
        </div>
        <button
          onClick={onAdd}
          className="mb-2 inline-flex items-center justify-center gap-1.5 rounded-lg bg-accent-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-accent-700"
        >
          <Plus className="h-4 w-4" /> Add bookmark
        </button>
        <NavItem active={view === "library" && !folder && !tag} onClick={() => { setView("library"); setFolder(null); setTag(null); }} icon={<BookOpen className="h-4 w-4" />} label="Library" />
        <NavItem active={view === "tags"} onClick={() => setView("tags")} icon={<Tags className="h-4 w-4" />} label="Tags" />
        <NavItem active={view === "folders"} onClick={() => setView("folders")} icon={<FolderTree className="h-4 w-4" />} label="Folders" />
        <NavItem active={view === "rules"} onClick={() => setView("rules")} icon={<Sparkles className="h-4 w-4" />} label="Rules" />
        <NavItem active={view === "check"} onClick={() => setView("check")} icon={<CheckCircle2 className="h-4 w-4" />} label="Check" />
        <NavItem active={view === "history"} onClick={() => setView("history")} icon={<Clock3 className="h-4 w-4" />} label="History" />
        <NavItem active={view === "settings"} onClick={() => setView("settings")} icon={<SettingsIcon className="h-4 w-4" />} label="Settings" />
        <SectionTitle icon={<FolderTree className="h-3.5 w-3.5" />} label="Folders" />
        {(folders.data?.folders ?? []).map((f) => (
          <FilterRow
            key={f.name}
            active={folder === f.name}
            label={f.name}
            count={f.count}
            onClick={() => { setFolder(folder === f.name ? null : f.name); setView("library"); }}
          />
        ))}
        <SectionTitle icon={<Tags className="h-3.5 w-3.5" />} label="Tags" />
        <div className="flex flex-wrap gap-1 px-2">
          {(tags.data?.tags ?? []).slice(0, 30).map((t) => (
            <button
              key={t.name}
              onClick={() => { setTag(tag === t.name ? null : t.name); setView("library"); }}
              className={`rounded-full px-2 py-0.5 text-xs ${tag === t.name ? "bg-accent-600 text-white" : "bg-neutral-100 text-neutral-600 hover:bg-neutral-200 dark:bg-neutral-800 dark:text-neutral-300"}`}
            >
              #{t.name}
            </button>
          ))}
        </div>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center justify-between border-b border-neutral-200 bg-white px-4 py-2 dark:border-neutral-800 dark:bg-neutral-900 md:hidden">
          <span className="font-semibold">liber</span>
          <button onClick={onAdd} className="inline-flex items-center gap-1 rounded-lg bg-accent-600 px-3 py-1.5 text-sm font-medium text-white">
            <Plus className="h-4 w-4" /> Add
          </button>
        </header>
        <main className="mx-auto w-full max-w-5xl flex-1 px-4 py-4">{children}</main>
      </div>
    </div>
  );
}

function NavItem({ active, onClick, icon, label }: { active: boolean; onClick: () => void; icon: React.ReactNode; label: string }) {
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-2 rounded-lg px-2 py-1.5 text-sm ${active ? "bg-accent-50 font-medium text-accent-700 dark:bg-accent-700/20 dark:text-accent-100" : "text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"}`}
    >
      {icon}
      {label}
    </button>
  );
}

function SectionTitle({ icon, label }: { icon: React.ReactNode; label: string }) {
  return (
    <div className="mt-3 flex items-center gap-1.5 px-2 text-xs font-medium uppercase tracking-wide text-neutral-400">
      {icon}
      {label}
    </div>
  );
}

function FilterRow({ active, label, count, onClick }: { active: boolean; label: string; count: number; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`flex items-center justify-between rounded-lg px-2 py-1 text-sm ${active ? "bg-accent-50 font-medium text-accent-700 dark:bg-accent-700/20 dark:text-accent-100" : "text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"}`}
    >
      <span className="truncate">{label}</span>
      <span className="text-xs text-neutral-400">{count}</span>
    </button>
  );
}

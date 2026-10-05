import { useEffect, useState } from "react";
import { Command } from "cmdk";
import { useQuery } from "@tanstack/react-query";
import { type Bookmark } from "../api";
import { listBookmarks } from "../tauri";

export function Palette({
  onOpen,
  onAdd,
  onNavigate,
  onClose,
}: {
  onOpen: (b: Bookmark) => void;
  onAdd: () => void;
  onNavigate: (view: string) => void;
  onClose: () => void;
}) {
  const [q, setQ] = useState("");
  const results = useQuery({
    queryKey: ["palette", q],
    queryFn: () => listBookmarks({ q: q || undefined, per_page: 8 }),
    enabled: q.trim().length > 0,
  });

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pb-4 pt-[max(15vh,env(safe-area-inset-top))]" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="w-full max-w-lg overflow-hidden rounded-2xl bg-white shadow-xl dark:bg-neutral-900">
        <Command label="Command palette" className="[&_[cmdk-input]]:w-full">
          <div className="border-b border-neutral-200 dark:border-neutral-800">
            <Command.Input
              value={q}
              onValueChange={setQ}
              placeholder="Search bookmarks or type an action..."
              autoFocus
              className="w-full bg-transparent px-4 py-3 text-sm outline-none placeholder:text-neutral-400"
            />
          </div>
          <Command.List className="max-h-80 overflow-y-auto p-2">
            <Command.Empty className="px-3 py-4 text-sm text-neutral-400">No results.</Command.Empty>
            <Command.Group heading="Actions">
              <Item onSelect={() => { onAdd(); onClose(); }}>Add bookmark</Item>
              {["library", "tags", "folders", "rules", "check", "history", "settings"].map((v) => (
                <Item
                  key={v}
                  onSelect={() => {
                    onNavigate(v);
                    onClose();
                  }}
                >
                  Go to {v}
                </Item>
              ))}
            </Command.Group>
            {(results.data?.bookmarks ?? []).length > 0 && (
              <Command.Group heading="Bookmarks">
                {(results.data?.bookmarks ?? []).map((b) => (
                  <Item
                    key={b.uuid}
                    onSelect={() => {
                      onOpen(b);
                      onClose();
                    }}
                  >
                    {b.title}
                  </Item>
                ))}
              </Command.Group>
            )}
          </Command.List>
        </Command>
      </div>
    </div>
  );
}

function Item({ children, onSelect }: { children: React.ReactNode; onSelect: () => void }) {
  return (
    <Command.Item
      onSelect={onSelect}
      className="cursor-pointer rounded-lg px-3 py-1.5 text-sm aria-selected:bg-neutral-100 dark:aria-selected:bg-neutral-800"
    >
      {children}
    </Command.Item>
  );
}

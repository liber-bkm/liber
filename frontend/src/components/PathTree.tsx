import { useEffect, useMemo, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import {
  buildPathTree,
  pathAncestors,
  type PathCount,
  type PathNode,
} from "../tree";

export interface PathTreeProps {
  items: PathCount[];
  active?: string | null;
  onSelect?: (path: string) => void;
  storageKey?: string;
  labelPrefix?: string;
  rootLabel?: string;
  caseInsensitive?: boolean;
  sort?: "alpha" | "count";
  empty?: React.ReactNode;
  renderActions?: (node: PathNode) => React.ReactNode;
  className?: string;
}

function readExpanded(key?: string): Set<string> {
  if (!key) return new Set();
  try {
    const parsed = JSON.parse(localStorage.getItem(key) ?? "[]");
    return new Set(
      Array.isArray(parsed)
        ? parsed.filter((value): value is string => typeof value === "string")
        : []
    );
  } catch {
    return new Set();
  }
}

export function PathTree({
  items,
  active = null,
  onSelect,
  storageKey,
  labelPrefix = "",
  rootLabel = "Unfiled",
  caseInsensitive = false,
  sort = "alpha",
  empty,
  renderActions,
  className,
}: PathTreeProps) {
  const [expanded, setExpanded] = useState<Set<string>>(() => readExpanded(storageKey));
  const nodes = useMemo(
    () => buildPathTree(items, { caseInsensitive, rootLabel, sort }),
    [items, caseInsensitive, rootLabel, sort]
  );

  useEffect(() => {
    if (!storageKey) return;
    try {
      localStorage.setItem(storageKey, JSON.stringify([...expanded]));
    } catch {
      /* storage can be unavailable in private contexts */
    }
  }, [expanded, storageKey]);

  useEffect(() => {
    if (!active) return;
    const ancestors = pathAncestors(active);
    if (ancestors.length === 0) return;
    setExpanded((prev) => {
      let changed = false;
      const next = new Set(prev);
      for (const path of ancestors) {
        if (!next.has(path)) {
          next.add(path);
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [active]);

  const toggle = (path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  if (nodes.length === 0) return <>{empty ?? null}</>;

  const row = (node: PathNode, depth: number): React.ReactNode => {
    const hasChildren = node.children.length > 0;
    const open = expanded.has(node.path);
    const selected = active === node.path;
    return (
      <div key={node.path}>
        <div
          className={`flex items-center gap-1 rounded-lg py-1 pr-1 text-sm ${
            selected
              ? "bg-accent-50 font-medium text-accent-700 dark:bg-accent-700/20 dark:text-accent-100"
              : "text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
          }`}
          style={{ paddingLeft: 4 + depth * 12 }}
        >
          {hasChildren ? (
            <button
              type="button"
              onClick={() => toggle(node.path)}
              aria-expanded={open}
              aria-label={`${open ? "Collapse" : "Expand"} ${node.label}`}
              className="rounded p-0.5 text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-200"
            >
              {open ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
            </button>
          ) : (
            <span className="w-[18px]" />
          )}
          {onSelect ? (
            <button
              type="button"
              onClick={() => onSelect(node.path)}
              className="flex min-w-0 flex-1 items-center justify-between text-left"
              title={node.isRoot ? "Bookmarks outside any folder" : node.path}
            >
              <span className="truncate">
                {node.isRoot ? node.label : `${labelPrefix}${node.label}`}
              </span>
              <span
                className="ml-2 shrink-0 text-xs font-normal text-neutral-400"
                title={`${node.total} bookmark${node.total === 1 ? "" : "s"} including sub-items (${node.own} direct)`}
              >
                {node.total}
              </span>
            </button>
          ) : (
            <>
              <span className="min-w-0 flex-1 truncate" title={node.isRoot ? "Bookmarks outside any folder" : node.path}>
                {node.isRoot ? node.label : `${labelPrefix}${node.label}`}
              </span>
              <span
                className="shrink-0 text-xs text-neutral-400"
                title={`${node.total} bookmark${node.total === 1 ? "" : "s"} including sub-items (${node.own} direct)`}
              >
                {node.total}
              </span>
            </>
          )}
          {renderActions && (
            <span className="ml-1 flex shrink-0 items-center gap-0.5">
              {renderActions(node)}
            </span>
          )}
        </div>
        {hasChildren &&
          open &&
          node.children.map((child) => row(child, depth + 1))}
      </div>
    );
  };

  return <div className={className}>{nodes.map((node) => row(node, 0))}</div>;
}

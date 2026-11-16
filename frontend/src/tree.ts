export interface PathCount {
  name: string;
  count: number;
}

export interface PathNode {
  path: string;
  label: string;
  own: number;
  total: number;
  children: PathNode[];
  isRoot?: boolean;
}

export interface PathTreeOptions {
  separator?: string;
  caseInsensitive?: boolean;
  rootLabel?: string;
  sort?: "alpha" | "count";
}

/**
 * Builds a nested tree from slash-separated names. Missing parents are
 * synthesized and every node carries its own and aggregated bookmark counts.
 */
export function buildPathTree(
  items: PathCount[],
  options: PathTreeOptions = {}
): PathNode[] {
  const separator = options.separator ?? "/";
  const caseInsensitive = options.caseInsensitive ?? false;
  const rootLabel = options.rootLabel ?? "Unfiled";
  const sortMode = options.sort ?? "alpha";
  const roots: PathNode[] = [];
  const index = new Map<string, PathNode>();
  const keyOf = (path: string) => (caseInsensitive ? path.toLowerCase() : path);

  for (const item of items) {
    const name = item.name.trim();
    if (name === "/" || name === "") {
      let root = index.get("/");
      if (!root) {
        root = { path: "/", label: rootLabel, own: 0, total: 0, children: [], isRoot: true };
        index.set("/", root);
        roots.push(root);
      }
      root.own += item.count;
      continue;
    }

    const parts = name.split(separator).map((part) => part.trim()).filter(Boolean);
    if (parts.length === 0) continue;

    let parent: PathNode | null = null;
    for (const part of parts) {
      const candidate: string = parent ? `${parent.path}${separator}${part}` : part;
      const key = keyOf(candidate);
      let node: PathNode | undefined = index.get(key);
      if (!node) {
        node = { path: candidate, label: part, own: 0, total: 0, children: [] };
        index.set(key, node);
        (parent ? parent.children : roots).push(node);
      }
      parent = node;
    }
    if (parent) parent.own += item.count;
  }

  const finalize = (nodes: PathNode[]): number => {
    for (const node of nodes) {
      node.total = node.own + finalize(node.children);
    }
    nodes.sort((a, b) => {
      if (sortMode === "count" && a.total !== b.total) return b.total - a.total;
      return a.label.localeCompare(b.label);
    });
    return nodes.reduce((sum, node) => sum + node.total, 0);
  };
  finalize(roots);
  return roots;
}

export function flattenPathTree(nodes: PathNode[]): PathNode[] {
  const out: PathNode[] = [];
  const walk = (items: PathNode[]) => {
    for (const node of items) {
      out.push(node);
      walk(node.children);
    }
  };
  walk(nodes);
  return out;
}

export function pathAncestors(path: string, separator = "/"): string[] {
  const parts = path.split(separator).filter(Boolean);
  const out: string[] = [];
  let current = "";
  for (const part of parts) {
    current = current ? `${current}${separator}${part}` : part;
    out.push(current);
  }
  return out;
}

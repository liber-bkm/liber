import { describe, expect, it } from "vitest";
import { buildPathTree, flattenPathTree, pathAncestors } from "./tree";

describe("buildPathTree", () => {
  it("synthesizes parents and aggregates counts", () => {
    const roots = buildPathTree([
      { name: "tech", count: 1 },
      { name: "tech/rust", count: 2 },
      { name: "tech/rust/async", count: 1 },
      { name: "home", count: 3 },
    ]);

    expect(roots.map((node) => node.path)).toEqual(["home", "tech"]);
    const tech = roots.find((node) => node.path === "tech");
    expect(tech?.own).toBe(1);
    expect(tech?.total).toBe(4);
    const rust = tech?.children[0];
    expect(rust?.path).toBe("tech/rust");
    expect(rust?.own).toBe(2);
    expect(rust?.total).toBe(3);
    expect(rust?.children[0].total).toBe(1);
  });

  it("merges case variants when asked", () => {
    const roots = buildPathTree(
      [
        { name: "Dev", count: 1 },
        { name: "dev/rust", count: 2 },
      ],
      { caseInsensitive: true }
    );
    expect(roots).toHaveLength(1);
    expect(roots[0].path).toBe("Dev");
    expect(roots[0].total).toBe(3);
    expect(roots[0].children[0].path).toBe("Dev/rust");
  });

  it("keeps the folder root separate", () => {
    const roots = buildPathTree([
      { name: "/", count: 2 },
      { name: "tech", count: 1 },
    ]);
    const root = roots.find((node) => node.isRoot);
    expect(root?.path).toBe("/");
    expect(root?.label).toBe("Unfiled");
    expect(root?.total).toBe(2);
  });

  it("flattens and lists ancestors", () => {
    const roots = buildPathTree([
      { name: "dev", count: 1 },
      { name: "dev/rust", count: 1 },
    ]);
    expect(flattenPathTree(roots).map((node) => node.path)).toEqual([
      "dev",
      "dev/rust",
    ]);
    expect(pathAncestors("dev/rust/async")).toEqual([
      "dev",
      "dev/rust",
      "dev/rust/async",
    ]);
  });
});

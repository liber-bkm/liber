import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PathTree } from "./components/PathTree";

afterEach(cleanup);

const items = [
  { name: "tech", count: 1 },
  { name: "tech/rust", count: 2 },
  { name: "home", count: 3 },
];

describe("PathTree", () => {
  it("expands and collapses nested folders", () => {
    render(<PathTree items={items} onSelect={() => {}} />);
    expect(screen.queryByText("rust")).not.toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("Expand tech"));
    expect(screen.getByText("rust")).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("Collapse tech"));
    expect(screen.queryByText("rust")).not.toBeInTheDocument();
  });

  it("selects the full path", () => {
    const onSelect = vi.fn();
    render(<PathTree items={items} onSelect={onSelect} />);
    fireEvent.click(screen.getByText("home"));
    expect(onSelect).toHaveBeenCalledWith("home");
  });

  it("auto-expands the active path", async () => {
    render(<PathTree items={items} active="tech/rust" onSelect={() => {}} />);
    expect(await screen.findByText("rust")).toBeInTheDocument();
  });
});

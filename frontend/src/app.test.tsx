import { describe, expect, it } from "vitest";
import { parseAddQuery } from "./App";

describe("bookmarklet prefill", () => {
  it("parses add plus title tags folder", () => {
    const r = parseAddQuery("?add=https%3A%2F%2Fexample.com%2Fx&title=Hello&tags=a+b&folder=read");
    expect(r).toEqual({ url: "https://example.com/x", title: "Hello", tags: "a b", folder: "read" });
  });

  it("returns null without add", () => {
    expect(parseAddQuery("?title=Hello")).toBeNull();
    expect(parseAddQuery("")).toBeNull();
  });

  it("defaults missing fields to empty", () => {
    expect(parseAddQuery("?add=https%3A%2F%2Fexample.com")).toEqual({
      url: "https://example.com",
      title: "",
      tags: "",
      folder: "",
    });
  });
});

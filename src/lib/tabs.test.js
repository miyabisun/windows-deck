import { describe, expect, it } from "vitest";
import { buttonsFor } from "./tabs.js";

const buttons = [
  { id: "output", desktop: null },
  { id: "sf6-volume", desktop: "D65417D3-28D1-4D1A-8671-F07FD9BD3B45" },
  { id: "blue-archive", desktop: "F2E0C498-C132-42A1-9301-B151850E5D50" },
];
const desktops = (current) =>
  [
    "79B71DC8-820C-433A-BE92-8A07A79B68DB",
    "D65417D3-28D1-4D1A-8671-F07FD9BD3B45",
    "F2E0C498-C132-42A1-9301-B151850E5D50",
  ].map((id, index) => ({ id, name: `d${index}`, index, current: index === current }));

describe("buttonsFor", () => {
  it("shows the shared buttons and those bound to the current desktop", () => {
    expect(buttonsFor(buttons, desktops(0)).map((b) => b.id)).toEqual(["output"]);
    expect(buttonsFor(buttons, desktops(1)).map((b) => b.id)).toEqual(["output", "sf6-volume"]);
  });

  it("matches desktop ids without regard to case", () => {
    const lower = [{ id: "x", desktop: "f2e0c498-c132-42a1-9301-b151850e5d50" }];
    expect(buttonsFor(lower, desktops(2)).map((b) => b.id)).toEqual(["x"]);
  });

  it("shows every button when there are no desktops to choose from", () => {
    expect(buttonsFor(buttons, []).map((b) => b.id)).toEqual([
      "output",
      "sf6-volume",
      "blue-archive",
    ]);
  });
});

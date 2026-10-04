import { describe, expect, it } from "vitest";
import { buttonsFor } from "./tabs.js";

const buttons = [
  { id: "output", desktop: null, except: ["dev"] },
  { id: "everywhere", desktop: null, except: [] },
  { id: "sf6", desktop: "SF6", except: [] },
  { id: "blue-archive", desktop: "ブルアカ", except: [] },
];
const desktops = (current) =>
  ["dev", "sf6", "ブルアカ"].map((name, index) => ({
    id: `GUID-${index}`,
    name,
    index,
    current: name === current,
  }));
const ids = (list) => list.map((b) => b.id);

describe("buttonsFor", () => {
  it("shows a desktop's own buttons after the shared ones, matching names without case", () => {
    expect(ids(buttonsFor(buttons, desktops("sf6")))).toEqual(["output", "everywhere", "sf6"]);
    expect(ids(buttonsFor(buttons, desktops("ブルアカ")))).toEqual([
      "output",
      "everywhere",
      "blue-archive",
    ]);
  });

  it("leaves shared buttons out of the desktops they except", () => {
    expect(ids(buttonsFor(buttons, desktops("dev")))).toEqual(["everywhere"]);
    const upper = [{ id: "x", desktop: null, except: ["DEV"] }];
    expect(ids(buttonsFor(upper, desktops("dev")))).toEqual([]);
  });

  it("shows every button when there are no desktops to choose from", () => {
    expect(ids(buttonsFor(buttons, []))).toEqual(["output", "everywhere", "sf6", "blue-archive"]);
  });
});

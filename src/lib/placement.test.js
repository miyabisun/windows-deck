import { describe, expect, it } from "vitest";
import { chooseMonitor } from "./placement.js";

const display = (n) => `\\\\.\\DISPLAY${n}`;

const monitors = [
  { id: "mon-1", name: "U13NA", gdi_name: display(1), primary: false, touch: false },
  { id: "mon-3", name: "25M2S", gdi_name: display(3), primary: true, touch: false },
  { id: "mon-2", name: "JAPANNEXT MNT", gdi_name: display(2), primary: false, touch: true },
];

describe("chooseMonitor", () => {
  it("uses the configured monitor by id, name or display name, ignoring case", () => {
    expect(chooseMonitor(monitors, "mon-1")).toBe(display(1));
    expect(chooseMonitor(monitors, "japannext mnt")).toBe(display(2));
    expect(chooseMonitor(monitors, "\\\\.\\display3")).toBe(display(3));
  });

  it("falls back to the primary monitor when the configured one is not connected", () => {
    expect(chooseMonitor(monitors, "10-inch")).toBeNull();
  });

  it("uses the only touch monitor when nothing is configured", () => {
    expect(chooseMonitor(monitors, null)).toBe(display(2));
    expect(chooseMonitor(monitors, "")).toBe(display(2));
  });

  it("uses the primary monitor when there is no touch monitor or more than one", () => {
    expect(
      chooseMonitor(
        monitors.filter((m) => !m.touch),
        null,
      ),
    ).toBeNull();
    const two = [
      ...monitors,
      { id: "mon-4", name: "10in", gdi_name: display(4), primary: false, touch: true },
    ];
    expect(chooseMonitor(two, null)).toBeNull();
    expect(chooseMonitor([], null)).toBeNull();
  });
});

import { describe, expect, it } from "vitest";
import { placement } from "./placement.js";

const display = (n) => `\\\\.\\DISPLAY${n}`;

const monitors = [
  { id: "mon-1", name: "U13NA", gdi_name: display(1), primary: false, touch: false },
  { id: "mon-3", name: "25M2S", gdi_name: display(3), primary: true, touch: true },
  { id: "mon-2", name: "T10FA        ", gdi_name: display(2), primary: false, touch: true },
];

describe("placement", () => {
  it("fills the configured monitor, found by id, name or display name, ignoring case", () => {
    expect(placement(monitors, "mon-1")).toEqual({ kind: "fill", display: display(1) });
    expect(placement(monitors, "t10fa")).toEqual({ kind: "fill", display: display(2) });
    expect(placement(monitors, "\\\\.\\display3")).toEqual({ kind: "fill", display: display(3) });
  });

  it("waits, rather than using another monitor, while the configured one is not there", () => {
    expect(placement(monitors, "10-inch")).toEqual({
      kind: "wait",
      reason: "monitor 10-inch is not connected",
    });
    expect(placement(null, "mon-2")).toEqual({
      kind: "wait",
      reason: "windows-link has not reported the monitors",
    });
  });

  it("asks for the setting when no monitor is configured, even with a single touch monitor", () => {
    const one = monitors.filter((m) => m.id !== "mon-3");
    expect(placement(one, null)).toEqual({ kind: "setup", candidates: one });
    expect(placement(monitors, "  ")).toEqual({ kind: "setup", candidates: monitors });
    expect(placement(null, undefined)).toEqual({ kind: "setup", candidates: [] });
  });
});

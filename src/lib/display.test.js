import { describe, expect, it } from "vitest";
import { describeState, pressFailure } from "./display.js";

describe("describeState", () => {
  it("shows the current output device and notes a disconnected alternative", () => {
    const state = {
      kind: "output",
      current: "motu",
      current_name: "MOTU (MOTU M Series)",
      options: [
        { alias: "motu", name: "MOTU (MOTU M Series)", connected: true },
        { alias: "jbl", name: "BTイヤホン (JBL Tour Pro 3)", connected: false },
      ],
    };
    expect(describeState(state)).toEqual({
      text: "MOTU (MOTU M Series)",
      note: "BTイヤホン (JBL Tour Pro 3) は未接続",
      failed: false,
    });
  });

  it("falls back to the alias, then to an unknown output", () => {
    const options = [{ alias: "jbl", name: "JBL", connected: true }];
    expect(
      describeState({ kind: "output", current: "jbl", current_name: null, options }).text,
    ).toBe("jbl");
    expect(describeState({ kind: "output", current: null, current_name: null, options }).text).toBe(
      "出力先が不明",
    );
  });

  it("shows an application's volume as a percentage, or that it is not running", () => {
    expect(describeState({ kind: "volume", running: true, volume: 0.2, levels: [0.2, 1] })).toEqual(
      {
        text: "20%",
        note: null,
        failed: false,
      },
    );
    expect(
      describeState({ kind: "volume", running: true, volume: 0.999, levels: [0.2, 1] }).text,
    ).toBe("100%");
    expect(
      describeState({ kind: "volume", running: false, volume: null, levels: [0.2, 1] }).text,
    ).toBe("起動していません");
  });

  it("reports a button that windows-link cannot read as failed with its message", () => {
    expect(describeState({ kind: "error", message: "audio service stopped" })).toEqual({
      text: "状態を読めません",
      note: "audio service stopped",
      failed: true,
    });
  });

  it("does not break on a state kind this version does not know", () => {
    expect(describeState({ kind: "future" })).toEqual({
      text: "状態不明",
      note: null,
      failed: false,
    });
    expect(describeState(undefined)).toEqual({ text: "状態不明", note: null, failed: false });
  });
});

describe("pressFailure", () => {
  it("explains the conflicts a user can fix", () => {
    expect(pressFailure(409, { error: "device_unavailable" })).toBe(
      "切替先のデバイスがつながっていません",
    );
    expect(pressFailure(409, { error: "not_running" })).toBe("対象のアプリが起動していません");
    expect(pressFailure(404, { error: "not_found" })).toBe(
      "windows-link にこのボタンがありません（設定が変わった可能性があります）",
    );
  });

  it("keeps the server's message for other errors", () => {
    expect(pressFailure(500, { error: "audio", message: "COM failed" })).toBe(
      "Windows の操作に失敗しました: COM failed",
    );
    expect(pressFailure(502, null)).toBe("windows-link がエラーを返しました（HTTP 502）");
  });

  it("explains that the request did not reach windows-link", () => {
    expect(pressFailure(null, null)).toBe("windows-link に届きませんでした");
  });
});

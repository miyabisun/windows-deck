import { describe, expect, it } from "vitest";
import {
  stateIcon,
  describeState,
  labelFailure,
  keysFailure,
  mixerFailure,
  libraryFailure,
  pressFailure,
  switchFailure,
} from "./display.js";

describe("describeState of the mute button and the mixer", () => {
  it("says what a press does and the volume", () => {
    expect(describeState({ kind: "mute", muted: false, volume: 0.45 })).toEqual({
      text: "ミュート",
      note: "音量 45%",
      failed: false,
    });
    expect(describeState({ kind: "mute", muted: true, volume: 0.45 })).toEqual({
      text: "ミュート解除",
      note: "ミュート中",
      failed: false,
    });
    expect(describeState({ kind: "mixer", muted: false, volume: 0.45 })).toEqual({
      text: "音量 45%",
      note: null,
      failed: false,
    });
    expect(describeState({ kind: "mixer", muted: true, volume: 0.45 })).toEqual({
      text: "音量 45%",
      note: "ミュート中",
      failed: false,
    });
  });
});

describe("stateIcon", () => {
  it("shows the current device, whether the output is muted, and a speaker for the mixer", () => {
    const output = (current, icons) => ({
      kind: "output",
      current,
      options: [
        { alias: "d3v", icon: icons?.[0] ?? null },
        { alias: "jbl", icon: icons?.[1] ?? null },
      ],
    });
    expect(stateIcon(output("d3v", ["speaker", "headphones"]))).toBe("speaker");
    expect(stateIcon(output("jbl", ["speaker", "headphones"]))).toBe("headphones");
    // Without icons in the configuration, or with another device as the default.
    expect(stateIcon(output("jbl"))).toBe(null);
    expect(stateIcon(output(null, ["speaker", "headphones"]))).toBe(null);
    expect(stateIcon({ kind: "mute", muted: false, volume: 1 })).toBe("sound");
    expect(stateIcon({ kind: "mute", muted: true, volume: 1 })).toBe("muted");
    expect(stateIcon({ kind: "mixer", muted: false, volume: 1 })).toBe("volume");
    expect(stateIcon({ kind: "game", running: false })).toBe(null);
    expect(stateIcon(undefined)).toBe(null);
  });
});

describe("mixerFailure", () => {
  it("says why the mixer cannot read or change a volume", () => {
    expect(mixerFailure(404, { error: "not_found" })).toBe(
      "このアプリは今は音を出していません（ミキサーを開き直してください）",
    );
    expect(mixerFailure(500, { error: "audio", message: "no device" })).toBe(
      "音量を操作できません: no device",
    );
    expect(mixerFailure(null, null)).toBe("windows-link に届きませんでした");
  });
});

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

describe("switchFailure", () => {
  it("explains why a desktop switch failed", () => {
    expect(switchFailure(404, { error: "not_found" })).toBe("このデスクトップはもうありません");
    expect(switchFailure(503, { error: "desktops", message: "service stopped" })).toBe(
      "仮想デスクトップを切り替えられません: service stopped",
    );
    expect(switchFailure(null, null)).toBe("windows-link に届きませんでした");
  });
});

describe("voice buttons", () => {
  it("say what a press does: join the voice chat, or leave it while in it", () => {
    expect(describeState({ kind: "voice", available: true, joined: true, reason: null })).toEqual({
      text: "ボイチャから退室",
      note: "参加中",
      failed: false,
    });
    expect(
      describeState({ kind: "voice", available: true, joined: false, reason: null }).text,
    ).toBe("ボイチャに入室");
  });

  it("explain an unreachable Discord in a way the user can act on", () => {
    const off = (reason) =>
      describeState({ kind: "voice", available: false, joined: false, reason });
    expect(off("Discord is not running")).toEqual({
      text: "Discord 未接続",
      note: "Discord が起動していません（押すと起動して参加します）",
      failed: false,
    });
    expect(off("windows-link needs approval in Discord: press a Discord button").note).toBe(
      "押すと Discord に許可の確認が出ます",
    );
    expect(off("secrets.yaml has no discord section").note).toBe(
      "secrets.yaml has no discord section",
    );
  });

  it("explain failed presses", () => {
    expect(
      pressFailure(409, { error: "discord_unavailable", message: "Discord did not start in time" }),
    ).toBe("Discord につながりません: Discord did not start in time");
    expect(pressFailure(409, { error: "discord_rejected" })).toBe("Discord で許可されませんでした");
  });
});

describe("launch and game buttons", () => {
  it("say what a press will do", () => {
    expect(describeState({ kind: "launch", running: false })).toEqual({
      text: "起動",
      note: null,
      failed: false,
    });
    expect(describeState({ kind: "launch", running: true })).toEqual({
      text: "前に出す",
      note: "起動中",
      failed: false,
    });
    expect(describeState({ kind: "game", running: false })).toEqual({
      text: "起動",
      note: null,
      failed: false,
    });
    expect(describeState({ kind: "game", running: true })).toEqual({
      text: "終了",
      note: "起動中",
      failed: false,
    });
  });

  it("explain failed presses", () => {
    expect(pressFailure(409, { error: "no_window", message: "x" })).toBe(
      "ゲームのウィンドウがまだ無いため閉じられません",
    );
    expect(pressFailure(500, { error: "launch", message: "cannot open x" })).toBe(
      "起動できませんでした: cannot open x",
    );
  });
});

describe("library buttons", () => {
  it("say that a press opens the list", () => {
    expect(describeState({ kind: "library", pins: [] })).toEqual({
      text: "一覧を開く",
      note: null,
      failed: false,
    });
  });

  it("explain why license keys cannot be shown", () => {
    expect(keysFailure(409, { error: "keys_unavailable", message: "DLsite is down" })).toBe(
      "DLsite に問い合わせできません: DLsite is down",
    );
    expect(keysFailure(404, { error: "not_found" })).toBe(
      "このゲームのシリアル番号は分かりません（DLsite の作品が分かっていません）",
    );
    expect(keysFailure(null, null)).toBe("windows-link に届きませんでした");
  });

  it("explain a failed start or pin", () => {
    expect(libraryFailure(404, { error: "not_found" })).toBe(
      "ライブラリにこのゲームがありません（一覧を開き直してください）",
    );
    expect(libraryFailure(409, { error: "not_downloaded", message: "ダウンロード中 40%" })).toBe(
      "まだこの PC にありません（ダウンロード中 40%）",
    );
    expect(libraryFailure(500, { error: "launch", message: "denied" })).toBe(
      "起動できませんでした: denied",
    );
    expect(libraryFailure(null, null)).toBe("windows-link に届きませんでした");
    expect(libraryFailure(409, { error: "no_program" })).toBe(
      "起動できるファイルが見つかりません（ローカルファイル閲覧で中を確かめてください）",
    );
  });
});

describe("labelFailure", () => {
  it("says why a label could not be changed", () => {
    expect(
      labelFailure(409, { error: "labels_unavailable", message: "Steam が起動していません" }),
    ).toBe("ラベルを変更できません: Steam が起動していません");
    expect(
      labelFailure(400, { error: "invalid_label", message: 'a label named "RPG" exists' }),
    ).toBe('ラベルを変更できません: a label named "RPG" exists');
    expect(labelFailure(404, { error: "not_found" })).toBe(
      "ラベルかゲームが見つかりません（一覧を開き直してください）",
    );
    expect(labelFailure(null, null)).toBe("windows-link に届きませんでした");
  });
});

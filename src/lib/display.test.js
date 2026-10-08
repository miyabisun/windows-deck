import { describe, expect, it } from "vitest";
import {
  stateIcon,
  describeState,
  labelFailure,
  keysFailure,
  mixerFailure,
  libraryFailure,
  pressFailure,
  signInFailure,
  switchFailure,
  updateFailure,
  updateResult,
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

  it("says what a press does to an app's own mute, and when it has no sound", () => {
    expect(describeState({ kind: "app_mute", running: true, muted: false })).toEqual({
      text: "ミュート",
      note: null,
      failed: false,
    });
    expect(describeState({ kind: "app_mute", running: true, muted: true })).toEqual({
      text: "ミュート解除",
      note: "ミュート中",
      failed: false,
    });
    expect(describeState({ kind: "app_mute", running: false, muted: false })).toEqual({
      text: "起動していません",
      note: null,
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
    expect(stateIcon({ kind: "app_mute", running: true, muted: false })).toBe("sound");
    expect(stateIcon({ kind: "app_mute", running: true, muted: true })).toBe("muted");
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

describe("signing in to a shop", () => {
  it("says on a library button when the shop needs signing in", () => {
    expect(describeState({ kind: "library", pins: [], sign_in: "fanza" })).toEqual({
      text: "ライブラリ",
      note: "ログインが必要です",
      failed: false,
    });
    expect(describeState({ kind: "library", pins: [], sign_in: null }).note).toBe(null);
  });

  it("explains a sign-in that did not go through", () => {
    expect(signInFailure(null, { error: "closed" })).toBe(
      "ログインせずにログイン画面が閉じられました",
    );
    expect(signInFailure(null, { error: "not_panel" })).toBe(
      "ログインはタッチパネルのアプリからだけできます",
    );
    expect(signInFailure(400, { error: "invalid_session", message: "x" })).toBe(
      "ログインを確認できませんでした。もう一度ログインしてください",
    );
    expect(signInFailure(null, null)).toBe("windows-link に届きませんでした");
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
      text: "ライブラリ",
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

describe("updating a library", () => {
  it("says what Steam started", () => {
    expect(updateResult({ updates: 3, installs: 24 })).toBe(
      "アップデート 3 件を始めました。インストール 24 件は Steam の画面で確定してください",
    );
    expect(updateResult({ updates: 1, installs: 0 })).toBe("アップデート 1 件を始めました");
    expect(updateResult({ updates: 0, installs: 2 })).toBe(
      "インストール 2 件は Steam の画面で確定してください",
    );
    expect(updateResult({ updates: 0, installs: 0 })).toBe(
      "アップデート・インストールするゲームはありません",
    );
  });

  it("says that a shop's downloads started", () => {
    expect(updateResult({ round: true })).toBe(
      "ダウンロードと更新を始めました（進み具合はライブラリに出ます）",
    );
  });

  it("explains why a library could not be updated", () => {
    expect(updateFailure(409, { error: "sign_in", message: "x" })).toBe(
      "ログインが必要です（ライブラリを開いてログインしてください）",
    );
    expect(
      updateFailure(409, { error: "update_unavailable", message: "Steam is not running" }),
    ).toBe("Steam が起動していないため、ライブラリを更新できません");
    expect(
      updateFailure(409, {
        error: "update_unavailable",
        message:
          "Steam does not accept remote control: create .cef-enable-remote-debugging in the Steam folder and restart Steam",
      }),
    ).toBe(
      "Steam の操作口が無効なため、ライブラリを更新できません（Steam フォルダに .cef-enable-remote-debugging を置いて Steam を再起動）",
    );
    expect(
      updateFailure(409, {
        error: "update_unavailable",
        message: "DLsite cannot download games now",
      }),
    ).toBe("ライブラリを更新できません: DLsite cannot download games now");
    expect(updateFailure(500, { error: "update", message: "TypeError: boom" })).toBe(
      "Windows の操作に失敗しました: TypeError: boom",
    );
    expect(updateFailure(null, null)).toBe("windows-link に届きませんでした");
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

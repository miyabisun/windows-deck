// What a button shows for a windows-link state, and why a press failed.

import { labelsLockedReason, steamDown } from "./library.js";

const UNKNOWN = { text: "状態不明", note: null, failed: false };

/** The colours a highlighted cursor comes in, as the menu lists them. */
const COLORS = [
  { color: "red", name: "赤" },
  { color: "blue", name: "青" },
  { color: "green", name: "緑" },
];

/**
 * The colours to choose for a highlighted cursor (the `cursor.highlight` button's
 * menu), the chosen one marked.
 * @param {any} state the button's state from windows-link
 * @returns {Array<{ color: string, label: string }>}
 */
export function colorMenu(state) {
  return COLORS.map(({ color, name }) => ({
    color,
    label: color === state?.color ? `${name}（選択中）` : name,
  }));
}

/**
 * @param {any} state the `state` of a windows-link button
 * @returns {{ text: string, note: string | null, failed: boolean }}
 */
export function describeState(state) {
  switch (state?.kind) {
    case "output": {
      const others = (state.options ?? []).filter((o) => o.alias !== state.current && !o.connected);
      return {
        text: state.current_name ?? state.current ?? "出力先が不明",
        note: others.length ? others.map((o) => `${o.name ?? o.alias} は未接続`).join("、") : null,
        failed: false,
      };
    }
    case "volume":
      return {
        text: state.running ? `${Math.round(state.volume * 100)}%` : "起動していません",
        note: null,
        failed: false,
      };
    case "launch":
      return state.running
        ? { text: "前に出す", note: "起動中", failed: false }
        : { text: "起動", note: null, failed: false };
    case "game":
      return state.running
        ? { text: "終了", note: "起動中", failed: false }
        : { text: "起動", note: null, failed: false };
    case "library":
      return {
        text: "ライブラリ",
        note: state.sign_in ? "ログインが必要です" : null,
        failed: false,
      };
    case "mute":
      return state.muted
        ? { text: "ミュート解除", note: "ミュート中", failed: false }
        : { text: "ミュート", note: `音量 ${Math.round(state.volume * 100)}%`, failed: false };
    case "app_mute":
      if (!state.running) return { text: "起動していません", note: null, failed: false };
      return state.muted
        ? { text: "ミュート解除", note: "ミュート中", failed: false }
        : { text: "ミュート", note: null, failed: false };
    case "mixer":
      return {
        text: `音量 ${Math.round(state.volume * 100)}%`,
        note: state.muted ? "ミュート中" : null,
        failed: false,
      };
    case "hold":
      return state.held
        ? { text: "自動操作を再開", note: "ユーザー操作中", failed: false }
        : { text: "ユーザー操作中にする", note: null, failed: false };
    case "highlight": {
      const color = COLORS.find((c) => c.color === state.color);
      return state.on
        ? {
            text: "元に戻す",
            note: color ? `目立たせ中（${color.name}）` : "目立たせ中",
            failed: false,
          }
        : { text: "目立たせる", note: null, failed: false };
    }
    case "error":
      return { text: "状態を読めません", note: state.message ?? null, failed: true };
    default:
      return UNKNOWN;
  }
}

/**
 * The icon of a button's state, if it has one: the current output device (when the
 * button says what each is), sound or muted, and a speaker for the mixer.
 * @param {any} state the button's state from windows-link
 * @returns {keyof typeof import("./Icon.svelte").ICONS | null} the icon's name
 */
export function stateIcon(state) {
  switch (state?.kind) {
    case "output":
      return state.options?.find((/** @type {any} */ o) => o.alias === state.current)?.icon ?? null;
    case "mute":
    case "app_mute":
      return state.muted ? "muted" : "sound";
    case "mixer":
      return "volume";
    default:
      return null;
  }
}

/**
 * The reason shown on a button after a failed press.
 * @param {number | null} status HTTP status, or null when the request did not get an answer
 * @param {any} body the JSON error body, if any
 */
export function pressFailure(status, body) {
  if (status === null) return "windows-link に届きませんでした";
  switch (body?.error) {
    case "device_unavailable":
      return "切替先のデバイスがつながっていません";
    case "not_running":
      return "対象のアプリが起動していません";
    case "not_found":
      return "windows-link にこのボタンがありません（設定が変わった可能性があります）";
    case "no_window":
      return "ゲームのウィンドウがまだ無いため閉じられません";
    case "launch":
      return `起動できませんでした: ${body.message}`;
  }
  if (body?.message) return `Windows の操作に失敗しました: ${body.message}`;
  return `windows-link がエラーを返しました（HTTP ${status}）`;
}

/**
 * Why resting or waking did not go through: the panel's monitor unknown (`no_monitor`,
 * from the panel itself), or windows-link refusing or failing.
 * @param {number | null} status HTTP status, or null without an answer from windows-link
 * @param {any} body the JSON error body, if any
 */
export function restFailure(status, body) {
  switch (body?.error) {
    case "no_monitor":
      return "deck のモニターが分からないため休止できません（設定の monitor を確かめてください）";
    case "not_found":
      return "deck のモニターが windows-link に見つかりません";
    case "unrestored":
      return `明るさを戻せなかったモニターがあります。そのモニターのボタンで戻してください（${body.message}）`;
  }
  return pressFailure(status, body);
}

/**
 * Why signing in to a shop did not go through: the login window (status null with
 * `closed`, or `not_panel` outside the panel's app), or windows-link refusing it.
 * @param {number | null} status HTTP status, or null without an answer from windows-link
 * @param {any} body the JSON error body, if any
 */
export function signInFailure(status, body) {
  if (body?.error === "closed") return "ログインせずにログイン画面が閉じられました";
  if (body?.error === "not_panel") return "ログインはタッチパネルのアプリからだけできます";
  if (body?.error === "invalid_session")
    return "ログインを確認できませんでした。もう一度ログインしてください";
  return pressFailure(status, body);
}

/**
 * The reason shown after a library game could not be started, pinned or unpinned.
 * @param {number | null} status HTTP status, or null when the request did not get an answer
 * @param {any} body the JSON error body, if any
 */
export function libraryFailure(status, body) {
  if (body?.error === "no_program")
    return "起動できるファイルが見つかりません（ローカルファイル閲覧で中を確かめてください）";
  if (body?.error === "not_found")
    return "ライブラリにこのゲームがありません（一覧を開き直してください）";
  if (body?.error === "not_downloaded") return `まだこの PC にありません（${body.message}）`;
  return pressFailure(status, body);
}

/**
 * What updating a library started: Steam's updates and the installs it asks to confirm on
 * its own window, or a shop's round of downloads.
 * @param {any} body windows-link's answer
 */
export function updateResult(body) {
  if (typeof body?.updates !== "number")
    return "ダウンロードと更新を始めました（進み具合はライブラリに出ます）";
  const said = [];
  if (body.updates > 0) said.push(`アップデート ${body.updates} 件を始めました`);
  if (body.installs > 0)
    said.push(`インストール ${body.installs} 件は Steam の画面で確定してください`);
  return said.join("。") || "アップデート・インストールするゲームはありません";
}

/**
 * Why a shop's round could not read the purchases, in words the user can act on.
 * @param {string} reason windows-link's `purchases`
 */
function purchasesReason(reason) {
  if (reason.includes("no DLsite account"))
    return "windows-link の secrets.yaml に DLsite のアカウントがありません";
  const refused = reason.match(/login page: http status: (\d+)/);
  if (refused) return `DLsite のログイン画面が HTTP ${refused[1]} を返しました`;
  const signIn = reason.match(/^(\S+) needs signing in again through the panel/);
  if (signIn) return `${signIn[1]} へのログインが必要です。ライブラリを開いてログインしてください`;
  return reason;
}

/**
 * What a shop's round of downloads and updates is doing, or how the last one ended,
 * and why it could not read the purchases.
 * @param {any} round windows-link's `GET …/library/update`
 * @param {string} [timeZone] where the end is written; the panel's own unless given
 * @returns {{ busy: boolean, text: string, failure: string | null }}
 */
export function roundSummary(round, timeZone) {
  const failure = round.purchases
    ? `購入一覧を読めませんでした（${purchasesReason(round.purchases)}）。新しいゲームと更新は探せていません`
    : round.folder
      ? `ゲームのフォルダを読めないため、ダウンロードしませんでした（${round.folder}）`
      : null;
  const jobs = round.jobs ?? [];
  const done = jobs.filter((/** @type {any} */ j) => j.status === "完了").length;
  if (round.doing === "purchases") return { busy: true, text: "購入一覧を読んでいます", failure };
  if (round.doing === "downloads")
    return {
      busy: true,
      text: `ダウンロードと更新: ${jobs.length} 件のうち ${done} 件が完了`,
      failure,
    };
  if (!round.finished)
    return { busy: false, text: "windows-link の起動から、まだ一度も更新していません", failure };
  const ended = new Intl.DateTimeFormat("ja-JP", {
    timeZone,
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(round.finished * 1000);
  const failed = jobs.filter((/** @type {any} */ j) => j.status.includes("失敗")).length;
  // Without the purchases it could not look, so "nothing new" would not be true.
  const result = jobs.length
    ? `完了 ${done} 件・失敗 ${failed} 件`
    : failure
      ? null
      : "新しいゲームも更新もありませんでした";
  return { busy: false, text: `${ended} に終わりました${result ? `。${result}` : ""}`, failure };
}

/**
 * The reason shown on a library button after it could not be updated.
 * @param {number | null} status HTTP status, or null when the request did not get an answer
 * @param {any} body the JSON error body, if any
 */
export function updateFailure(status, body) {
  if (body?.error === "sign_in")
    return "ログインが必要です（ライブラリを開いてログインしてください）";
  if (body?.error === "update_unavailable")
    return (
      steamDown(body.message, "ライブラリを更新できません") ??
      `ライブラリを更新できません: ${body.message}`
    );
  return pressFailure(status, body);
}

/**
 * The reason shown when the mixer cannot read or change a volume.
 * @param {number | null} status HTTP status, or null when the request did not get an answer
 * @param {any} body the JSON error body, if any
 */
export function mixerFailure(status, body) {
  if (body?.error === "not_found")
    return "このアプリは今は音を出していません（ミキサーを開き直してください）";
  if (body?.error === "audio") return `音量を操作できません: ${body.message}`;
  return pressFailure(status, body);
}

/**
 * The reason shown when a game's license keys cannot be shown.
 * @param {number | null} status HTTP status, or null when the request did not get an answer
 * @param {any} body the JSON error body, if any
 */
export function keysFailure(status, body) {
  if (body?.error === "keys_unavailable") return `DLsite に問い合わせできません: ${body.message}`;
  if (body?.error === "not_found")
    return "このゲームのシリアル番号は分かりません（DLsite の作品が分かっていません）";
  return pressFailure(status, body);
}

/**
 * The reason shown after a label could not be created, renamed, deleted or filled.
 * @param {number | null} status HTTP status, or null when the request did not get an answer
 * @param {any} body the JSON error body, if any
 */
export function labelFailure(status, body) {
  if (body?.error === "not_found")
    return "ラベルかゲームが見つかりません（一覧を開き直してください）";
  if (body?.error === "labels_unavailable") return labelsLockedReason(body.message);
  if (body?.error === "invalid_label" || body?.error === "labels")
    return `ラベルを変更できません: ${body.message}`;
  return pressFailure(status, body);
}

/**
 * The reason shown after a desktop tab could not switch Windows to its desktop.
 * @param {number | null} status HTTP status, or null when the request did not get an answer
 * @param {any} body the JSON error body, if any
 */
export function switchFailure(status, body) {
  if (status === null) return "windows-link に届きませんでした";
  if (body?.error === "not_found") return "このデスクトップはもうありません";
  return `仮想デスクトップを切り替えられません: ${body?.message ?? `HTTP ${status}`}`;
}

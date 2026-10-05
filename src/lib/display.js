// What a button shows for a windows-link state, and why a press failed.

import { labelsLockedReason } from "./library.js";

const UNKNOWN = { text: "状態不明", note: null, failed: false };

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
    case "voice":
      if (state.available)
        return state.joined
          ? { text: "ボイチャから退室", note: "参加中", failed: false }
          : { text: "ボイチャに入室", note: null, failed: false };
      return { text: "Discord 未接続", note: voiceReason(state.reason), failed: false };
    case "launch":
      return state.running
        ? { text: "前に出す", note: "起動中", failed: false }
        : { text: "起動", note: null, failed: false };
    case "game":
      return state.running
        ? { text: "終了", note: "起動中", failed: false }
        : { text: "起動", note: null, failed: false };
    case "library":
      return { text: "一覧を開く", note: null, failed: false };
    case "error":
      return { text: "状態を読めません", note: state.message ?? null, failed: true };
    default:
      return UNKNOWN;
  }
}

/** Why a voice button cannot reach Discord, in words the user can act on. */
function voiceReason(reason) {
  if (!reason) return null;
  if (reason.startsWith("Discord is not running")) {
    return "Discord が起動していません（押すと起動して参加します）";
  }
  if (reason.startsWith("windows-link needs approval"))
    return "押すと Discord に許可の確認が出ます";
  return reason;
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
    case "discord_unavailable":
      return `Discord につながりません: ${body.message}`;
    case "discord_rejected":
      return "Discord で許可されませんでした";
    case "no_window":
      return "ゲームのウィンドウがまだ無いため閉じられません";
    case "launch":
      return `起動できませんでした: ${body.message}`;
    case "discord":
      return `Discord の操作に失敗しました: ${body.message}`;
  }
  if (body?.message) return `Windows の操作に失敗しました: ${body.message}`;
  return `windows-link がエラーを返しました（HTTP ${status}）`;
}

/**
 * The reason shown after a library game could not be started, pinned or unpinned.
 * @param {number | null} status HTTP status, or null when the request did not get an answer
 * @param {any} body the JSON error body, if any
 */
export function libraryFailure(status, body) {
  if (body?.error === "not_found")
    return "ライブラリにこのゲームがありません（一覧を開き直してください）";
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

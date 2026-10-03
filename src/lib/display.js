// What a button shows for a windows-link state, and why a press failed.

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
    case "error":
      return { text: "状態を読めません", note: state.message ?? null, failed: true };
    default:
      return UNKNOWN;
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
  }
  if (body?.message) return `Windows の操作に失敗しました: ${body.message}`;
  return `windows-link がエラーを返しました（HTTP ${status}）`;
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

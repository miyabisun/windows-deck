// Searching a windows-link game library (`GET /buttons/{id}/library`).

/** Fold full-width letters and case so that "ｓｔｒｅｅｔ" finds "Street". */
const fold = (/** @type {string} */ text) => text.normalize("NFKC").toLowerCase();

/**
 * The games to show, like fzf: every word of `query` is in the name or the detail (the
 * maker), or failing that its letters come there in order (those follow the others);
 * with labels selected, the
 * game has one of them; and a game with a `hide` label shows only while that label is
 * selected.
 * @template {{ name: string, detail?: string | null, labels: string[] }} T
 * @param {T[]} items
 * @param {{ query?: string, active?: string[], hide?: string[] }} filter
 * @returns {T[]}
 */
export function visibleItems(items, { query = "", active = [], hide = [] }) {
  const words = fold(query).split(/\s+/).filter(Boolean);
  const plain = [];
  const scattered = [];
  for (const item of items) {
    if (active.length && !item.labels.some((label) => active.includes(label))) continue;
    if (item.labels.some((label) => hide.includes(label) && !active.includes(label))) continue;
    const text = fold(item.detail ? `${item.name} ${item.detail}` : item.name);
    if (words.every((word) => text.includes(word))) plain.push(item);
    else if (words.every((word) => inOrder(word, text))) scattered.push(item);
  }
  return plain.concat(scattered);
}

/** Whether the letters of `word` come in `text` in this order, with anything between. */
function inOrder(word, text) {
  let at = 0;
  for (const letter of word) {
    at = text.indexOf(letter, at);
    if (at < 0) return false;
    at += letter.length;
  }
  return true;
}

/**
 * Why the library lists only the installed games, in words the user can act on.
 * @param {string | null} reason windows-link's `partial`
 */
export function partialReason(reason) {
  if (!reason) return null;
  if (reason.includes("no steam api_key"))
    return "windows-link の secrets.yaml に Steam の API キーが無いため、インストール済みのゲームだけを出しています";
  if (reason.includes("rejected the key"))
    return "Steam が API キーを受け付けません。インストール済みのゲームだけを出しています";
  if (reason.includes("not fetched yet"))
    return "所有ゲームを取得中です。インストール済みのゲームだけを出しています";
  const missing = reason.match(/^(.*) does not exist$/);
  if (missing) return `ゲームのフォルダ ${missing[1]} がありません`;
  const empty = reason.match(/^there are no games in (.*)$/);
  if (empty) return `${empty[1]} にゲームがありません`;
  return `インストール済みのゲームだけを出しています: ${reason}`;
}

/**
 * Why labels cannot be changed right now, in words the user can act on.
 * @param {string | null} reason windows-link's `labels_locked` or an error message
 */
export function labelsLockedReason(reason) {
  if (!reason) return null;
  if (reason.includes("not running")) return "Steam が起動していないため、ラベルは変更できません";
  if (reason.includes("cef-enable-remote-debugging"))
    return "Steam の操作口が無効なため、ラベルは変更できません（Steam フォルダに .cef-enable-remote-debugging を置いて Steam を再起動）";
  return `ラベルを変更できません: ${reason}`;
}

/**
 * Where windows-link serves a library game's picture.
 * @param {string} base windows-link URL
 * @param {string} button the library button's ID
 * @param {string} item the game's ID
 */
export function pictureUrl(base, button, item) {
  return `${base}/buttons/${encodeURIComponent(button)}/library/${encodeURIComponent(item)}/image`;
}

/**
 * The games pinned to the library buttons among `buttons`, in button order; a tab shows
 * them after its buttons.
 * @param {Array<{ id: string, state?: any }>} buttons
 * @returns {Array<{ button: string, pin: { id: string, name: string }, icon: boolean }>}
 *   `icon` when the pictures are program icons to show whole
 */
export function pinsOf(buttons) {
  return buttons.flatMap((b) =>
    b.state?.kind === "library"
      ? (b.state.pins ?? []).map((/** @type {any} */ pin) => ({
          button: b.id,
          pin,
          icon: b.state.pictures === "icon",
        }))
      : [],
  );
}

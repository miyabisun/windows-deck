// Searching a windows-link game library (`GET /buttons/{id}/library`).

/** Fold full-width letters and case so that "ｓｔｒｅｅｔ" finds "Street". */
const fold = (/** @type {string} */ text) => text.normalize("NFKC").toLowerCase();

/**
 * The games to show: every word of `query` is in the name; with labels selected, the
 * game has one of them; and a game with a `hide` label shows only while that label is
 * selected.
 * @template {{ name: string, labels: string[] }} T
 * @param {T[]} items
 * @param {{ query?: string, active?: string[], hide?: string[] }} filter
 * @returns {T[]}
 */
export function visibleItems(items, { query = "", active = [], hide = [] }) {
  const words = fold(query).split(/\s+/).filter(Boolean);
  return items.filter((item) => {
    const name = fold(item.name);
    if (!words.every((word) => name.includes(word))) return false;
    if (active.length && !item.labels.some((label) => active.includes(label))) return false;
    return !item.labels.some((label) => hide.includes(label) && !active.includes(label));
  });
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
  return `インストール済みのゲームだけを出しています: ${reason}`;
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
 * @returns {Array<{ button: string, pin: { id: string, name: string } }>}
 */
export function pinsOf(buttons) {
  return buttons.flatMap((b) =>
    b.state?.kind === "library"
      ? (b.state.pins ?? []).map((/** @type {any} */ pin) => ({ button: b.id, pin }))
      : [],
  );
}

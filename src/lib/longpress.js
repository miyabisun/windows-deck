// Long press on touch tiles (DESIGN.md "Components").

/** How long a finger or the mouse button must stay down. */
export const LONG_PRESS_MS = 500;
/** How far it may move meanwhile, so that scrolling is not a long press. */
const SLOP_PX = 10;

/**
 * Svelte action: call `onlong` with where the press is when a pointer stays down
 * `LONG_PRESS_MS` without moving. The click that ends that press is swallowed so it
 * does not also tap.
 * @param {HTMLElement} node
 * @param {((point: { x: number, y: number }) => void) | null} onlong
 */
export function longpress(node, onlong) {
  /** @type {ReturnType<typeof setTimeout> | null} */
  let timer = null;
  let origin = { x: 0, y: 0 };
  let fired = false;

  const cancel = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  };
  const down = (/** @type {PointerEvent} */ event) => {
    // A long touch may end without a click; forget it on the next press.
    fired = false;
    cancel();
    if (!onlong || event.button !== 0) return;
    origin = { x: event.clientX, y: event.clientY };
    timer = setTimeout(() => {
      timer = null;
      fired = true;
      onlong?.({ ...origin });
    }, LONG_PRESS_MS);
  };
  const move = (/** @type {PointerEvent} */ event) => {
    if (timer !== null && Math.hypot(event.clientX - origin.x, event.clientY - origin.y) > SLOP_PX)
      cancel();
  };
  const click = (/** @type {MouseEvent} */ event) => {
    if (!fired) return;
    fired = false;
    event.preventDefault();
    event.stopImmediatePropagation();
  };

  const listeners = /** @type {const} */ ([
    ["pointerdown", down],
    ["pointermove", move],
    ["pointerup", cancel],
    ["pointercancel", cancel],
    ["pointerleave", cancel],
  ]);
  for (const [type, fn] of listeners) node.addEventListener(type, /** @type {any} */ (fn));
  node.addEventListener("click", click, true);
  return {
    /** @param {((point: { x: number, y: number }) => void) | null} next */
    update(next) {
      onlong = next;
    },
    destroy() {
      cancel();
      for (const [type, fn] of listeners) node.removeEventListener(type, /** @type {any} */ (fn));
      node.removeEventListener("click", click, true);
    },
  };
}

// Which monitor the panel goes to, from windows-link's `GET /touch-monitors`.

/**
 * @typedef {{ id: string, name: string, gdi_name: string, primary: boolean, touch: boolean }} Monitor
 * @typedef {{ kind: "fill", display: string }
 *   | { kind: "wait", reason: string }
 *   | { kind: "setup", candidates: Monitor[] }} Placement
 */

/**
 * The panel fills only the configured monitor. While that one is not there (not
 * connected, or windows-link has not answered) it waits instead of using another, and
 * without a configured monitor it asks for one.
 * @param {Monitor[] | null} monitors windows-link's answer, or null without one
 * @param {string | null | undefined} preference configured monitor: id, name or display name
 * @returns {Placement}
 */
export function placement(monitors, preference) {
  const wanted = preference?.trim().toLowerCase();
  if (!wanted) return { kind: "setup", candidates: monitors ?? [] };
  if (!monitors) return { kind: "wait", reason: "windows-link has not reported the monitors" };
  const match = monitors.find((m) =>
    [m.id, m.name, m.gdi_name].some((value) => value?.trim().toLowerCase() === wanted),
  );
  if (!match) return { kind: "wait", reason: `monitor ${preference} is not connected` };
  return { kind: "fill", display: match.gdi_name };
}

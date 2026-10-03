// Which monitor the panel goes to, from windows-link's `GET /touch-monitors`.

/**
 * @param {Array<{ id: string, name: string, gdi_name: string, touch: boolean }>} monitors
 * @param {string | null | undefined} preference configured monitor: id, name or display name
 * @returns {string | null} the display name (`\.\DISPLAYn`) to use, or null for the primary monitor
 */
export function chooseMonitor(monitors, preference) {
  if (preference) {
    const wanted = preference.toLowerCase();
    const match = monitors.find((m) =>
      [m.id, m.name, m.gdi_name].some((value) => value?.toLowerCase() === wanted),
    );
    return match?.gdi_name ?? null;
  }
  const touch = monitors.filter((m) => m.touch);
  return touch.length === 1 ? touch[0].gdi_name : null;
}

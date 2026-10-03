// Which buttons a desktop tab shows.

/**
 * Buttons without a desktop belong to every tab; the others only to their desktop's tab.
 * Without desktops (the feature is off in windows-link) every button is shown.
 * @template {{ desktop?: string | null }} B
 * @param {B[]} buttons
 * @param {Array<{ id: string, current: boolean }>} desktops
 * @returns {B[]}
 */
export function buttonsFor(buttons, desktops) {
  const current = desktops.find((d) => d.current);
  if (!current) return buttons;
  const id = current.id.toLowerCase();
  return buttons.filter((b) => !b.desktop || b.desktop.toLowerCase() === id);
}

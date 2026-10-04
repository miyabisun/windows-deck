// Which buttons a desktop tab shows.

const same = (a, b) => a.toLowerCase() === b.toLowerCase();

/**
 * A desktop's tab shows the shared buttons (no `desktop`) except those that name it in
 * `except`, then the buttons of its own desktop file (`desktop` is the desktop's name).
 * Without desktops (the feature is off in windows-link) every button is shown.
 * @template {{ desktop?: string | null, except?: string[] }} B
 * @param {B[]} buttons
 * @param {Array<{ name: string, current: boolean }>} desktops
 * @returns {B[]}
 */
export function buttonsFor(buttons, desktops) {
  const current = desktops.find((d) => d.current);
  if (!current) return buttons;
  return buttons.filter((b) =>
    b.desktop
      ? same(b.desktop, current.name)
      : !(b.except ?? []).some((name) => same(name, current.name)),
  );
}

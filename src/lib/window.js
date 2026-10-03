// The panel window: its configuration and which monitor it fills. Outside Tauri (the
// browser during development and tests) the configuration comes from the page URL.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { chooseMonitor } from "./placement.js";

const DEFAULT_LINK = "http://127.0.0.1:4730";
const MONITOR_LOOKUP_MS = 2000;

/** @returns {Promise<{ link: string, monitor: string | null, error: string | null }>} */
export async function loadConfig() {
  if (isTauri()) return invoke("deck_config");
  const params = new URLSearchParams(location.search);
  return { link: params.get("link") ?? DEFAULT_LINK, monitor: params.get("monitor"), error: null };
}

/**
 * Fill the configured monitor (or the touch monitor windows-link reports) and show the
 * window. Without an answer from windows-link the window goes to the primary monitor.
 * @returns {Promise<boolean>} whether windows-link answered, so the choice is final
 */
export async function placeWindow(link, preference) {
  if (!isTauri()) return true;
  let target = null;
  let answered = false;
  try {
    const response = await fetch(`${link}/touch-monitors`, {
      signal: AbortSignal.timeout(MONITOR_LOOKUP_MS),
    });
    if (response.ok) {
      target = chooseMonitor(await response.json(), preference);
      answered = true;
    }
  } catch {
    // windows-link is not reachable yet; use the primary monitor for now.
  }
  await invoke("place_window", { monitor: target });
  return answered;
}

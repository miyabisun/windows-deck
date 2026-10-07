// The panel window: its configuration and which monitor it fills. Outside Tauri (the
// browser during development and tests) the configuration comes from the page URL.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { placement } from "./placement.js";

const DEFAULT_LINK = "http://127.0.0.1:4730";
const MONITOR_LOOKUP_MS = 2000;
const PLACE_RETRY_MS = 5000;

/** @returns {Promise<{ link: string, monitor: string | null, error: string | null, path: string }>} */
export async function loadConfig() {
  if (isTauri()) return invoke("deck_config");
  const params = new URLSearchParams(location.search);
  return {
    link: params.get("link") ?? DEFAULT_LINK,
    monitor: params.get("monitor"),
    error: null,
    path: "",
  };
}

/**
 * Open a shop's login window (on the primary monitor, shown on every virtual desktop
 * through windows-link at `link`) and wait until the user has signed in.
 * @param {string} shop only `fanza` has one
 * @param {string} link windows-link's URL
 * @returns {Promise<any[]>} the shop's cookies; rejects with `{ error }`: `closed` when the
 *   window was closed first, `not_panel` outside the panel's app
 */
export async function shopLogin(shop, link) {
  if (!isTauri() || shop !== "fanza") throw { error: "not_panel" };
  try {
    return await invoke("fanza_login", { link });
  } catch (reason) {
    const message = String(reason);
    throw message.includes("closed") ? { error: "closed" } : { error: "window", message };
  }
}

/**
 * This window's handle for windows-link to pin. In the browser it comes from `?hwnd=`
 * (for tests), or there is none.
 * @returns {Promise<number | null>}
 */
export async function windowHandle() {
  if (isTauri()) return invoke("window_handle");
  const hwnd = new URLSearchParams(location.search).get("hwnd");
  return hwnd ? Number(hwnd) : null;
}

/** @returns {Promise<import("./placement.js").Monitor[] | null>} null without an answer */
async function touchMonitors(link) {
  try {
    const response = await fetch(`${link}/touch-monitors`, {
      signal: AbortSignal.timeout(MONITOR_LOOKUP_MS),
    });
    return response.ok ? await response.json() : null;
  } catch {
    return null;
  }
}

/**
 * Fill the configured monitor and show the window, waiting (hidden) until windows-link
 * reports that monitor and it is connected. Without a configured monitor, show an
 * ordinary window that asks for one. Outside Tauri there is no window to place.
 * @returns {Promise<import("./placement.js").Placement | null>}
 */
export async function placeWindow(link, preference) {
  if (!isTauri()) return null;
  let waitingFor = null;
  for (;;) {
    let choice = placement(await touchMonitors(link), preference);
    if (choice.kind !== "wait") {
      try {
        await invoke("place_window", { monitor: choice.kind === "fill" ? choice.display : null });
        return choice;
      } catch (err) {
        choice = { kind: "wait", reason: String(err) };
      }
    }
    if (choice.reason !== waitingFor) {
      waitingFor = choice.reason;
      await invoke("wait_note", { reason: waitingFor });
    }
    await new Promise((resolve) => setTimeout(resolve, PLACE_RETRY_MS));
  }
}

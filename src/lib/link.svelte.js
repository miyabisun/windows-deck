// Connection to windows-link: the button list and states from the `/events` socket,
// presses over HTTP, and automatic reconnection.

import {
  keysFailure,
  mixerFailure,
  labelFailure,
  libraryFailure,
  pressFailure,
  signInFailure,
  switchFailure,
} from "./display.js";
import { shopLogin } from "./window.js";

/** The `pending` and `failures` key of a library game. */
export const itemKey = (/** @type {string} */ id, /** @type {string} */ item) => `${id}/${item}`;

const FIRST_RETRY_MS = 1000;
const MAX_RETRY_MS = 5000;

export class Link {
  /** @type {"connecting" | "connected" | "disconnected"} */
  status = $state("connecting");
  /** @type {any[]} */
  buttons = $state([]);
  /** @type {Record<string, true>} */
  pending = $state({});
  /** @type {Record<string, string>} */
  failures = $state({});
  /** @type {Array<{ id: string, name: string, index: number, current: boolean }>} */
  desktops = $state([]);
  /** @type {string[]} desktop files whose desktop does not exist */
  unmatched = $state([]);
  /** @type {string | null} why windows-link has no desktops, if it says */
  desktopsError = $state(null);
  /** @type {string | null} the desktop a tab is switching to */
  switching = $state(null);
  /** @type {string | null} */
  desktopFailure = $state(null);
  /**
   * Bumped whenever the window must be pinned again: on each new connection (windows-link
   * may have restarted) and when windows-link reconnects to Explorer, which forgets pins.
   */
  pinRound = $state(0);

  #base;
  #env;
  #socket = null;
  #retry = FIRST_RETRY_MS;
  #stopped = false;

  /**
   * @param {string} base windows-link URL, such as `http://127.0.0.1:4730`
   * @param {{ WebSocket?: any, fetch?: typeof fetch, setTimeout?: (fn: () => void, ms: number) => unknown, signIn?: (shop: string, base: string) => Promise<any[]> }} [env]
   */
  constructor(base, env = {}) {
    this.#base = base.replace(/\/+$/, "");
    this.#env = {
      WebSocket: env.WebSocket ?? globalThis.WebSocket,
      fetch: env.fetch ?? ((...args) => globalThis.fetch(...args)),
      setTimeout: env.setTimeout ?? ((fn, ms) => globalThis.setTimeout(fn, ms)),
      signIn: env.signIn ?? shopLogin,
    };
  }

  get base() {
    return this.#base;
  }

  start() {
    this.#stopped = false;
    this.#open();
  }

  stop() {
    this.#stopped = true;
    this.#socket?.close();
  }

  #open() {
    const socket = new this.#env.WebSocket(`${this.#base.replace(/^http/, "ws")}/events`);
    this.#socket = socket;
    socket.onopen = () => {
      this.#retry = FIRST_RETRY_MS;
    };
    socket.onmessage = (event) => this.#receive(JSON.parse(event.data));
    socket.onclose = () => {
      if (this.#stopped || socket !== this.#socket) return;
      this.status = "disconnected";
      const delay = this.#retry;
      this.#retry = Math.min(this.#retry * 2, MAX_RETRY_MS);
      this.#env.setTimeout(() => {
        if (!this.#stopped) this.#open();
      }, delay);
    };
  }

  #receive(message) {
    if (message.type === "snapshot") {
      this.buttons = message.buttons;
      this.desktops = message.desktops ?? [];
      this.unmatched = message.unmatched ?? [];
      this.desktopsError = message.desktops_error ?? null;
      this.status = "connected";
      this.pinRound += 1;
    } else if (message.type === "button") {
      this.#replace(message.button);
    } else if (message.type === "desktops") {
      this.desktops = message.desktops;
      this.unmatched = message.unmatched ?? [];
      this.desktopsError = message.error ?? null;
      this.desktopFailure = null;
      if (message.reason === "reconnected") this.pinRound += 1;
    }
  }

  #replace(button) {
    const index = this.buttons.findIndex((b) => b.id === button.id);
    if (index >= 0) this.buttons[index] = button;
  }

  /** Press a button; ignored while it is already being pressed or while disconnected. */
  async press(id) {
    if (this.pending[id] || this.status !== "connected") return;
    this.pending[id] = true;
    delete this.failures[id];
    try {
      const response = await this.#env.fetch(
        `${this.#base}/buttons/${encodeURIComponent(id)}/press`,
        { method: "POST" },
      );
      const body = await response.json().catch(() => null);
      if (response.ok && body?.button) this.#replace(body.button);
      else this.failures[id] = pressFailure(response.status, body);
    } catch {
      this.failures[id] = pressFailure(null, null);
    } finally {
      delete this.pending[id];
    }
  }

  /**
   * A library button's games.
   * @returns {Promise<{ library: any, failure: string | null }>}
   */
  /**
   * Sign in to a shop (`fanza`) in the panel's login window, then hand its cookies to
   * windows-link, which reads the games bought from then on.
   * @param {string} shop
   * @returns {Promise<string | null>} why it did not go through, or null
   */
  async signIn(shop) {
    let cookies;
    try {
      cookies = await this.#env.signIn(shop, this.#base);
    } catch (reason) {
      return signInFailure(null, reason);
    }
    try {
      const response = await this.#env.fetch(`${this.#base}/${encodeURIComponent(shop)}/session`, {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ cookies }),
      });
      if (response.ok) return null;
      return signInFailure(response.status, await response.json().catch(() => null));
    } catch {
      return signInFailure(null, null);
    }
  }

  async library(id) {
    try {
      const response = await this.#env.fetch(
        `${this.#base}/buttons/${encodeURIComponent(id)}/library`,
      );
      const body = await response.json().catch(() => null);
      if (response.ok && body?.items) return { library: body, failure: null };
      return { library: null, failure: libraryFailure(response.status, body) };
    } catch {
      return { library: null, failure: libraryFailure(null, null) };
    }
  }

  /**
   * Start a library game, or bring it to the front when it runs. Ignored while it is
   * already starting; a failure stays under `itemKey` until the next try.
   * @returns {Promise<"started" | "choose" | "failed">} "choose" when the game has
   *   several programs and none is chosen yet
   */
  async startItem(id, item) {
    const key = itemKey(id, item);
    if (this.pending[key] || this.status !== "connected") return "failed";
    this.pending[key] = true;
    delete this.failures[key];
    try {
      const response = await this.#env.fetch(
        `${this.#base}/buttons/${encodeURIComponent(id)}/library/${encodeURIComponent(item)}/start`,
        { method: "POST" },
      );
      if (response.ok) return "started";
      const body = await response.json().catch(() => null);
      if (body?.error === "choose_program") return "choose";
      this.failures[key] = libraryFailure(response.status, body);
    } catch {
      this.failures[key] = libraryFailure(null, null);
    } finally {
      delete this.pending[key];
    }
    return "failed";
  }

  /**
   * The programs a library game can start with, and the one in use.
   * @returns {Promise<{ programs: { candidates: string[], chosen: string | null } | null, failure: string | null }>}
   */
  async programs(id, item) {
    const { status, body } = await this.#call(
      "GET",
      `/buttons/${encodeURIComponent(id)}/library/${encodeURIComponent(item)}/programs`,
    );
    return status === 0
      ? { programs: body, failure: null }
      : { programs: null, failure: libraryFailure(status, body) };
  }

  /**
   * The default output's volume and the apps with sound on it.
   * @returns {Promise<{ mixer: any, failure: string | null }>}
   */
  async mixer() {
    const { status, body } = await this.#call("GET", "/audio/mixer");
    return status === 0
      ? { mixer: body, failure: null }
      : { mixer: null, failure: mixerFailure(status, body) };
  }

  /**
   * Change the default output's volume and/or mute; answers the mixer.
   * @param {{ volume?: number, muted?: boolean }} change
   * @returns {Promise<{ mixer: any, failure: string | null }>}
   */
  async setMaster(change) {
    const { status, body } = await this.#call("PUT", "/audio/master", change);
    return status === 0
      ? { mixer: body, failure: null }
      : { mixer: null, failure: mixerFailure(status, body) };
  }

  /**
   * Change one program's volume; answers the mixer.
   * @returns {Promise<{ mixer: any, failure: string | null }>}
   */
  async setAppVolume(process, volume) {
    const { status, body } = await this.#call("PUT", `/audio/apps/${encodeURIComponent(process)}`, {
      volume,
    });
    return status === 0
      ? { mixer: body, failure: null }
      : { mixer: null, failure: mixerFailure(status, body) };
  }

  /**
   * A game's license keys, read from its store each time.
   * @returns {Promise<{ keys: Array<{ label: string, value: string }> | null, failure: string | null }>}
   */
  async licenseKeys(id, item) {
    const { status, body } = await this.#call(
      "GET",
      `/buttons/${encodeURIComponent(id)}/library/${encodeURIComponent(item)}/keys`,
    );
    return status === 0
      ? { keys: body.keys, failure: null }
      : { keys: null, failure: keysFailure(status, body) };
  }

  /** Remember which program starts a game. @returns {Promise<string | null>} why it failed, if it did */
  async chooseProgram(id, item, program) {
    const { status, body } = await this.#call(
      "PUT",
      `/buttons/${encodeURIComponent(id)}/library/${encodeURIComponent(item)}/program`,
      { program },
    );
    return status === 0 ? null : libraryFailure(status, body);
  }

  /**
   * Pin a library game to its button's tab, or take it off.
   * @returns {Promise<string | null>} why it failed, if it did
   */
  async setPinned(id, item, pinned) {
    const key = itemKey(id, item);
    delete this.failures[key];
    const failed = await this.#setPinned(id, item, pinned);
    if (failed) this.failures[key] = failed;
    return failed;
  }

  async #setPinned(id, item, pinned) {
    try {
      const response = await this.#env.fetch(
        `${this.#base}/buttons/${encodeURIComponent(id)}/pins/${encodeURIComponent(item)}`,
        { method: pinned ? "PUT" : "DELETE" },
      );
      const body = await response.json().catch(() => null);
      if (response.ok && body?.button) {
        this.#replace(body.button);
        return null;
      }
      return libraryFailure(response.status, body);
    } catch {
      return libraryFailure(null, null);
    }
  }

  /**
   * One request to windows-link that answers with JSON or nothing.
   * @param {string} method
   * @param {string} path
   * @param {any} [body] sent as JSON
   * @returns {Promise<{ status: number | null, body: any }>} status null when nothing answered
   */
  async #call(method, path, body) {
    try {
      const response = await this.#env.fetch(`${this.#base}${path}`, {
        method,
        ...(body === undefined
          ? {}
          : { headers: { "content-type": "application/json" }, body: JSON.stringify(body) }),
      });
      const answer = await response.json().catch(() => null);
      return { status: response.ok ? 0 : response.status, body: answer };
    } catch {
      return { status: null, body: null };
    }
  }

  /**
   * Show an installed library game's folder in Explorer.
   * @returns {Promise<string | null>} why it failed, if it did
   */
  async openFolder(id, item) {
    const { status, body } = await this.#call(
      "POST",
      `/buttons/${encodeURIComponent(id)}/library/${encodeURIComponent(item)}/folder`,
    );
    if (status === 0) return null;
    return body?.error === "not_found"
      ? "このゲームはインストールされていません"
      : libraryFailure(status, body);
  }

  /**
   * Make a label in a library.
   * @returns {Promise<{ label: any, failure: string | null }>}
   */
  async createLabel(id, name) {
    const { status, body } = await this.#call("POST", `/buttons/${encodeURIComponent(id)}/labels`, {
      name,
    });
    return status === 0
      ? { label: body?.label ?? null, failure: null }
      : { label: null, failure: labelFailure(status, body) };
  }

  /** @returns {Promise<string | null>} why it failed, if it did */
  async renameLabel(id, label, name) {
    const { status, body } = await this.#call(
      "PATCH",
      `/buttons/${encodeURIComponent(id)}/labels/${encodeURIComponent(label)}`,
      { name },
    );
    return status === 0 ? null : labelFailure(status, body);
  }

  /** Delete a label; its games stay. @returns {Promise<string | null>} why it failed, if it did */
  async deleteLabel(id, label) {
    const { status, body } = await this.#call(
      "DELETE",
      `/buttons/${encodeURIComponent(id)}/labels/${encodeURIComponent(label)}`,
    );
    return status === 0 ? null : labelFailure(status, body);
  }

  /** Put a game in a label or take it out. @returns {Promise<string | null>} why it failed, if it did */
  async setLabel(id, label, item, on) {
    const { status, body } = await this.#call(
      on ? "PUT" : "DELETE",
      `/buttons/${encodeURIComponent(id)}/labels/${encodeURIComponent(label)}/items/${encodeURIComponent(item)}`,
    );
    return status === 0 ? null : labelFailure(status, body);
  }

  /** Ask windows-link to switch Windows to a desktop; one switch at a time. */
  async switchDesktop(id) {
    if (this.switching || this.status !== "connected") return;
    this.switching = id;
    this.desktopFailure = null;
    try {
      const response = await this.#env.fetch(
        `${this.#base}/desktops/${encodeURIComponent(id)}/switch`,
        { method: "POST" },
      );
      const body = await response.json().catch(() => null);
      if (response.ok && body?.desktops) this.desktops = body.desktops;
      else this.desktopFailure = switchFailure(response.status, body);
    } catch {
      this.desktopFailure = switchFailure(null, null);
    } finally {
      this.switching = null;
    }
  }

  /**
   * Show a window on every virtual desktop.
   * @param {number} hwnd
   * @returns {Promise<boolean>} whether windows-link pinned it
   */
  async pin(hwnd) {
    try {
      const response = await this.#env.fetch(`${this.#base}/windows/${hwnd}/pin`, {
        method: "POST",
      });
      return response.ok;
    } catch {
      return false;
    }
  }

  /** Create a desktop for a desktop file that has none; Windows switches to it. */
  async createDesktop(name) {
    if (this.status !== "connected") return;
    this.desktopFailure = null;
    try {
      const response = await this.#env.fetch(`${this.#base}/desktops`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ name }),
      });
      const body = await response.json().catch(() => null);
      if (response.ok && body?.desktops) {
        this.desktops = body.desktops;
        this.unmatched = body.unmatched ?? [];
      } else if (body?.error === "exists") {
        this.desktopFailure = `「${name}」はもうあります`;
      } else {
        this.desktopFailure = switchFailure(response.status, body);
      }
    } catch {
      this.desktopFailure = switchFailure(null, null);
    }
  }

  /**
   * Put the PC to sleep.
   * @returns {Promise<boolean>} whether windows-link accepted
   */
  async sleep() {
    try {
      const response = await this.#env.fetch(`${this.#base}/power/sleep`, { method: "POST" });
      return response.ok;
    } catch {
      return false;
    }
  }
}

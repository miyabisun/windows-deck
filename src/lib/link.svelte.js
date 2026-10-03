// Connection to windows-link: the button list and states from the `/events` socket,
// presses over HTTP, and automatic reconnection.

import { pressFailure, switchFailure } from "./display.js";

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
   * @param {{ WebSocket?: any, fetch?: typeof fetch, setTimeout?: (fn: () => void, ms: number) => unknown }} [env]
   */
  constructor(base, env = {}) {
    this.#base = base.replace(/\/+$/, "");
    this.#env = {
      WebSocket: env.WebSocket ?? globalThis.WebSocket,
      fetch: env.fetch ?? ((...args) => globalThis.fetch(...args)),
      setTimeout: env.setTimeout ?? ((fn, ms) => globalThis.setTimeout(fn, ms)),
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
      this.desktopsError = message.desktops_error ?? null;
      this.status = "connected";
      this.pinRound += 1;
    } else if (message.type === "button") {
      this.#replace(message.button);
    } else if (message.type === "desktops") {
      this.desktops = message.desktops;
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
}

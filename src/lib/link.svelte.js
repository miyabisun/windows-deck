// Connection to windows-link: the button list and states from the `/events` socket,
// presses over HTTP, and automatic reconnection.

import { pressFailure } from "./display.js";

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
      this.status = "connected";
    } else if (message.type === "button") {
      this.#replace(message.button);
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
}

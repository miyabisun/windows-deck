import { beforeEach, describe, expect, it } from "vitest";
import { Link } from "./link.svelte.js";

class FakeSocket {
  static all = [];
  /** @type {(() => void) | null} */
  onopen = null;
  /** @type {((event: { data: string }) => void) | null} */
  onmessage = null;
  /** @type {(() => void) | null} */
  onclose = null;
  constructor(url) {
    this.url = url;
    this.closed = false;
    FakeSocket.all.push(this);
  }
  open() {
    this.onopen?.();
  }
  send(message) {
    this.onmessage?.({ data: JSON.stringify(message) });
  }
  drop() {
    this.onclose?.();
  }
  close() {
    this.closed = true;
  }
}

const output = (current) => ({
  id: "output",
  type: "audio.output_toggle",
  label: "出力切替",
  state: { kind: "output", current, current_name: current, options: [] },
});

function setup() {
  FakeSocket.all = [];
  const timers = [];
  const requests = [];
  let respond;
  const link = new Link("http://127.0.0.1:4730", {
    WebSocket: FakeSocket,
    fetch: (url, init) => {
      requests.push({ url, init });
      return new Promise((resolve, reject) => (respond = { resolve, reject }));
    },
    setTimeout: (fn, ms) => timers.push({ fn, ms }),
  });
  const reply = (status, body) =>
    respond.resolve({ ok: status < 400, status, json: async () => body });
  const fail = () => respond.reject(new TypeError("Failed to fetch"));
  return { link, timers, requests, reply, fail, socket: () => FakeSocket.all.at(-1) };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("Link", () => {
  let t;
  beforeEach(() => {
    t = setup();
    t.link.start();
  });

  it("connects to the events socket and takes the buttons from the snapshot", () => {
    expect(t.socket().url).toBe("ws://127.0.0.1:4730/events");
    expect(t.link.status).toBe("connecting");
    t.socket().open();
    t.socket().send({ type: "snapshot", buttons: [output("motu")] });
    expect(t.link.status).toBe("connected");
    expect(t.link.buttons.map((b) => b.state.current)).toEqual(["motu"]);
  });

  it("applies button events and ignores other messages", () => {
    t.socket().send({ type: "snapshot", buttons: [output("motu")] });
    t.socket().send({ type: "button", button: output("jbl") });
    t.socket().send({ type: "desktops", desktops: [] });
    expect(t.link.buttons.map((b) => b.state.current)).toEqual(["jbl"]);
  });

  it("presses a button, shows it as pending and takes the new state from the answer", async () => {
    t.socket().send({ type: "snapshot", buttons: [output("motu")] });
    const pressed = t.link.press("output");
    expect(t.requests[0].url).toBe("http://127.0.0.1:4730/buttons/output/press");
    expect(t.requests[0].init.method).toBe("POST");
    expect(t.link.pending.output).toBe(true);

    t.link.press("output");
    expect(t.requests).toHaveLength(1);

    t.reply(200, { button: output("jbl") });
    await pressed;
    expect(t.link.pending.output).toBeUndefined();
    expect(t.link.buttons[0].state.current).toBe("jbl");
  });

  it("keeps a failed press's reason on the button until it is pressed again", async () => {
    t.socket().send({ type: "snapshot", buttons: [output("motu")] });
    let pressed = t.link.press("output");
    t.reply(409, { error: "device_unavailable", message: "jbl is not connected" });
    await pressed;
    expect(t.link.failures.output).toBe("切替先のデバイスがつながっていません");
    expect(t.link.buttons[0].state.current).toBe("motu");

    pressed = t.link.press("output");
    expect(t.link.failures.output).toBeUndefined();
    t.fail();
    await pressed;
    expect(t.link.failures.output).toBe("windows-link に届きませんでした");
  });

  it("reconnects with a growing delay and does not press while disconnected", async () => {
    t.socket().open();
    t.socket().send({ type: "snapshot", buttons: [output("motu")] });
    t.socket().drop();
    expect(t.link.status).toBe("disconnected");
    expect(t.link.buttons).toHaveLength(1);

    await t.link.press("output");
    expect(t.requests).toHaveLength(0);

    const delays = [];
    for (let i = 0; i < 5; i++) {
      const timer = t.timers.shift();
      delays.push(timer.ms);
      timer.fn();
      t.socket().drop();
    }
    expect(delays).toEqual([1000, 2000, 4000, 5000, 5000]);

    t.timers.shift().fn();
    t.socket().open();
    t.socket().send({ type: "snapshot", buttons: [output("jbl")] });
    expect(t.link.status).toBe("connected");
    t.socket().drop();
    expect(t.timers.shift().ms).toBe(1000);
  });

  it("stops reconnecting once stopped", async () => {
    const socket = t.socket();
    t.link.stop();
    expect(socket.closed).toBe(true);
    socket.drop();
    await settle();
    expect(t.timers).toHaveLength(0);
  });
});

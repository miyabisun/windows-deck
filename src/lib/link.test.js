import { beforeEach, describe, expect, it } from "vitest";
import { Link, itemKey } from "./link.svelte.js";

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

const desktops = (current) =>
  ["dev", "ゲーム", "ブルアカ"].map((name, index) => ({
    id: `GUID-${index}`,
    name,
    index,
    current: index === current,
  }));

describe("Link desktops", () => {
  let t;
  beforeEach(() => {
    t = setup();
    t.link.start();
    t.socket().open();
  });

  it("takes the desktops from the snapshot and follows desktop events", () => {
    t.socket().send({ type: "snapshot", buttons: [], desktops: desktops(0), desktops_error: null });
    expect(t.link.desktops.map((d) => d.name)).toEqual(["dev", "ゲーム", "ブルアカ"]);
    expect(t.link.pinRound).toBe(1);

    t.socket().send({ type: "desktops", reason: "changed", desktops: desktops(2), error: null });
    expect(t.link.desktops.find((d) => d.current).name).toBe("ブルアカ");
  });

  it("has no desktops when windows-link cannot reach them", () => {
    t.socket().send({
      type: "snapshot",
      buttons: [],
      desktops: [],
      desktops_error: "virtual desktops are not enabled",
    });
    expect(t.link.desktops).toEqual([]);
    expect(t.link.desktopsError).toBe("virtual desktops are not enabled");
  });

  it("asks for the window to be pinned again after reconnecting or an Explorer restart", () => {
    t.socket().send({ type: "snapshot", buttons: [], desktops: [] });
    t.socket().drop();
    t.timers.shift().fn();
    t.socket().open();
    t.socket().send({ type: "snapshot", buttons: [], desktops: [] });
    expect(t.link.pinRound).toBe(2);
    t.socket().send({ type: "desktops", reason: "changed", desktops: desktops(1), error: null });
    expect(t.link.pinRound).toBe(2);
    t.socket().send({
      type: "desktops",
      reason: "reconnected",
      desktops: desktops(1),
      error: null,
    });
    expect(t.link.pinRound).toBe(3);
  });

  it("asks windows-link to switch and shows the answer", async () => {
    t.socket().send({ type: "snapshot", buttons: [], desktops: desktops(0) });
    const switched = t.link.switchDesktop("GUID-1");
    expect(t.requests[0].url).toBe("http://127.0.0.1:4730/desktops/GUID-1/switch");
    expect(t.requests[0].init.method).toBe("POST");
    expect(t.link.switching).toBe("GUID-1");
    t.link.switchDesktop("GUID-2");
    expect(t.requests).toHaveLength(1);

    t.reply(200, { desktops: desktops(1), error: null });
    await switched;
    expect(t.link.switching).toBeNull();
    expect(t.link.desktops.find((d) => d.current).id).toBe("GUID-1");
  });

  it("says why a switch failed until the desktops change", async () => {
    t.socket().send({ type: "snapshot", buttons: [], desktops: desktops(0) });
    let switched = t.link.switchDesktop("GUID-9");
    t.reply(404, { error: "not_found", message: "no desktop with this id" });
    await switched;
    expect(t.link.desktopFailure).toBe("このデスクトップはもうありません");

    t.socket().send({ type: "desktops", reason: "removed", desktops: desktops(0), error: null });
    expect(t.link.desktopFailure).toBeNull();

    switched = t.link.switchDesktop("GUID-1");
    t.fail();
    await switched;
    expect(t.link.desktopFailure).toBe("windows-link に届きませんでした");
  });

  it("pins a window and reports whether it worked", async () => {
    t.socket().send({ type: "snapshot", buttons: [], desktops: desktops(0) });
    let pinned = t.link.pin(4723016);
    expect(t.requests[0].url).toBe("http://127.0.0.1:4730/windows/4723016/pin");
    expect(t.requests[0].init.method).toBe("POST");
    t.reply(200, { pinned: true });
    expect(await pinned).toBe(true);

    pinned = t.link.pin(4723016);
    t.reply(503, { error: "desktops", message: "not enabled" });
    expect(await pinned).toBe(false);
  });
});

describe("Link desktop files and sleep", () => {
  let t;
  beforeEach(() => {
    t = setup();
    t.link.start();
    t.socket().open();
  });

  it("knows which desktop files have no desktop yet", () => {
    t.socket().send({ type: "snapshot", buttons: [], desktops: desktops(0), unmatched: ["SF6"] });
    expect(t.link.unmatched).toEqual(["SF6"]);
    t.socket().send({ type: "desktops", reason: "created", desktops: desktops(0), unmatched: [] });
    expect(t.link.unmatched).toEqual([]);
  });

  it("creates a desktop for a desktop file and takes the new list", async () => {
    t.socket().send({ type: "snapshot", buttons: [], desktops: desktops(0), unmatched: ["SF6"] });
    const created = t.link.createDesktop("SF6");
    expect(t.requests[0].url).toBe("http://127.0.0.1:4730/desktops");
    expect(t.requests[0].init.method).toBe("POST");
    expect(JSON.parse(t.requests[0].init.body)).toEqual({ name: "SF6" });
    const list = [...desktops(-1), { id: "GUID-3", name: "SF6", index: 3, current: true }];
    t.reply(201, { desktops: list, unmatched: [], error: null });
    await created;
    expect(t.link.desktops.find((d) => d.current).name).toBe("SF6");
    expect(t.link.unmatched).toEqual([]);
  });

  it("says why a desktop could not be created", async () => {
    t.socket().send({ type: "snapshot", buttons: [], desktops: desktops(0), unmatched: ["SF6"] });
    const created = t.link.createDesktop("SF6");
    t.reply(409, { error: "exists", message: "a desktop with this name already exists" });
    await created;
    expect(t.link.desktopFailure).toBe("「SF6」はもうあります");
  });

  it("asks windows-link to put the PC to sleep", async () => {
    t.socket().send({ type: "snapshot", buttons: [], desktops: [] });
    const slept = t.link.sleep();
    expect(t.requests[0].url).toBe("http://127.0.0.1:4730/power/sleep");
    expect(t.requests[0].init.method).toBe("POST");
    t.reply(202, { sleeping: true });
    expect(await slept).toBe(true);
  });
});

describe("Link libraries", () => {
  let t;
  const library = (pins) => ({
    id: "games",
    type: "steam.library",
    label: "ゲーム検索",
    state: { kind: "library", pins },
  });
  const sf6 = { id: "1364780", name: "Street Fighter 6" };
  beforeEach(() => {
    t = setup();
    t.link.start();
    t.socket().send({ type: "snapshot", buttons: [library([])], desktops: [] });
  });

  it("reads a library button's games", async () => {
    const read = t.link.library("games");
    expect(t.requests[0].url).toBe("http://127.0.0.1:4730/buttons/games/library");
    const body = { items: [], labels: [], hide: [], partial: null };
    t.reply(200, body);
    expect(await read).toEqual({ library: body, failure: null });

    const failed = t.link.library("games");
    t.fail();
    expect(await failed).toEqual({ library: null, failure: "windows-link に届きませんでした" });
  });

  it("starts a game, showing it as pending, and keeps a failure on it", async () => {
    let started = t.link.startItem("games", "1364780");
    expect(t.requests[0].url).toBe("http://127.0.0.1:4730/buttons/games/library/1364780/start");
    expect(t.requests[0].init.method).toBe("POST");
    expect(t.link.pending[itemKey("games", "1364780")]).toBe(true);
    t.reply(204, null);
    expect(await started).toBe(true);
    expect(t.link.pending[itemKey("games", "1364780")]).toBeUndefined();

    started = t.link.startItem("games", "1364780");
    t.reply(500, { error: "launch", message: "denied" });
    expect(await started).toBe(false);
    expect(t.link.failures[itemKey("games", "1364780")]).toBe("起動できませんでした: denied");
  });

  it("pins and unpins a game and takes the button's new state", async () => {
    const pinned = t.link.setPinned("games", "1364780", true);
    expect(t.requests[0].url).toBe("http://127.0.0.1:4730/buttons/games/pins/1364780");
    expect(t.requests[0].init.method).toBe("PUT");
    t.reply(200, { button: library([sf6]) });
    expect(await pinned).toBeNull();
    expect(t.link.buttons[0].state.pins).toEqual([sf6]);

    const unpinned = t.link.setPinned("games", "1364780", false);
    expect(t.requests[1].init.method).toBe("DELETE");
    t.reply(404, { error: "not_found", message: "no such game" });
    expect(await unpinned).toBe("ライブラリにこのゲームがありません（一覧を開き直してください）");
  });

  it("opens a game's folder", async () => {
    const opened = t.link.openFolder("games", "1364780");
    expect(t.requests[0].url).toBe("http://127.0.0.1:4730/buttons/games/library/1364780/folder");
    expect(t.requests[0].init.method).toBe("POST");
    t.reply(204, null);
    expect(await opened).toBeNull();

    const missing = t.link.openFolder("games", "2");
    t.reply(404, { error: "not_found", message: "the game is not installed" });
    expect(await missing).toBe("このゲームはインストールされていません");
  });

  it("creates, renames and deletes labels", async () => {
    const created = t.link.createLabel("games", "RPG");
    expect(t.requests[0].url).toBe("http://127.0.0.1:4730/buttons/games/labels");
    expect(t.requests[0].init.method).toBe("POST");
    expect(JSON.parse(t.requests[0].init.body)).toEqual({ name: "RPG" });
    const label = { id: "uc-9", name: "RPG", editable: true };
    t.reply(201, { label });
    expect(await created).toEqual({ label, failure: null });

    const renamed = t.link.renameLabel("games", "uc-9", "JRPG");
    expect(t.requests[1].url).toBe("http://127.0.0.1:4730/buttons/games/labels/uc-9");
    expect(t.requests[1].init.method).toBe("PATCH");
    expect(JSON.parse(t.requests[1].init.body)).toEqual({ name: "JRPG" });
    t.reply(204, null);
    expect(await renamed).toBeNull();

    const deleted = t.link.deleteLabel("games", "uc-9");
    expect(t.requests[2].init.method).toBe("DELETE");
    t.reply(409, { error: "labels_unavailable", message: "Steam が起動していません" });
    expect(await deleted).toBe("ラベルを変更できません: Steam が起動していません");
  });

  it("puts a game in a label and takes it out", async () => {
    const added = t.link.setLabel("games", "hidden", "1364780", true);
    expect(t.requests[0].url).toBe(
      "http://127.0.0.1:4730/buttons/games/labels/hidden/items/1364780",
    );
    expect(t.requests[0].init.method).toBe("PUT");
    t.reply(204, null);
    expect(await added).toBeNull();

    const removed = t.link.setLabel("games", "hidden", "1364780", false);
    expect(t.requests[1].init.method).toBe("DELETE");
    t.fail();
    expect(await removed).toBe("windows-link に届きませんでした");
  });
});

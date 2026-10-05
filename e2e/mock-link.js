// A stand-in for windows-link: the button API and `/events`, with switches for tests.
import http from "node:http";
import { WebSocketServer } from "ws";

const output = (current) => ({
  id: "output",
  type: "audio.output_toggle",
  label: "出力切替",
  desktop: null,
  state: {
    kind: "output",
    current,
    current_name: current === "motu" ? "MOTU (MOTU M Series)" : "BTイヤホン (JBL Tour Pro 3)",
    options: [
      { alias: "motu", name: "MOTU (MOTU M Series)", connected: true },
      { alias: "jbl", name: "BTイヤホン (JBL Tour Pro 3)", connected: true },
    ],
  },
});

const volume = (value) => ({
  id: "sf6-volume",
  type: "audio.app_volume_toggle",
  label: "スト6 音量",
  desktop: null,
  state: { kind: "volume", running: true, volume: value, levels: [0.2, 1] },
});

export function initialDesktops(current = 0) {
  return ["dev", "ゲーム", "ブルアカ", "アダルト"].map((name, index) => ({
    id: `GUID-${index}`,
    name,
    index,
    current: index === current,
  }));
}

/** A `steam.library` button with its pins. */
export const libraryButton = (pins = []) => ({
  id: "games",
  type: "steam.library",
  label: "ゲーム検索",
  desktop: null,
  except: [],
  icon: false,
  state: { kind: "library", pins },
});

/** The default output's volume and the apps with sound, as `GET /audio/mixer` gives them. */
export const initialMixer = () => ({
  master: { volume: 0.45, muted: false },
  apps: [
    { process: "Discord.exe", name: "Discord", volume: 1 },
    { process: "StreetFighter6.exe", name: "StreetFighter6", volume: 0.2 },
  ],
});

/** The mute button and the mixer button, following `master`. */
export const muteButton = (master) => ({
  id: "mute",
  type: "audio.mute_toggle",
  label: "ミュート",
  desktop: null,
  except: [],
  icon: false,
  state: { kind: "mute", ...master },
});
export const mixerButton = (master) => ({
  id: "mixer",
  type: "audio.mixer",
  label: "ミキサー",
  desktop: null,
  except: [],
  icon: false,
  state: { kind: "mixer", ...master },
});

/** A `dlsite.library` button, whose pictures are program icons. */
export const dlsiteButton = (pins = []) => ({
  id: "dlsite",
  type: "dlsite.library",
  label: "DLsite",
  desktop: null,
  except: [],
  icon: false,
  state: { kind: "library", pins, pictures: "whole", license_keys: true },
});

/**
 * The games behind `dlsiteButton`: "a1" starts, "b2" has two programs to choose from,
 * "c3" has none.
 */
export function initialDlsite() {
  const game = (id, name, detail, choosable = false, image = null) => ({
    id,
    name,
    detail,
    choosable,
    image,
    installed: true,
    labels: [],
  });
  return {
    items: [
      game("a1", "湿度の高い夏のマゾ", "3Djp_Art", false, "/dlsite-art/a1.svg"),
      game("b2", "催眠アプリ", "Saimin Soft", true),
      game("c3", "壊れたゲーム", "Other"),
    ],
    labels: [
      { id: "favorite", name: "お気に入り", editable: false },
      { id: "hidden", name: "非表示", editable: false },
    ],
    hide: ["hidden"],
    partial: null,
    labels_locked: null,
    programs: { b2: { candidates: ["app.exe", "startup.exe"], chosen: null } },
    keys: { a1: [{ label: "ライセンスキー", value: "ABCD-1234-EFGH-5678" }] },
  };
}

/** The games behind `libraryButton`; labels name their games by ID. */
export function initialLibrary() {
  const game = (id, name, labels, installed = true) => ({ id, name, installed, labels });
  return {
    items: [
      game("1364780", "Street Fighter™ 6", ["favorite"]),
      game("646570", "Slay the Spire", []),
      game("2868840", "Slay the Spire 2", ["uc-r15"], false),
      game("1039890", "METAL SLUG", ["hidden"]),
    ],
    labels: [
      { id: "favorite", name: "お気に入り", editable: false },
      { id: "hidden", name: "非表示", editable: false },
      { id: "uc-r15", name: "R15", editable: true },
    ],
    hide: ["hidden"],
    partial: null,
    labels_locked: null,
  };
}

export function initialButtons() {
  return [output("motu"), volume(1)];
}

async function readJson(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  return JSON.parse(Buffer.concat(chunks).toString("utf8"));
}

export async function startMockLink() {
  const link = {
    buttons: initialButtons(),
    presses: [],
    /** @type {{ status: number, body: any } | null} */
    nextFailure: null,
    delayMs: 0,
    down: false,
    desktops: initialDesktops(),
    /** @type {string | null} */
    desktopsError: null,
    /** @type {string[]} desktop files without a desktop */
    unmatched: [],
    sleeps: 0,
    /** @type {string[]} */
    switches: [],
    /** @type {string[]} */
    pins: [],
    library: initialLibrary(),
    dlsite: initialDlsite(),
    mixer: initialMixer(),
    /** @type {string[]} games started from a library */
    started: [],
    /** @type {string[]} games whose folder was opened */
    folders: [],
  };
  const sockets = new Set();
  const broadcast = (message) => {
    for (const socket of sockets) socket.send(JSON.stringify(message));
  };

  const server = http.createServer(async (req, res) => {
    res.setHeader("Access-Control-Allow-Origin", "*");
    if (req.method === "OPTIONS") {
      res.setHeader("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE");
      res.setHeader("Access-Control-Allow-Headers", "content-type");
      res.statusCode = 204;
      return res.end();
    }
    const sw = req.url.match(/^\/desktops\/([^/]+)\/switch$/);
    if (req.method === "POST" && sw && !link.down) {
      const id = decodeURIComponent(sw[1]);
      link.switches.push(id);
      res.setHeader("Content-Type", "application/json");
      if (!link.desktops.some((d) => d.id === id)) {
        res.statusCode = 404;
        return res.end(JSON.stringify({ error: "not_found", message: "no desktop with this id" }));
      }
      link.desktops = link.desktops.map((d) => ({ ...d, current: d.id === id }));
      broadcast({ type: "desktops", reason: "changed", desktops: link.desktops, error: null });
      return res.end(JSON.stringify({ desktops: link.desktops, error: null }));
    }
    if (req.method === "POST" && req.url === "/desktops" && !link.down) {
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      const { name } = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      res.setHeader("Content-Type", "application/json");
      if (link.desktops.some((d) => d.name === name)) {
        res.statusCode = 409;
        return res.end(JSON.stringify({ error: "exists", message: "exists" }));
      }
      link.desktops = [
        ...link.desktops.map((d) => ({ ...d, current: false })),
        { id: `GUID-${link.desktops.length}`, name, index: link.desktops.length, current: true },
      ];
      link.unmatched = link.unmatched.filter((n) => n !== name);
      broadcast({
        type: "desktops",
        reason: "created",
        desktops: link.desktops,
        unmatched: link.unmatched,
        error: null,
      });
      res.statusCode = 201;
      return res.end(
        JSON.stringify({ desktops: link.desktops, unmatched: link.unmatched, error: null }),
      );
    }
    if (req.method === "POST" && req.url === "/power/sleep") {
      link.sleeps += 1;
      res.statusCode = 202;
      res.setHeader("Content-Type", "application/json");
      return res.end(JSON.stringify({ sleeping: true }));
    }
    if (req.method === "GET" && req.url.startsWith("/dlsite-art/")) {
      res.setHeader("Content-Type", "image/svg+xml");
      return res.end(
        '<svg xmlns="http://www.w3.org/2000/svg" width="560" height="420"><rect width="560" height="420" fill="#2e86c1"/></svg>',
      );
    }
    const dl = req.url.match(
      /^\/buttons\/dlsite\/library(?:\/([^/]+)\/(start|programs|program|image|keys))?$/,
    );
    if (dl && !link.down) {
      const [, item, action] = dl;
      const { programs, keys, ...listing } = link.dlsite;
      const json = (status, body) => {
        res.statusCode = status;
        res.setHeader("Content-Type", "application/json");
        return res.end(body === undefined ? undefined : JSON.stringify(body));
      };
      if (!item)
        return json(200, {
          ...listing,
          items: listing.items.map((i) => ({ ...i, pinned: false })),
        });
      if (action === "image") {
        res.setHeader("Content-Type", "image/svg+xml");
        return res.end(
          '<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256"><circle cx="128" cy="128" r="120" fill="#c0392b"/></svg>',
        );
      }
      if (action === "keys") return json(200, { keys: keys[item] ?? [] });
      if (action === "programs")
        return programs[item] ? json(200, programs[item]) : json(404, { error: "not_found" });
      if (action === "program") {
        const { program } = await readJson(req);
        programs[item].chosen = program;
        res.statusCode = 204;
        return res.end();
      }
      if (item === "c3")
        return json(409, { error: "no_program", message: "it has no program to start" });
      const coming = listing.items.find((i) => i.id === item && !i.installed);
      if (coming) return json(409, { error: "not_downloaded", message: coming.status });
      if (programs[item] && !programs[item].chosen)
        return json(409, { error: "choose_program", message: "choose which program starts it" });
      link.started.push(programs[item] ? `${item}:${programs[item].chosen}` : item);
      res.statusCode = 204;
      return res.end();
    }
    const picture = req.url.match(/^\/buttons\/games\/library\/(\d+)\/image$/);
    if (req.method === "GET" && picture) {
      res.setHeader("Content-Type", "image/svg+xml");
      return res.end(
        '<svg xmlns="http://www.w3.org/2000/svg" width="460" height="215"><rect width="460" height="215" fill="#3b6ea8"/></svg>',
      );
    }
    if (req.method === "GET" && req.url === "/buttons/games/library" && !link.down) {
      const pinned = new Set(
        link.buttons.find((b) => b.id === "games").state.pins.map((p) => p.id),
      );
      res.setHeader("Content-Type", "application/json");
      return res.end(
        JSON.stringify({
          ...link.library,
          items: link.library.items.map((i) => ({ ...i, pinned: pinned.has(i.id) })),
        }),
      );
    }
    const start = req.url.match(/^\/buttons\/games\/library\/([^/]+)\/start$/);
    if (req.method === "POST" && start && !link.down) {
      link.started.push(start[1]);
      res.statusCode = 204;
      return res.end();
    }
    const pinItem = req.url.match(/^\/buttons\/games\/pins\/([^/]+)$/);
    if ((req.method === "PUT" || req.method === "DELETE") && pinItem && !link.down) {
      const index = link.buttons.findIndex((b) => b.id === "games");
      const item = link.library.items.find((i) => i.id === pinItem[1]);
      res.setHeader("Content-Type", "application/json");
      if (!item) {
        res.statusCode = 404;
        return res.end(JSON.stringify({ error: "not_found", message: "no such game" }));
      }
      const others = link.buttons[index].state.pins.filter((p) => p.id !== item.id);
      const pins = req.method === "PUT" ? [...others, { id: item.id, name: item.name }] : others;
      link.buttons[index] = libraryButton(pins);
      broadcast({ type: "button", button: link.buttons[index] });
      return res.end(JSON.stringify({ button: link.buttons[index] }));
    }
    const folder = req.url.match(/^\/buttons\/games\/library\/([^/]+)\/folder$/);
    if (req.method === "POST" && folder && !link.down) {
      const item = link.library.items.find((i) => i.id === folder[1]);
      if (!item?.installed) {
        res.statusCode = 404;
        res.setHeader("Content-Type", "application/json");
        return res.end(JSON.stringify({ error: "not_found", message: "not installed" }));
      }
      link.folders.push(item.id);
      res.statusCode = 204;
      return res.end();
    }
    if (req.method === "POST" && req.url === "/buttons/games/labels" && !link.down) {
      const { name } = await readJson(req);
      const label = { id: `uc-${link.library.labels.length + 1}`, name, editable: true };
      link.library.labels.push(label);
      res.statusCode = 201;
      res.setHeader("Content-Type", "application/json");
      return res.end(JSON.stringify({ label }));
    }
    const label = req.url.match(/^\/buttons\/games\/labels\/([^/]+)$/);
    if (label && !link.down && (req.method === "PATCH" || req.method === "DELETE")) {
      const id = decodeURIComponent(label[1]);
      if (req.method === "PATCH") {
        const { name } = await readJson(req);
        link.library.labels.find((l) => l.id === id).name = name;
      } else {
        link.library.labels = link.library.labels.filter((l) => l.id !== id);
        for (const item of link.library.items) item.labels = item.labels.filter((l) => l !== id);
      }
      res.statusCode = 204;
      return res.end();
    }
    const member = req.url.match(/^\/buttons\/games\/labels\/([^/]+)\/items\/([^/]+)$/);
    if (member && !link.down && (req.method === "PUT" || req.method === "DELETE")) {
      const id = decodeURIComponent(member[1]);
      const item = link.library.items.find((i) => i.id === member[2]);
      item.labels = item.labels.filter((l) => l !== id);
      if (req.method === "PUT") item.labels.push(id);
      res.statusCode = 204;
      return res.end();
    }
    const icon = req.url.match(/^\/buttons\/([^/]+)\/icon$/);
    if (req.method === "GET" && icon) {
      res.setHeader("Content-Type", "image/svg+xml");
      return res.end(
        '<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="#e0a800"/></svg>',
      );
    }
    const pin = req.url.match(/^\/windows\/([^/]+)\/pin$/);
    if (req.method === "POST" && pin && !link.down) {
      link.pins.push(pin[1]);
      res.setHeader("Content-Type", "application/json");
      return res.end(JSON.stringify({ pinned: true }));
    }
    // The mute and mixer buttons follow the master volume.
    const masterChanged = () => {
      for (const [index, button] of link.buttons.entries()) {
        const next =
          button.id === "mute"
            ? muteButton(link.mixer.master)
            : button.id === "mixer"
              ? mixerButton(link.mixer.master)
              : null;
        if (next) {
          link.buttons[index] = next;
          broadcast({ type: "button", button: next });
        }
      }
    };
    if (req.url === "/audio/mixer" && req.method === "GET" && !link.down) {
      res.setHeader("Content-Type", "application/json");
      return res.end(JSON.stringify(link.mixer));
    }
    if (req.url === "/audio/master" && req.method === "PUT" && !link.down) {
      const change = await readJson(req);
      if (change.volume !== undefined) {
        link.mixer.master.volume = change.volume;
        link.mixer.master.muted = false;
      }
      if (change.muted !== undefined) link.mixer.master.muted = change.muted;
      masterChanged();
      res.setHeader("Content-Type", "application/json");
      return res.end(JSON.stringify(link.mixer));
    }
    const appVolume = req.url.match(/^\/audio\/apps\/([^/]+)$/);
    if (appVolume && req.method === "PUT" && !link.down) {
      const { volume } = await readJson(req);
      const app = link.mixer.apps.find((a) => a.process === decodeURIComponent(appVolume[1]));
      res.setHeader("Content-Type", "application/json");
      if (!app) {
        res.statusCode = 404;
        return res.end(JSON.stringify({ error: "not_found", message: "no sound" }));
      }
      app.volume = volume;
      return res.end(JSON.stringify(link.mixer));
    }
    if (req.url === "/buttons/mute/press" && req.method === "POST" && !link.down) {
      link.presses.push("mute");
      link.mixer.master.muted = !link.mixer.master.muted;
      masterChanged();
      res.setHeader("Content-Type", "application/json");
      return res.end(JSON.stringify({ button: link.buttons.find((b) => b.id === "mute") }));
    }
    const press = req.url.match(/^\/buttons\/([^/]+)\/press$/);
    if (req.method === "POST" && press && !link.down) {
      const id = decodeURIComponent(press[1]);
      link.presses.push(id);
      if (link.delayMs) await new Promise((r) => setTimeout(r, link.delayMs));
      res.setHeader("Content-Type", "application/json");
      if (link.nextFailure) {
        const { status, body } = link.nextFailure;
        link.nextFailure = null;
        res.statusCode = status;
        return res.end(JSON.stringify(body));
      }
      const index = link.buttons.findIndex((b) => b.id === id);
      if (index < 0) {
        res.statusCode = 404;
        return res.end(JSON.stringify({ error: "not_found", message: "no button" }));
      }
      const button = link.buttons[index];
      link.buttons[index] =
        button.state.kind === "voice"
          ? { ...button, state: { ...button.state, joined: !button.state.joined } }
          : button.id === "output"
            ? output(button.state.current === "motu" ? "jbl" : "motu")
            : volume(button.state.volume > 0.6 ? 0.2 : 1);
      broadcast({ type: "button", button: link.buttons[index] });
      return res.end(JSON.stringify({ button: link.buttons[index] }));
    }
    res.statusCode = 404;
    res.end();
  });

  const wss = new WebSocketServer({ noServer: true });
  server.on("upgrade", (req, socket, head) => {
    if (link.down || req.url !== "/events") return socket.destroy();
    wss.handleUpgrade(req, socket, head, (ws) => {
      sockets.add(ws);
      ws.on("close", () => sockets.delete(ws));
      ws.send(
        JSON.stringify({
          type: "snapshot",
          buttons: link.buttons,
          desktops: link.desktops,
          unmatched: link.unmatched,
          desktops_error: link.desktopsError,
        }),
      );
    });
  });

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = /** @type {import("node:net").AddressInfo} */ (server.address());
  // The art lives on the mock, like DLsite's on its own host.
  link.dlsite.items[0].image = `http://127.0.0.1:${port}/dlsite-art/a1.svg`;

  return {
    link,
    url: `http://127.0.0.1:${port}`,
    /** Change a button as if Windows changed it, and notify like windows-link does. */
    change(button) {
      const index = link.buttons.findIndex((b) => b.id === button.id);
      link.buttons[index] = button;
      broadcast({ type: "button", button });
    },
    /** Change the desktops as if Windows changed them, and notify like windows-link does. */
    setDesktops(desktops, reason = "changed") {
      link.desktops = desktops;
      broadcast({ type: "desktops", reason, desktops, unmatched: link.unmatched, error: null });
    },
    goDown() {
      link.down = true;
      for (const socket of sockets) socket.terminate();
    },
    comeUp() {
      link.down = false;
    },
    output,
    volume,
    close: () =>
      new Promise((resolve) => {
        for (const socket of sockets) socket.terminate();
        wss.close();
        server.close(resolve);
        // The page may still hold keep-alive connections; do not wait for them.
        server.closeAllConnections();
      }),
  };
}

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

export function initialButtons() {
  return [output("motu"), volume(1)];
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
  };
  const sockets = new Set();
  const broadcast = (message) => {
    for (const socket of sockets) socket.send(JSON.stringify(message));
  };

  const server = http.createServer(async (req, res) => {
    res.setHeader("Access-Control-Allow-Origin", "*");
    if (req.method === "OPTIONS") {
      res.setHeader("Access-Control-Allow-Methods", "GET, POST, PUT");
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
      }),
  };
}
